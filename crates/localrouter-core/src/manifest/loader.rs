use std::{collections::BTreeMap, fs, path::Path};

use anyhow::{Context, Result, anyhow};

use crate::models::{Project, Workspace, now_rfc3339};

use super::{
    detect::autodetect_manifest,
    schema::{
        KNOWN_ADAPTERS, KNOWN_PROTOCOLS, KNOWN_WORKSPACE_STRATEGIES, ManifestService,
        ProjectManifest,
    },
    service::{services_from_manifest, services_from_manifest_for_workspace},
    utils::{is_valid_dns_label, resolve_service_cwd, slugify, stable_id},
    workspace::{detect_workspaces, project_identity},
};

#[derive(Debug, Clone)]
pub struct LoadedWorkspace {
    pub workspace: Workspace,
    pub services: Vec<crate::models::ServiceDef>,
}

#[derive(Debug)]
pub struct LoadedProject {
    pub project: Project,
    pub workspaces: Vec<LoadedWorkspace>,
    pub services: Vec<crate::models::ServiceDef>,
    pub manifest: String,
    pub config_source: String,
}

pub fn load_project(path: &Path) -> Result<LoadedProject> {
    load_project_with_options(path, true)
}

pub fn load_project_with_options(path: &Path, allow_autodetect: bool) -> Result<LoadedProject> {
    let canonical = path
        .canonicalize()
        .context("failed to resolve project path")?;
    let (manifest, manifest_text, config_source) =
        load_manifest_snapshot(&canonical, allow_autodetect)?;

    let project_name = manifest.project.clone();
    let project_id = stable_id("proj", &project_identity(&canonical));
    let workspaces = detect_workspaces(&project_id, &canonical)
        .into_iter()
        .map(|workspace| {
            let workspace_path = Path::new(&workspace.path);
            let (workspace_manifest, _, _) =
                load_manifest_snapshot(workspace_path, allow_autodetect)?;
            let services = services_from_manifest_for_workspace(
                &project_id,
                &workspace.id,
                &workspace_manifest,
            )?;
            Ok(LoadedWorkspace {
                workspace,
                services,
            })
        })
        .collect::<Result<Vec<_>>>()?;
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
        workspaces,
        services,
        manifest: manifest_text,
        config_source,
    })
}

