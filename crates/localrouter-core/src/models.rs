use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum HealthStatus {
    Healthy,
    Unhealthy,
    Starting,
    Stopped,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum RouteStatus {
    Active,
    Stale,
    Conflict,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum LogLevel {
    Info,
    Warn,
    Error,
    Debug,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Project {
    pub id: String,
    pub name: String,
    pub path: String,
    pub created_at: String,
    pub config_source: String,
    #[serde(default)]
    pub proxy_disabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Workspace {
    pub id: String,
    pub project_id: String,
    pub name: String,
    pub branch: String,
    pub path: String,
    pub is_active: bool,
    pub slug: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServiceDef {
    pub id: String,
    pub project_id: String,
    pub name: String,
    pub command: String,
    pub protocol: String,
    pub adapter: String,
    pub route: String,
    pub healthcheck: String,
    pub language: String,
    pub cwd: Option<String>,
    #[serde(default)]
    pub env: BTreeMap<String, String>,
    #[serde(default)]
    pub depends_on: Vec<String>,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Instance {
    pub id: String,
    pub service_id: String,
    pub service_name: String,
    pub workspace_id: String,
    pub workspace_name: String,
    pub project_id: String,
    pub project_name: String,
    pub port: u16,
    pub pid: u32,
    pub status: HealthStatus,
    pub url: String,
    pub uptime: String,
    pub cpu: f32,
    pub memory: u64,
    pub started_at: Option<String>,
    pub last_exit: Option<i32>,
    pub status_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Route {
    pub id: String,
    pub pattern: String,
    #[serde(default)]
    pub url: String,
    pub target: String,
    pub service_id: String,
    pub service_name: String,
    pub workspace_id: String,
    pub workspace_name: String,
    pub project_id: String,
    pub project_name: String,
    pub status: RouteStatus,
    pub conflict_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LogEntry {
    pub timestamp: String,
    pub level: LogLevel,
    pub source: String,
    pub message: String,
    pub instance_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphNode {
    pub id: String,
    #[serde(rename = "type")]
    pub node_type: String,
    pub label: String,
    pub status: Option<HealthStatus>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphEdge {
    pub source: String,
    pub target: String,
    #[serde(rename = "type")]
    pub edge_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphSnapshot {
    pub nodes: Vec<GraphNode>,
    pub edges: Vec<GraphEdge>,
    pub generated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectDetail {
    pub project: Project,
    pub workspaces: Vec<Workspace>,
    pub services: Vec<ServiceDef>,
    pub instances: Vec<Instance>,
    pub routes: Vec<Route>,
    pub manifest: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DaemonConfig {
    pub api_port: u16,
    pub proxy_port: u16,
    pub dns_suffix: String,
    pub log_level: String,
    pub healthcheck_interval: u64,
    #[serde(default = "default_dependency_ready_timeout")]
    pub dependency_ready_timeout: u64,
    pub auto_detect: bool,
    pub hot_reload: bool,
}

fn default_dependency_ready_timeout() -> u64 {
    30
}

impl Default for DaemonConfig {
    fn default() -> Self {
        Self {
            api_port: 9731,
            proxy_port: 9730,
            dns_suffix: ".localhost".to_string(),
            log_level: "info".to_string(),
            healthcheck_interval: 10,
            dependency_ready_timeout: default_dependency_ready_timeout(),
            auto_detect: true,
            hot_reload: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HealthResponse {
    pub ok: bool,
    pub daemon: String,
    pub api_port: u16,
    pub proxy_port: u16,
    pub counts: DashboardStats,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct DashboardStats {
    pub total_projects: usize,
    pub active_workspaces: usize,
    pub running_instances: usize,
    pub unhealthy_instances: usize,
    pub stopped_instances: usize,
    pub active_routes: usize,
    pub conflict_routes: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EventsEnvelope {
    pub event_type: String,
    pub timestamp: String,
    pub payload: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AddProjectRequest {
    pub path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManifestUpdateRequest {
    pub manifest: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogsQuery {
    pub instance_id: Option<String>,
    pub cursor: Option<usize>,
    pub limit: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RegistrySnapshot {
    pub projects: Vec<Project>,
    pub workspaces: Vec<Workspace>,
    pub services: Vec<ServiceDef>,
    pub instances: Vec<Instance>,
    pub routes: Vec<Route>,
}

pub fn now_rfc3339() -> String {
    Utc::now().to_rfc3339()
}

pub fn uptime_string(started_at: Option<&str>) -> String {
    let Some(started_at) = started_at else {
        return "—".to_string();
    };
    let parsed = DateTime::parse_from_rfc3339(started_at).ok();
    let Some(parsed) = parsed else {
        return "—".to_string();
    };
    let elapsed = Utc::now().signed_duration_since(parsed.with_timezone(&Utc));
    if elapsed.num_seconds() < 60 {
        format!("{}s", elapsed.num_seconds())
    } else if elapsed.num_minutes() < 60 {
        format!("{}m", elapsed.num_minutes())
    } else {
        format!("{}h {}m", elapsed.num_hours(), elapsed.num_minutes() % 60)
    }
}
