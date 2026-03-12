use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ProjectManifest {
    pub project: String,
    #[serde(default)]
    pub workspace: WorkspaceStrategy,
    #[serde(default)]
    pub proxy: ManifestProxy,
    #[serde(default)]
    pub services: BTreeMap<String, ManifestService>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceStrategy {
    pub strategy: String,
}

impl Default for WorkspaceStrategy {
    fn default() -> Self {
        Self {
            strategy: "git-worktree".to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ManifestProxy {
    pub disabled: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ManifestService {
    pub command: String,
    pub cwd: Option<String>,
    pub protocol: Option<String>,
    pub adapter: Option<String>,
    pub route: Option<String>,
    pub healthcheck: Option<String>,
    #[serde(default)]
    pub env: BTreeMap<String, String>,
    #[serde(default)]
    pub depends_on: Vec<String>,
    pub disabled: Option<bool>,
    pub language: Option<String>,
}
