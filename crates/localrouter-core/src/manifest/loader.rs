use std::{collections::BTreeMap, fs, path::Path};

use anyhow::{Context, Result, anyhow};

use crate::models::{Project, now_rfc3339};

use super::{
    detect::autodetect_manifest,
    schema::{
        KNOWN_ADAPTERS, KNOWN_PROTOCOLS, KNOWN_WORKSPACE_STRATEGIES, ManifestService,
        ProjectManifest,
    },
    service::services_from_manifest,
    utils::{is_valid_dns_label, resolve_service_cwd, slugify, stable_id},
    workspace::detect_workspace,
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
        let mut parsed: ProjectManifest =
            serde_yaml::from_str(&raw).context("failed to parse localrouter.yaml")?;
        if parsed.project.trim().is_empty() {
            parsed.project = canonical
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("project")
                .to_string();
        }
        validate_manifest(&parsed)?;
        validate_service_paths(&canonical, &parsed)?;
        (parsed, raw, "manifest".to_string())
    } else {
        let detected = autodetect_manifest(&canonical)?;
        let yaml = serde_yaml::to_string(&detected).context("failed to render manifest yaml")?;
        (detected, yaml, "autodetect".to_string())
    };

    let project_name = manifest.project.clone();
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
            proxy_disabled: manifest.proxy.disabled.unwrap_or(false),
        },
        workspace,
        services,
        manifest: manifest_text,
        config_source,
    })
}

pub fn parse_manifest(raw: &str) -> Result<ProjectManifest> {
    let parsed: ProjectManifest = serde_yaml::from_str(raw).context("failed to parse manifest")?;
    validate_manifest(&parsed)?;
    Ok(parsed)
}

pub fn validate_manifest(manifest: &ProjectManifest) -> Result<()> {
    if manifest.project.trim().is_empty() {
        return Err(anyhow!("manifest.project is required"));
    }
    if manifest.services.is_empty() {
        return Err(anyhow!(
            "manifest.services must define at least one service"
        ));
    }
    if !KNOWN_WORKSPACE_STRATEGIES.contains(&manifest.workspace.strategy.as_str()) {
        return Err(anyhow!(
            "unknown workspace.strategy '{}' (expected one of: {})",
            manifest.workspace.strategy,
            KNOWN_WORKSPACE_STRATEGIES.join(", ")
        ));
    }

    let mut route_slugs: BTreeMap<String, String> = BTreeMap::new();

    for (name, svc) in &manifest.services {
        validate_service(name, svc, &manifest.services)?;

        let route_name = svc.route.as_deref().unwrap_or(name);
        if route_name != "none" {
            let slug = slugify(route_name);
            if !is_valid_dns_label(&slug) {
                return Err(anyhow!(
                    "service '{}': route '{}' produces an invalid DNS label '{}' \
                     (must be 1-63 alphanumeric/hyphen characters, no leading/trailing hyphen)",
                    name,
                    route_name,
                    slug
                ));
            }
            if let Some(existing) = route_slugs.get(&slug) {
                return Err(anyhow!(
                    "services '{}' and '{}' produce the same route slug '{}'",
                    existing,
                    name,
                    slug
                ));
            }
            route_slugs.insert(slug, name.clone());
        }
    }
    Ok(())
}

fn validate_service(
    name: &str,
    svc: &ManifestService,
    all_services: &BTreeMap<String, ManifestService>,
) -> Result<()> {
    if svc.command.trim().is_empty() {
        return Err(anyhow!("service '{}' is missing command", name));
    }

    if svc.enabled.is_some() && svc.disabled.is_some() {
        return Err(anyhow!(
            "service '{}': cannot set both 'enabled' and 'disabled'; use one or the other",
            name
        ));
    }

    if let Some(ref protocol) = svc.protocol {
        if !KNOWN_PROTOCOLS.contains(&protocol.as_str()) {
            return Err(anyhow!(
                "service '{}': unknown protocol '{}' (expected one of: {})",
                name,
                protocol,
                KNOWN_PROTOCOLS.join(", ")
            ));
        }
    }

    if let Some(ref adapter) = svc.adapter {
        if !KNOWN_ADAPTERS.contains(&adapter.as_str()) {
            return Err(anyhow!(
                "service '{}': unknown adapter '{}' (expected one of: {})",
                name,
                adapter,
                KNOWN_ADAPTERS.join(", ")
            ));
        }
    }

    if let Some(ref route) = svc.route {
        if route != "none" {
            let slug = slugify(route);
            if !is_valid_dns_label(&slug) {
                return Err(anyhow!(
                    "service '{}': route '{}' produces an invalid DNS label '{}' \
                     (must be 1-63 alphanumeric/hyphen characters, no leading/trailing hyphen)",
                    name,
                    route,
                    slug
                ));
            }
        }
    }

    for dep in &svc.depends_on {
        if !all_services.contains_key(dep) {
            return Err(anyhow!(
                "service '{}': depends_on references '{}' which is not defined in services",
                name,
                dep
            ));
        }
    }

    if let Some(ref healthcheck) = svc.healthcheck {
        if !healthcheck.is_empty()
            && !healthcheck.starts_with("http://")
            && !healthcheck.starts_with("https://")
        {
            return Err(anyhow!(
                "service '{}': healthcheck '{}' must start with 'http://' or 'https://'",
                name,
                healthcheck
            ));
        }
    }

    validate_command_templates(name, &svc.command)?;

    Ok(())
}

