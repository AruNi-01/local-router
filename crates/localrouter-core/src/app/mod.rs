mod events;
mod health;
mod registry;
mod routes;
mod runtime;

use std::{
    collections::{BTreeMap, HashMap, VecDeque},
    net::SocketAddr,
    sync::Arc,
};

use anyhow::Result;
use reqwest::Client;
use tokio::sync::{Mutex, RwLock, broadcast, watch};

use crate::{
    models::{
        DaemonConfig, DashboardStats, EventsEnvelope, GraphEdge, GraphNode, GraphSnapshot,
        HealthResponse, Instance, LogEntry, LogsQuery, Project, Route, RouteStatus, ServiceDef,
        Workspace, now_rfc3339,
    },
    storage::{PersistedState, Storage},
};

use self::{
    health::{project_status, workspace_status},
    routes::regenerate_routes_locked,
};

const MAX_LOG_LINES: usize = 1000;

#[derive(Default)]
struct RuntimeState {
    projects: BTreeMap<String, Project>,
    workspaces: BTreeMap<String, Workspace>,
    services: BTreeMap<String, ServiceDef>,
    instances: BTreeMap<String, Instance>,
    routes: BTreeMap<String, Route>,
    manifests: BTreeMap<String, String>,
    logs: VecDeque<LogEntry>,
}

#[derive(Clone)]
pub struct AppState {
    inner: Arc<RwLock<RuntimeState>>,
    storage: Storage,
    config: Arc<RwLock<DaemonConfig>>,
    events: broadcast::Sender<EventsEnvelope>,
    health_tasks: Arc<Mutex<HashMap<String, watch::Sender<bool>>>>,
    start_locks: Arc<Mutex<HashMap<String, Arc<Mutex<()>>>>>,
    client: Client,
}

impl AppState {
    pub async fn load() -> Result<Self> {
        let storage = Storage::open_default()?;
        let persisted = storage.load()?;
        Self::from_persisted(storage, persisted).await
    }

    async fn from_persisted(storage: Storage, persisted: PersistedState) -> Result<Self> {
        let inner = RuntimeState {
            projects: persisted
                .projects
                .into_iter()
                .map(|item| (item.id.clone(), item))
                .collect(),
            workspaces: persisted
                .workspaces
                .into_iter()
                .map(|item| (item.id.clone(), item))
                .collect(),
            services: persisted
                .services
                .into_iter()
                .map(|item| (item.id.clone(), item))
                .collect(),
            instances: persisted
                .instances
                .into_iter()
                .map(|item| (item.id.clone(), item))
                .collect(),
            routes: persisted
                .routes
                .into_iter()
                .map(|item| (item.id.clone(), item))
                .collect(),
            manifests: persisted.manifests,
            logs: persisted.logs,
        };
        let (events, _) = broadcast::channel(512);
        let app = Self {
            inner: Arc::new(RwLock::new(inner)),
            storage,
            config: Arc::new(RwLock::new(persisted.config)),
            events,
            health_tasks: Arc::new(Mutex::new(HashMap::new())),
            start_locks: Arc::new(Mutex::new(HashMap::new())),
            client: Client::builder()
                .redirect(reqwest::redirect::Policy::none())
                .build()?,
        };
        {
            let config = app.config.read().await.clone();
            let mut inner = app.inner.write().await;
            regenerate_routes_locked(&mut inner, &config.dns_suffix, config.proxy_port);
        }
        app.persist().await?;
        app.bootstrap_if_empty().await?;
        Ok(app)
    }

    #[cfg(test)]
    pub async fn for_tests(persisted: PersistedState) -> Result<Self> {
        let tmpdir = tempfile::tempdir()?;
        let storage = Storage::open_at(tmpdir.path().join("state.sqlite3"))?;
        let app = Self::from_persisted(storage, persisted).await?;
        std::mem::forget(tmpdir);
        Ok(app)
    }

    pub async fn health(&self) -> HealthResponse {
        let config = self.config.read().await.clone();
        let stats = self.stats().await;
        HealthResponse {
            ok: true,
            daemon: env!("CARGO_PKG_VERSION").to_string(),
            api_port: config.api_port,
            proxy_port: config.proxy_port,
            counts: stats,
        }
    }

    pub async fn config(&self) -> DaemonConfig {
        self.config.read().await.clone()
    }

    pub async fn update_config(&self, config: DaemonConfig) -> Result<DaemonConfig> {
        *self.config.write().await = config.clone();
        {
            let mut inner = self.inner.write().await;
            regenerate_routes_locked(&mut inner, &config.dns_suffix, config.proxy_port);
        }
        self.persist().await?;
        Ok(config)
    }

