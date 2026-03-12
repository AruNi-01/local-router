mod go;
mod node;
mod python;
mod repo;
mod rust;
mod shared;

use std::{collections::BTreeMap, fs, path::Path};

use anyhow::{Context, Result};
use walkdir::WalkDir;

use super::schema::{ManifestService, ProjectManifest};

use repo::RepoLayout;
pub use repo::classify_repo;
use shared::{DetectedCandidate, dedupe_candidates};

pub fn autodetect_manifest(path: &Path) -> Result<ProjectManifest> {
    let project_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("project")
        .to_string();
    let repo = classify_repo(path);
    let mut manifest = ProjectManifest {
        project: project_name,
        ..ProjectManifest::default()
    };

    for candidate in detect_candidates(path, &repo)? {
        if let Some((service_name, service)) = candidate.into_manifest_service() {
            insert_manifest_service(&mut manifest, service_name, service);
        }
    }

    if manifest.services.is_empty() {
        manifest.services.insert(
            "app".to_string(),
            ManifestService {
                command: "python -m http.server ${PORT}".to_string(),
                cwd: None,
                protocol: Some("http".to_string()),
                adapter: Some("generic".to_string()),
                route: Some("app".to_string()),
                healthcheck: Some("http://127.0.0.1:${PORT}".to_string()),
                env: BTreeMap::new(),
                depends_on: Vec::new(),
                disabled: Some(false),
                language: Some("generic".to_string()),
            },
        );
    }

    Ok(manifest)
}

fn detect_candidates(project_root: &Path, repo: &RepoLayout) -> Result<Vec<DetectedCandidate>> {
    let mut candidates = Vec::new();

    for package_json in WalkDir::new(project_root)
        .max_depth(4)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|entry| entry.file_name() == "package.json")
    {
        let raw = fs::read_to_string(package_json.path()).context("failed to read package.json")?;
        let parsed: serde_json::Value =
            serde_json::from_str(&raw).context("failed to parse package.json")?;
        if let Some(candidate) =
            node::detect_node_candidate(project_root, package_json.path(), &parsed, repo)
        {
            candidates.push(candidate);
        }
    }

    for cargo_toml in WalkDir::new(project_root)
        .max_depth(5)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|entry| entry.file_name() == "Cargo.toml")
    {
        if let Some(candidate) =
            rust::detect_cargo_candidate(project_root, cargo_toml.path(), repo)?
        {
            candidates.push(candidate);
        }
    }

    for pyproject in WalkDir::new(project_root)
        .max_depth(5)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|entry| entry.file_name() == "pyproject.toml")
    {
        if let Some(candidate) =
            python::detect_python_candidate(project_root, pyproject.path(), repo)?
        {
            candidates.push(candidate);
        }
    }

    for manage_py in WalkDir::new(project_root)
        .max_depth(5)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|entry| entry.file_name() == "manage.py")
    {
        candidates.push(python::detect_django_candidate(
            project_root,
            manage_py.path(),
        ));
    }

    for go_mod in WalkDir::new(project_root)
        .max_depth(5)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|entry| entry.file_name() == "go.mod")
    {
        if let Some(candidate) = go::detect_go_candidate(project_root, go_mod.path(), repo)? {
            candidates.push(candidate);
        }
    }

    Ok(dedupe_candidates(candidates))
}