fn load_manifest_snapshot(
    path: &Path,
    allow_autodetect: bool,
) -> Result<(ProjectManifest, String, String)> {
    let manifest_path = path.join("localrouter.yaml");
    if manifest_path.exists() {
        let raw = fs::read_to_string(&manifest_path).context("failed to read localrouter.yaml")?;
        let mut parsed: ProjectManifest =
            serde_yaml::from_str(&raw).context("failed to parse localrouter.yaml")?;
        if parsed.project.trim().is_empty() {
            parsed.project = path
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("project")
                .to_string();
        }
        validate_manifest(&parsed)?;
        validate_service_paths(path, &parsed)?;
        Ok((parsed, raw, "manifest".to_string()))
    } else if !allow_autodetect {
        Err(anyhow!(
            "localrouter.yaml not found at {} and auto-detect is disabled",
            manifest_path.display()
        ))
    } else {
        let detected = autodetect_manifest(path)?;
        let yaml = serde_yaml::to_string(&detected).context("failed to render manifest yaml")?;
        Ok((detected, yaml, "autodetect".to_string()))
    }
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
            let resolved = resolve_service_cwd(&project_path.to_string_lossy(), Some(cwd.as_str()));
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
    use std::{fs, path::Path, process::Command};

    use tempfile::tempdir;

    use super::{load_project, load_project_with_options, parse_manifest};

    #[test]
    fn rejects_invalid_protocol() {
        let yaml =
            "project: test\nservices:\n  web:\n    command: node server.js\n    protocol: ftp\n";
        let result = parse_manifest(yaml);
        assert!(result.is_err(), "expected error for protocol 'ftp'");
        let msg = result.unwrap_err().to_string();
        assert!(
            msg.contains("unknown protocol"),
            "expected 'unknown protocol' in: {msg}"
        );
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
        assert!(
            result.is_err(),
            "expected error for missing depends_on reference"
        );
    }

    #[test]
    fn rejects_enabled_and_disabled_together() {
        let yaml = "project: test\nservices:\n  web:\n    command: node server.js\n    enabled: true\n    disabled: true\n";
        let result = parse_manifest(yaml);
        assert!(
            result.is_err(),
            "expected error for both enabled and disabled"
        );
    }

    #[test]
    fn rejects_invalid_healthcheck_url() {
        let yaml = "project: test\nservices:\n  web:\n    command: node server.js\n    healthcheck: just-text\n";
        let result = parse_manifest(yaml);
        assert!(result.is_err(), "expected error for invalid healthcheck");
    }

    #[test]
    fn rejects_lowercase_template_var() {
        let yaml =
            "project: test\nservices:\n  web:\n    command: \"node server.js --port ${port}\"\n";
        let result = parse_manifest(yaml);
        assert!(result.is_err(), "expected error for lowercase ${{port}}");
    }

    #[test]
    fn accepts_valid_manifest() {
        let yaml =
            "project: test\nservices:\n  web:\n    command: node server.js\n    protocol: http\n";
        let result = parse_manifest(yaml);
        assert!(
            result.is_ok(),
            "expected valid manifest to parse: {:?}",
            result.err()
        );
    }

    #[test]
    fn rejects_empty_services() {
        let yaml = "project: test\nservices: {}\n";
        let result = parse_manifest(yaml);
        assert!(result.is_err(), "expected error for empty services");
    }

    #[test]
    fn load_project_requires_manifest_when_autodetect_is_disabled() {
        let temp = tempdir().unwrap();
        let result = load_project_with_options(temp.path(), false);
        assert!(result.is_err(), "expected missing manifest to fail");
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("auto-detect is disabled")
        );
    }

    #[test]
    fn load_project_discovers_linked_worktrees_and_stable_project_id() {
        let temp = tempdir().unwrap();
        let root = temp.path().join("repo");
        fs::create_dir_all(&root).unwrap();

        run_git(&root, &["init"]);
        run_git(&root, &["branch", "-M", "main"]);
        fs::write(
            root.join("localrouter.yaml"),
            "project: demo\nservices:\n  web:\n    command: echo hello\n    protocol: none\n    route: none\n",
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
        let worktree_canonical = worktree.canonicalize().unwrap();
        fs::write(
            worktree.join("localrouter.yaml"),
            "project: demo\nservices:\n  api:\n    command: echo worktree\n    protocol: none\n    route: none\n",
        )
        .unwrap();

        let root_loaded = load_project(&root).unwrap();
        let worktree_loaded = load_project(&worktree).unwrap();

        assert_eq!(root_loaded.project.id, worktree_loaded.project.id);
        assert_eq!(root_loaded.workspaces.len(), 2);
        assert_eq!(worktree_loaded.workspaces.len(), 2);
        assert!(root_loaded.workspaces.iter().any(|entry| {
            entry.workspace.path == worktree_canonical.to_string_lossy().as_ref()
        }));
        let root_entry = root_loaded
            .workspaces
            .iter()
            .find(|entry| entry.workspace.path == root.canonicalize().unwrap().to_string_lossy())
            .unwrap();
        let worktree_entry = root_loaded
            .workspaces
            .iter()
            .find(|entry| entry.workspace.path == worktree_canonical.to_string_lossy())
            .unwrap();
        assert_eq!(
            root_entry
                .services
                .iter()
                .map(|service| service.name.as_str())
                .collect::<Vec<_>>(),
            vec!["web"]
        );
        assert_eq!(
            worktree_entry
                .services
                .iter()
                .map(|service| service.name.as_str())
                .collect::<Vec<_>>(),
            vec!["api"]
        );
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
