use std::{collections::VecDeque, path::Path};

use anyhow::{Context, Result, anyhow};
use nix::{
    sys::signal::{Signal, kill},
    unistd::Pid,
};
use serde_json::json;

use crate::{
    manifest::{LoadedProject, load_project, parse_manifest, services_from_manifest, stable_id},
    models::{AddProjectRequest, HealthStatus, Instance, ProjectDetail},
    storage::PersistedState,
};

use super::{
    AppState,
    routes::{build_project_routes, reconcile_routes_locked},
};

impl AppState {
    pub async fn project_detail(&self, project_id: &str) -> Result<ProjectDetail> {
        self.refresh_metrics().await;
        let inner = self.inner.read().await;
        let project = inner
            .projects
            .get(project_id)
            .cloned()
            .ok_or_else(|| anyhow!("project not found"))?;
        Ok(ProjectDetail {
            project: project.clone(),
            workspaces: inner
                .workspaces
                .values()
                .filter(|workspace| workspace.project_id == project.id)
                .cloned()
                .collect(),
            services: inner
                .services
                .values()
                .filter(|service| service.project_id == project.id)
                .cloned()
                .collect(),
            instances: inner
                .instances
                .values()
                .filter(|instance| instance.project_id == project.id)
                .cloned()
                .collect(),
            routes: inner
                .routes
                .values()
                .filter(|route| route.project_id == project.id)
                .cloned()
                .collect(),
            manifest: inner.manifests.get(project_id).cloned().unwrap_or_default(),
        })
    }

    pub async fn add_project_request(&self, request: AddProjectRequest) -> Result<ProjectDetail> {
        let loaded = load_project(Path::new(&request.path))?;
        let project_id = loaded.project.id.clone();
        self.register_project(loaded).await?;
        self.project_detail(&project_id).await
    }

    pub async fn rescan_project(&self, project_id: &str) -> Result<ProjectDetail> {
        let project = self
            .inner
            .read()
            .await
            .projects
            .get(project_id)
            .cloned()
            .ok_or_else(|| anyhow!("project not found"))?;
        let loaded = load_project(Path::new(&project.path))?;
        self.register_project(loaded).await?;
        self.project_detail(project_id).await
    }

    pub async fn remove_project(&self, project_id: &str) -> Result<()> {
        let pids = {
            let inner = self.inner.read().await;
            if !inner.projects.contains_key(project_id) {
                return Err(anyhow!("project not found"));
            }
            inner
                .instances
                .values()
                .filter(|instance| instance.project_id == project_id && instance.pid > 0)
                .map(|instance| instance.pid)
                .collect::<Vec<_>>()
        };
        for pid in pids {
            let _ = kill(Pid::from_raw(pid as i32), Signal::SIGTERM);
        }

        {
            let mut inner = self.inner.write().await;
            inner.projects.retain(|_, project| project.id != project_id);
            inner
                .workspaces
                .retain(|_, workspace| workspace.project_id != project_id);
            inner
                .services
                .retain(|_, service| service.project_id != project_id);
            inner
                .instances
                .retain(|_, instance| instance.project_id != project_id);
            inner
                .routes
                .retain(|_, route| route.project_id != project_id);
            inner.manifests.remove(project_id);
            inner
                .logs
                .retain(|entry| !entry.instance_id.starts_with(project_id));
            reconcile_routes_locked(&mut inner);
        }
        self.emit("project_removed", json!({ "projectId": project_id }))
            .await;
        self.persist().await?;
        Ok(())
    }

    pub async fn update_manifest(&self, project_id: &str, raw: String) -> Result<ProjectDetail> {
        let manifest = parse_manifest(&raw)?;
        let config = self.config.read().await.clone();
        let workspaces = self
            .inner
            .read()
            .await
            .workspaces
            .values()
            .filter(|workspace| workspace.project_id == project_id)
            .cloned()
            .collect::<Vec<_>>();
        let services = services_from_manifest(project_id, &manifest)?;

        {
            let mut inner = self.inner.write().await;
            inner.manifests.insert(project_id.to_string(), raw.clone());
            inner
                .services
                .retain(|_, service| service.project_id != project_id);
            inner
                .routes
                .retain(|_, route| route.project_id != project_id);

            for service in &services {
                inner.services.insert(service.id.clone(), service.clone());
            }

            for workspace in &workspaces {
                for service in &services {
                    let instance_id = stable_id(
                        "inst",
                        &format!("{project_id}:{}:{}", workspace.id, service.id),
                    );
                    inner
                        .instances
                        .entry(instance_id.clone())
                        .or_insert_with(|| Instance {
                            id: instance_id.clone(),
                            service_id: service.id.clone(),
                            service_name: service.name.clone(),
                            workspace_id: workspace.id.clone(),
                            workspace_name: workspace.name.clone(),
                            project_id: project_id.to_string(),
                            project_name: manifest.project.clone(),
                            port: 0,
                            pid: 0,
                            status: HealthStatus::Stopped,
                            url: String::new(),
                            uptime: "—".to_string(),
                            cpu: 0.0,
                            memory: 0,
                            started_at: None,
                            last_exit: None,
                            status_reason: Some(
                                "Instance is registered but not running.".to_string(),
                            ),
                        });
                    if service.route != "none" {
                        for route in build_project_routes(
                            &inner.projects[project_id],
                            workspace,
                            service,
                            workspaces.len() == 1,
                            None,
                            &config.dns_suffix,
                            config.proxy_port,
                        ) {
                            inner.routes.insert(route.id.clone(), route);
                        }
                    }
                }
            }
            reconcile_routes_locked(&mut inner);
        }
        self.emit("service_spec_changed", json!({ "projectId": project_id }))
            .await;
        self.persist().await?;
        self.project_detail(project_id).await
    }

