use std::{collections::VecDeque, path::Path};

use crate::{
    manifest::{
        LoadedProject, LoadedWorkspace, load_project_with_options, parse_manifest, stable_id,
        validate_service_paths, write_manifest_to_disk,
    },
    models::{AddProjectRequest, HealthStatus, Instance, ProjectDetail, ServiceDef},
    storage::PersistedState,
};
use anyhow::{Context, Result, anyhow};
use nix::{
    sys::signal::{Signal, kill},
    unistd::Pid,
};
use serde_json::json;

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
        let manifest = inner.manifests.get(project_id).cloned().unwrap_or_default();
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
            manifest,
        })
    }

    pub async fn add_project_request(&self, request: AddProjectRequest) -> Result<ProjectDetail> {
        let loaded = self
            .load_project_from_disk(Path::new(&request.path))
            .await?;
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
        let loaded = self
            .load_project_from_disk(Path::new(&project.path))
            .await?;
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

        let project_path = {
            let inner = self.inner.read().await;
            inner
                .projects
                .get(project_id)
                .map(|p| p.path.clone())
                .ok_or_else(|| anyhow!("project not found"))?
        };

        validate_service_paths(Path::new(&project_path), &manifest)?;

        write_manifest_to_disk(&project_path, &raw)?;
        let loaded = self
            .load_project_from_disk(Path::new(&project_path))
            .await?;
        self.register_project(loaded).await?;

        self.emit("service_spec_changed", json!({ "projectId": project_id }))
            .await;
        self.project_detail(project_id).await
    }

    pub(crate) async fn bootstrap_if_empty(&self) -> Result<()> {
        if !self.inner.read().await.projects.is_empty() {
            return Ok(());
        }
        let cwd = std::env::current_dir().context("failed to read current directory")?;
        if let Ok(loaded) = self.load_project_from_disk(&cwd).await {
            self.register_project(loaded).await?;
        }
        Ok(())
    }

    pub(crate) async fn register_project(&self, loaded: LoadedProject) -> Result<()> {
        let project_id = loaded.project.id.clone();
        let project_name = loaded.project.name.clone();
        let config = self.config.read().await.clone();
        let manifest = loaded.manifest.clone();
        let workspaces = loaded
            .workspaces
            .iter()
            .map(|entry| entry.workspace.clone())
            .collect::<Vec<_>>();
        let services = loaded
            .workspaces
            .iter()
            .flat_map(|entry| entry.services.iter().cloned())
            .collect::<Vec<_>>();
        let restart_ids = self.instance_ids_to_restart(&loaded).await;

        for instance_id in &restart_ids {
            let _ = self
                .stop_instance_for_reconcile(
                    instance_id,
                    "Stopped because project configuration changed.",
                )
                .await;
        }

        {
            let mut inner = self.inner.write().await;
            let previous_instances = inner
                .instances
                .values()
                .filter(|instance| instance.project_id == project_id)
                .cloned()
                .map(|instance| (instance.id.clone(), instance))
                .collect::<std::collections::BTreeMap<_, _>>();

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
            for LoadedWorkspace {
                workspace,
                services,
            } in &loaded.workspaces
            {
                for service in services {
                    let instance_id = stable_id(
                        "inst",
                        &format!("{}:{}:{}", project_id, workspace.id, service.id),
                    );
                    let desired_instance = registered_instance(
                        &instance_id,
                        service,
                        workspace,
                        &project_id,
                        &project_name,
                    );
                    let next_instance = previous_instances
                        .get(&instance_id)
                        .map(|existing| merge_instance_state(existing, &desired_instance))
                        .unwrap_or(desired_instance);
                    let route_target = (next_instance.pid > 0 && next_instance.port > 0)
                        .then(|| format!("127.0.0.1:{}", next_instance.port));
                    inner.instances.insert(instance_id.clone(), next_instance);
                    if service.enabled && service.route != "none" {
                        for route in build_project_routes(
                            &loaded.project,
                            workspace,
                            service,
                            workspaces.len() == 1,
                            route_target.clone(),
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

    async fn load_project_from_disk(&self, path: &Path) -> Result<LoadedProject> {
        let allow_autodetect = self.config.read().await.auto_detect;
        load_project_with_options(path, allow_autodetect)
    }

    async fn instance_ids_to_restart(&self, loaded: &LoadedProject) -> Vec<String> {
        let desired_workspaces = loaded
            .workspaces
            .iter()
            .map(|entry| (entry.workspace.id.clone(), entry.workspace.clone()))
            .collect::<std::collections::BTreeMap<_, _>>();
        let desired_services = loaded
            .workspaces
            .iter()
            .flat_map(|entry| entry.services.iter().cloned())
            .map(|service| (service.id.clone(), service))
            .collect::<std::collections::BTreeMap<_, _>>();
        let desired_instance_ids = loaded
            .workspaces
            .iter()
            .flat_map(|entry| {
                entry.services.iter().map(move |service| {
                    stable_id(
                        "inst",
                        &format!(
                            "{}:{}:{}",
                            loaded.project.id, entry.workspace.id, service.id
                        ),
                    )
                })
            })
            .collect::<std::collections::BTreeSet<_>>();

        let inner = self.inner.read().await;
        let previous_project = inner.projects.get(&loaded.project.id);
        inner
            .instances
            .values()
            .filter(|instance| instance.project_id == loaded.project.id && instance.pid > 0)
            .filter(|instance| {
                let Some(service) = desired_services.get(&instance.service_id) else {
                    return true;
                };
                let Some(workspace) = desired_workspaces.get(&instance.workspace_id) else {
                    return true;
                };

                !desired_instance_ids.contains(&instance.id)
                    || !matches_project_runtime(previous_project, &loaded.project)
                    || service_requires_restart(inner.services.get(&instance.service_id), service)
                    || workspace_requires_restart(
                        inner.workspaces.get(&instance.workspace_id),
                        workspace,
                    )
            })
            .map(|instance| instance.id.clone())
            .collect()
    }

    async fn stop_instance_for_reconcile(
        &self,
        instance_id: &str,
        reason: &str,
    ) -> Result<Instance> {
        self.stop_instance_inner(instance_id, reason).await
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

fn registered_instance(
    instance_id: &str,
    service: &ServiceDef,
    workspace: &crate::models::Workspace,
    project_id: &str,
    project_name: &str,
) -> Instance {
    Instance {
        id: instance_id.to_string(),
        service_id: service.id.clone(),
        service_name: service.name.clone(),
        workspace_id: workspace.id.clone(),
        workspace_name: workspace.name.clone(),
        project_id: project_id.to_string(),
        project_name: project_name.to_string(),
        port: 0,
        pid: 0,
        status: HealthStatus::Stopped,
        url: String::new(),
        uptime: "—".to_string(),
        cpu: 0.0,
        memory: 0,
        started_at: None,
        last_exit: None,
        status_reason: Some(registered_instance_reason(service)),
    }
}

fn registered_instance_reason(service: &ServiceDef) -> String {
    if service.enabled {
        "Instance is registered but not running.".to_string()
    } else {
        "Disabled in manifest; enable the service before starting.".to_string()
    }
}

fn merge_instance_state(existing: &Instance, desired: &Instance) -> Instance {
    let mut next = desired.clone();
    next.port = existing.port;
    next.pid = existing.pid;
    next.status = existing.status.clone();
    next.url = existing.url.clone();
    next.uptime = existing.uptime.clone();
    next.cpu = existing.cpu;
    next.memory = existing.memory;
    next.started_at = existing.started_at.clone();
    next.last_exit = existing.last_exit;
    next.status_reason = if next.pid == 0 && next.status == HealthStatus::Stopped {
        Some(registered_instance_reason_from_instance(desired))
    } else {
        existing.status_reason.clone()
    };
    next
}

fn registered_instance_reason_from_instance(instance: &Instance) -> String {
    instance
        .status_reason
        .clone()
        .unwrap_or_else(|| "Instance is registered but not running.".to_string())
}

fn matches_project_runtime(
    current: Option<&crate::models::Project>,
    desired: &crate::models::Project,
) -> bool {
    current
        .map(|current| {
            current.name == desired.name && current.proxy_disabled == desired.proxy_disabled
        })
        .unwrap_or(false)
}

fn service_requires_restart(current: Option<&ServiceDef>, desired: &ServiceDef) -> bool {
    let Some(current) = current else {
        return true;
    };
    current.name != desired.name
        || current.command != desired.command
        || current.protocol != desired.protocol
        || current.adapter != desired.adapter
        || current.route != desired.route
        || current.healthcheck != desired.healthcheck
        || current.language != desired.language
        || current.cwd != desired.cwd
        || current.env != desired.env
        || current.depends_on != desired.depends_on
        || current.enabled != desired.enabled
}

fn workspace_requires_restart(
    current: Option<&crate::models::Workspace>,
    desired: &crate::models::Workspace,
) -> bool {
    let Some(current) = current else {
        return true;
    };
    current.path != desired.path || current.slug != desired.slug
}

#[cfg(test)]
mod tests {
    use std::{fs, path::Path, process::Command};

    use tempfile::tempdir;

    use crate::{models::AddProjectRequest, storage::PersistedState};

    use super::AppState;

    #[tokio::test]
    async fn rescan_preserves_running_instances_when_specs_do_not_change() {
        let temp = tempdir().unwrap();
        let project = temp.path().join("demo");
        fs::create_dir_all(&project).unwrap();
        fs::write(
            project.join("localrouter.yaml"),
            "project: demo\nservices:\n  worker:\n    command: sh -c 'sleep 30'\n    protocol: none\n    route: none\n",
        )
        .unwrap();

        let app = AppState::for_tests(PersistedState::default())
            .await
            .unwrap();
        let detail = app
            .add_project_request(AddProjectRequest {
                path: project.to_string_lossy().to_string(),
            })
            .await
            .unwrap();
        let instance_id = detail.instances[0].id.clone();

        let started = app.start_instance(&instance_id).await.unwrap();
        assert!(started.pid > 0);

        let rescanned = app.rescan_project(&detail.project.id).await.unwrap();
        let next = rescanned
            .instances
            .iter()
            .find(|instance| instance.id == instance_id)
            .unwrap();

        assert_eq!(next.pid, started.pid);

        let _ = app.stop_instance(&instance_id).await;
    }

    #[tokio::test]
    async fn register_project_uses_each_worktree_manifest_independently() {
        let temp = tempdir().unwrap();
        let root = temp.path().join("repo");
        fs::create_dir_all(&root).unwrap();

        run_git(&root, &["init"]);
        run_git(&root, &["branch", "-M", "main"]);
        fs::write(
            root.join("localrouter.yaml"),
            "project: demo\nservices:\n  web:\n    command: echo root\n    protocol: none\n    route: none\n",
        )
        .unwrap();
        run_git(&root, &["add", "localrouter.yaml"]);
        run_git(
            &root,
            &[
                "-c",
                "user.name=LocalRouter",
                "-c",
                "user.email=localrouter@example.com",
                "commit",
                "-m",
                "init",
            ],
        );

        let worktree = temp.path().join("repo-wt");
        run_git(
            &root,
            &[
                "worktree",
                "add",
                "-b",
                "feature/worktree",
                worktree.to_str().unwrap(),
                "HEAD",
            ],
        );
        fs::write(
            worktree.join("localrouter.yaml"),
            "project: demo\nservices:\n  api:\n    command: echo worktree\n    protocol: none\n    route: none\n",
        )
        .unwrap();

        let app = AppState::for_tests(PersistedState::default())
            .await
            .unwrap();
        let detail = app
            .add_project_request(AddProjectRequest {
                path: root.to_string_lossy().to_string(),
            })
            .await
            .unwrap();

        let root_path = root.canonicalize().unwrap().to_string_lossy().to_string();
        let worktree_path = worktree
            .canonicalize()
            .unwrap()
            .to_string_lossy()
            .to_string();
        let root_workspace = detail
            .workspaces
            .iter()
            .find(|workspace| workspace.path == root_path)
            .unwrap();
        let worktree_workspace = detail
            .workspaces
            .iter()
            .find(|workspace| workspace.path == worktree_path)
            .unwrap();

        assert!(detail.instances.iter().any(|instance| {
            instance.workspace_id == root_workspace.id && instance.service_name == "web"
        }));
        assert!(!detail.instances.iter().any(|instance| {
            instance.workspace_id == root_workspace.id && instance.service_name == "api"
        }));
        assert!(detail.instances.iter().any(|instance| {
            instance.workspace_id == worktree_workspace.id && instance.service_name == "api"
        }));
        assert!(!detail.instances.iter().any(|instance| {
            instance.workspace_id == worktree_workspace.id && instance.service_name == "web"
        }));
    }

    fn run_git(dir: &Path, args: &[&str]) {
        let status = Command::new("git")
            .arg("-C")
            .arg(dir)
            .args(args)
            .status()
            .unwrap();
        assert!(status.success(), "git command failed: {:?}", args);
    }
}