    pub async fn stats(&self) -> DashboardStats {
        self.refresh_metrics().await;
        let inner = self.inner.read().await;
        DashboardStats {
            total_projects: inner.projects.len(),
            active_workspaces: inner.workspaces.values().filter(|ws| ws.is_active).count(),
            running_instances: inner
                .instances
                .values()
                .filter(|inst| {
                    matches!(
                        inst.status,
                        crate::models::HealthStatus::Healthy
                            | crate::models::HealthStatus::Starting
                    )
                })
                .count(),
            unhealthy_instances: inner
                .instances
                .values()
                .filter(|inst| inst.status == crate::models::HealthStatus::Unhealthy)
                .count(),
            stopped_instances: inner
                .instances
                .values()
                .filter(|inst| inst.status == crate::models::HealthStatus::Stopped)
                .count(),
            active_routes: inner
                .routes
                .values()
                .filter(|route| route.status == RouteStatus::Active)
                .count(),
            conflict_routes: inner
                .routes
                .values()
                .filter(|route| route.status == RouteStatus::Conflict)
                .count(),
        }
    }

    pub async fn projects(&self) -> Vec<Project> {
        self.inner.read().await.projects.values().cloned().collect()
    }

    pub async fn services(&self) -> Vec<ServiceDef> {
        self.inner.read().await.services.values().cloned().collect()
    }

    pub async fn workspaces(&self) -> Vec<Workspace> {
        self.inner
            .read()
            .await
            .workspaces
            .values()
            .cloned()
            .collect()
    }

    pub async fn instances(&self) -> Vec<Instance> {
        self.refresh_metrics().await;
        self.inner
            .read()
            .await
            .instances
            .values()
            .cloned()
            .collect()
    }

    pub async fn routes(&self) -> Vec<Route> {
        self.inner.read().await.routes.values().cloned().collect()
    }

    pub async fn active_route_for_host(&self, host: &str) -> Option<Route> {
        self.inner
            .read()
            .await
            .routes
            .values()
            .find(|route| route.pattern == host && route.status == RouteStatus::Active)
            .cloned()
    }

    pub async fn logs(&self, query: LogsQuery) -> Vec<LogEntry> {
        let limit = query.limit.unwrap_or(200);
        let cursor = query.cursor.unwrap_or(0);
        let inner = self.inner.read().await;
        inner
            .logs
            .iter()
            .filter(|entry| {
                query
                    .instance_id
                    .as_ref()
                    .map(|id| &entry.instance_id == id)
                    .unwrap_or(true)
            })
            .skip(cursor)
            .take(limit)
            .cloned()
            .collect()
    }

    pub async fn graph(&self) -> GraphSnapshot {
        let inner = self.inner.read().await;
        let mut nodes = Vec::new();
        let mut edges = Vec::new();

        for project in inner.projects.values() {
            nodes.push(GraphNode {
                id: project.id.clone(),
                node_type: "project".to_string(),
                label: project.name.clone(),
                status: Some(project_status(&inner, &project.id)),
            });
        }
        for workspace in inner.workspaces.values() {
            nodes.push(GraphNode {
                id: workspace.id.clone(),
                node_type: "workspace".to_string(),
                label: workspace.name.clone(),
                status: Some(workspace_status(&inner, &workspace.id)),
            });
            edges.push(GraphEdge {
                source: workspace.project_id.clone(),
                target: workspace.id.clone(),
                edge_type: "contains".to_string(),
            });
        }
        for instance in inner.instances.values() {
            let service_node_id = format!("{}::{}", instance.service_id, instance.workspace_id);
            nodes.push(GraphNode {
                id: service_node_id.clone(),
                node_type: "service".to_string(),
                label: instance.service_name.clone(),
                status: Some(instance.status.clone()),
            });
            edges.push(GraphEdge {
                source: instance.workspace_id.clone(),
                target: service_node_id.clone(),
                edge_type: "contains".to_string(),
            });
        }
        for route in inner.routes.values() {
            nodes.push(GraphNode {
                id: route.id.clone(),
                node_type: "route".to_string(),
                label: route.pattern.clone(),
                status: None,
            });
            edges.push(GraphEdge {
                source: format!("{}::{}", route.service_id, route.workspace_id),
                target: route.id.clone(),
                edge_type: "exposes".to_string(),
            });
        }
        for service in inner.services.values() {
            for dep in &service.depends_on {
                if let Some(target) = inner
                    .services
                    .values()
                    .find(|candidate| &candidate.name == dep)
                {
                    for workspace in inner
                        .workspaces
                        .values()
                        .filter(|ws| ws.project_id == service.project_id)
                    {
                        edges.push(GraphEdge {
                            source: format!("{}::{}", service.id, workspace.id),
                            target: format!("{}::{}", target.id, workspace.id),
                            edge_type: "depends_on".to_string(),
                        });
                    }
                }
            }
        }

        GraphSnapshot {
            nodes,
            edges,
            generated_at: now_rfc3339(),
        }
    }
}

pub async fn api_addr(app: &AppState) -> SocketAddr {
    let port = app.config.read().await.api_port;
    SocketAddr::from(([127, 0, 0, 1], port))
}

pub async fn proxy_addr(app: &AppState) -> SocketAddr {
    let port = app.config.read().await.proxy_port;
    SocketAddr::from(([127, 0, 0, 1], port))
}