fn validate_command_templates(name: &str, command: &str) -> Result<()> {
    let known = ["PORT", "HOST", "PUBLIC_URL"];
    for var in &known {
        let lower = format!("${{{}}}", var.to_lowercase());
        if command.contains(&lower) {
            return Err(anyhow!(
                "service '{}': found '{}' in command; template variables are case-sensitive, use '${{{}}}'",
                name,
                lower,
                var
            ));
        }
    }
    Ok(())
}

pub fn validate_service_paths(project_path: &Path, manifest: &ProjectManifest) -> Result<()> {
    for (name, svc) in &manifest.services {
        if let Some(ref cwd) = svc.cwd {
            let resolved =
                resolve_service_cwd(&project_path.to_string_lossy(), Some(cwd.as_str()));
            if !resolved.is_dir() {
                return Err(anyhow!(
                    "service '{}': cwd '{}' is not an existing directory (resolved to {})",
                    name,
                    cwd,
                    resolved.display()
                ));
            }
        }
    }
    Ok(())
}

pub fn write_manifest_to_disk(project_path: &str, manifest_yaml: &str) -> Result<()> {
    let manifest_path = Path::new(project_path).join("localrouter.yaml");
    fs::write(&manifest_path, manifest_yaml)
        .with_context(|| format!("failed to write {}", manifest_path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_invalid_protocol() {
        let yaml = "project: test\nservices:\n  web:\n    command: node server.js\n    protocol: ftp\n";
        let result = parse_manifest(yaml);
        assert!(result.is_err(), "expected error for protocol 'ftp'");
        let msg = result.unwrap_err().to_string();
        assert!(msg.contains("unknown protocol"), "expected 'unknown protocol' in: {msg}");
    }

    #[test]
    fn rejects_unknown_adapter() {
        let yaml = "project: test\nservices:\n  web:\n    command: node server.js\n    adapter: fake-thing\n";
        let result = parse_manifest(yaml);
        assert!(result.is_err(), "expected error for adapter 'fake-thing'");
    }

    #[test]
    fn rejects_missing_depends_on_ref() {
        let yaml = "project: test\nservices:\n  web:\n    command: node server.js\n    depends_on:\n      - ghost\n";
        let result = parse_manifest(yaml);
        assert!(result.is_err(), "expected error for missing depends_on reference");
    }

    #[test]
    fn rejects_enabled_and_disabled_together() {
        let yaml = "project: test\nservices:\n  web:\n    command: node server.js\n    enabled: true\n    disabled: true\n";
        let result = parse_manifest(yaml);
        assert!(result.is_err(), "expected error for both enabled and disabled");
    }

    #[test]
    fn rejects_invalid_healthcheck_url() {
        let yaml = "project: test\nservices:\n  web:\n    command: node server.js\n    healthcheck: just-text\n";
        let result = parse_manifest(yaml);
        assert!(result.is_err(), "expected error for invalid healthcheck");
    }

    #[test]
    fn rejects_lowercase_template_var() {
        let yaml = "project: test\nservices:\n  web:\n    command: \"node server.js --port ${port}\"\n";
        let result = parse_manifest(yaml);
        assert!(result.is_err(), "expected error for lowercase ${{port}}");
    }

    #[test]
    fn accepts_valid_manifest() {
        let yaml = "project: test\nservices:\n  web:\n    command: node server.js\n    protocol: http\n";
        let result = parse_manifest(yaml);
        assert!(result.is_ok(), "expected valid manifest to parse: {:?}", result.err());
    }

    #[test]
    fn rejects_empty_services() {
        let yaml = "project: test\nservices: {}\n";
        let result = parse_manifest(yaml);
        assert!(result.is_err(), "expected error for empty services");
    }
}