    pub(crate) async fn bootstrap_if_empty(&self) -> Result<()> {
        if !self.inner.read().await.projects.is_empty() {
            return Ok(());
        }
        let cwd = std::env::current_dir().context("failed to read current directory")?;
        if let Ok(loaded) = load_project(&cwd) {
            self.register_project(loaded).await?;
        }
        Ok(())
    }

    pub(crate) async fn register_project(&self, loaded: LoadedProject) -> Result<()> {
        let project_id = loaded.project.id.clone();
        let project_name = loaded.project.name.clone();
        let config = self.config.read().await.clone();
        let workspaces = vec![loaded.workspace.clone()];
        let services = loaded.services.clone();
        let manifest = loaded.manifest.clone();

        {
            let mut inner = self.inner.write().await;
            inner
                .projects
                .insert(loaded.project.id.clone(), loaded.project.clone());
            inner
                .workspaces
                .retain(|_, workspace| workspace.project_id != project_id);
            inner
                .services
                .retain(|_, service| service.project_id != project_id);
            inner
                .instances
                .retain(|_, instance| instance.project_id != project_id);
            inner
                .routes
                .retain(|_, route| route.project_id != project_id);
            inner.manifests.insert(project_id.clone(), manifest);

            for workspace in &workspaces {
                inner
                    .workspaces
                    .insert(workspace.id.clone(), workspace.clone());
            }
            for service in &services {
                inner.services.insert(service.id.clone(), service.clone());
            }
            for workspace in &workspaces {
                for service in &services {
                    let instance_id = stable_id(
                        "inst",
                        &format!("{}:{}:{}", project_id, workspace.id, service.id),
                    );
                    inner.instances.insert(
                        instance_id.clone(),
                        Instance {
                            id: instance_id.clone(),
                            service_id: service.id.clone(),
                            service_name: service.name.clone(),
                            workspace_id: workspace.id.clone(),
                            workspace_name: workspace.name.clone(),
                            project_id: project_id.clone(),
                            project_name: project_name.clone(),
                            port: 0,
                            pid: 0,
                            status: HealthStatus::Stopped,
                            url: String::new(),
                            uptime: "—".to_string(),
                            cpu: 0.0,
                            memory: 0,
                            started_at: None,
                            last_exit: None,
                            status_reason: Some(
                                "Instance is registered but not running.".to_string(),
                            ),
                        },
                    );
                    if service.route != "none" {
                        for route in build_project_routes(
                            &loaded.project,
                            workspace,
                            service,
                            workspaces.len() == 1,
                            None,
                            &config.dns_suffix,
                            config.proxy_port,
                        ) {
                            inner.routes.insert(route.id.clone(), route);
                        }
                    }
                }
            }
            reconcile_routes_locked(&mut inner);
        }
        self.emit("project_registered", json!({ "projectId": project_id }))
            .await;
        self.emit("workspace_detected", json!({ "projectId": project_id }))
            .await;
        self.persist().await?;
        Ok(())
    }

    pub(crate) async fn persist(&self) -> Result<()> {
        let inner = self.inner.read().await;
        let snapshot = PersistedState {
            config: self.config.read().await.clone(),
            manifests: inner.manifests.clone(),
            projects: inner.projects.values().cloned().collect(),
            workspaces: inner.workspaces.values().cloned().collect(),
            services: inner.services.values().cloned().collect(),
            instances: inner.instances.values().cloned().collect(),
            routes: inner.routes.values().cloned().collect(),
            logs: VecDeque::new(),
        };
        self.storage.save(&snapshot)
    }
}
