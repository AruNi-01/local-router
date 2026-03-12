use std::{fs, path::Path};

use anyhow::{Context, Result, anyhow};

use crate::models::{Project, now_rfc3339};

use super::{
    detect::autodetect_manifest, schema::ProjectManifest, service::services_from_manifest,
    utils::stable_id, workspace::detect_workspace,
};

pub struct LoadedProject {
    pub project: Project,
    pub workspace: crate::models::Workspace,
    pub services: Vec<crate::models::ServiceDef>,
    pub manifest: String,
    pub config_source: String,
}

pub fn load_project(path: &Path) -> Result<LoadedProject> {
    let canonical = path
        .canonicalize()
        .context("failed to resolve project path")?;
    let manifest_path = canonical.join("localrouter.yaml");
    let (manifest, manifest_text, config_source) = if manifest_path.exists() {
        let raw = fs::read_to_string(&manifest_path).context("failed to read localrouter.yaml")?;
        let parsed: ProjectManifest =
            serde_yaml::from_str(&raw).context("failed to parse localrouter.yaml")?;
        (parsed, raw, "manifest".to_string())
    } else {
        let detected = autodetect_manifest(&canonical)?;
        let yaml = serde_yaml::to_string(&detected).context("failed to render manifest yaml")?;
        (detected, yaml, "autodetect".to_string())
    };

    let project_name = if manifest.project.trim().is_empty() {
        canonical
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("project")
            .to_string()
    } else {
        manifest.project.clone()
    };
    let project_id = stable_id("proj", &canonical.to_string_lossy());
    let workspace = detect_workspace(&project_id, &canonical);
    let services = services_from_manifest(&project_id, &manifest)?;

    Ok(LoadedProject {
        project: Project {
            id: project_id,
            name: project_name,
            path: canonical.to_string_lossy().to_string(),
            created_at: now_rfc3339(),
            config_source: config_source.clone(),
        },
        workspace,
        services,
        manifest: manifest_text,
        config_source,
    })
}

pub fn parse_manifest(raw: &str) -> Result<ProjectManifest> {
    let parsed: ProjectManifest = serde_yaml::from_str(raw).context("failed to parse manifest")?;
    if parsed.project.trim().is_empty() {
        return Err(anyhow!("manifest.project is required"));
    }
    if parsed.services.is_empty() {
        return Err(anyhow!(
            "manifest.services must define at least one service"
        ));
    }
    Ok(parsed)
}