fn insert_manifest_service(
    manifest: &mut ProjectManifest,
    service_name: String,
    service: ManifestService,
) {
    if !manifest.services.contains_key(&service_name) {
        manifest.services.insert(service_name, service);
        return;
    }

    let mut suffix = 2usize;
    loop {
        let candidate = format!("{service_name}-{suffix}");
        if !manifest.services.contains_key(&candidate) {
            manifest.services.insert(candidate, service);
            return;
        }
        suffix += 1;
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use anyhow::Result;
    use tempfile::tempdir;

    use super::autodetect_manifest;

    #[test]
    fn autodetect_classifies_mixed_workspace_members() -> Result<()> {
        let temp = tempdir()?;
        let root = temp.path();

        fs::write(root.join("bun.lock"), "")?;
        fs::write(
            root.join("package.json"),
            r#"{
              "name": "atmos",
              "private": true,
              "workspaces": ["apps/*", "crates/*"],
              "scripts": {
                "dev": "bun --filter web dev"
              }
            }"#,
        )?;

        fs::create_dir_all(root.join("apps/web"))?;
        fs::write(
            root.join("apps/web/package.json"),
            r#"{
              "name": "web",
              "scripts": { "dev": "next dev --turbopack --port 3030" },
              "dependencies": { "next": "16.0.0" },
              "devDependencies": { "typescript": "^5.0.0" }
            }"#,
        )?;

        fs::create_dir_all(root.join("apps/docs"))?;
        fs::write(
            root.join("apps/docs/package.json"),
            r#"{
              "name": "docs",
              "scripts": { "dev": "next dev" },
              "dependencies": { "next": "16.0.0" }
            }"#,
        )?;

        fs::create_dir_all(root.join("apps/desktop"))?;
        fs::write(
            root.join("apps/desktop/package.json"),
            r#"{
              "name": "desktop",
              "scripts": { "dev": "tauri dev" }
            }"#,
        )?;

        fs::create_dir_all(root.join("apps/api/src"))?;
        fs::write(root.join("apps/api/src/main.rs"), "fn main() {}")?;
        fs::write(
            root.join("apps/api/Cargo.toml"),
            r#"[package]
name = "api"
version = "0.1.0"
edition = "2021"

[dependencies]
axum = "0.8"
"#,
        )?;

        let manifest = autodetect_manifest(root)?;

        assert!(manifest.services.contains_key("web"));
        assert!(manifest.services.contains_key("docs"));
        assert!(manifest.services.contains_key("api"));
        assert!(!manifest.services.contains_key("atmos"));

        let desktop = manifest.services.get("desktop").unwrap();
        assert_eq!(desktop.protocol.as_deref(), Some("none"));
        assert_eq!(desktop.route.as_deref(), Some("none"));
        assert_eq!(desktop.adapter.as_deref(), Some("tauri"));

        let web = manifest.services.get("web").unwrap();
        assert_eq!(web.adapter.as_deref(), Some("nextjs"));
        assert_eq!(web.language.as_deref(), Some("typescript"));
        assert_eq!(
            web.command,
            "bun run dev -- --hostname ${HOST} --port ${PORT}"
        );

        let api = manifest.services.get("api").unwrap();
        assert_eq!(api.adapter.as_deref(), Some("cargo-bin"));
        assert_eq!(api.language.as_deref(), Some("rust"));
        assert_eq!(api.cwd.as_deref(), Some("apps/api"));

        Ok(())
    }

    #[test]
    fn autodetect_detects_go_and_python_http_services() -> Result<()> {
        let temp = tempdir()?;
        let root = temp.path();

        fs::create_dir_all(root.join("services/go-api"))?;
        fs::write(
            root.join("services/go-api/go.mod"),
            r#"module github.com/example/go-api

go 1.23

require github.com/gin-gonic/gin v1.10.0
"#,
        )?;
        fs::write(
            root.join("services/go-api/main.go"),
            r#"package main

import "github.com/gin-gonic/gin"

func main() {
    router := gin.Default()
    router.Run()
}
"#,
        )?;

        fs::create_dir_all(root.join("services/py-api"))?;
        fs::write(
            root.join("services/py-api/pyproject.toml"),
            r#"[project]
name = "py-api"
dependencies = ["fastapi", "uvicorn"]
"#,
        )?;
        fs::write(
            root.join("services/py-api/main.py"),
            "from fastapi import FastAPI\napp = FastAPI()\n",
        )?;

        let manifest = autodetect_manifest(root)?;

        let go_api = manifest.services.get("go-api").unwrap();
        assert_eq!(go_api.adapter.as_deref(), Some("go-http"));
        assert_eq!(go_api.command, "go run .");
        assert_eq!(go_api.language.as_deref(), Some("go"));

        let py_api = manifest.services.get("py-api").unwrap();
        assert_eq!(py_api.adapter.as_deref(), Some("fastapi"));
        assert_eq!(
            py_api.command,
            "python -m uvicorn main:app --host ${HOST} --port ${PORT}"
        );
        assert_eq!(py_api.language.as_deref(), Some("python"));

        Ok(())
    }
}
