use std::{fs, path::Path};

use anyhow::{Context, Result};

use super::{
    repo::RepoLayout,
    shared::{
        CandidateKind, DetectedCandidate, DetectionConfidence, build_http_service,
        go_source_haystack, parse_go_module_name, relative_source_key,
    },
};
use crate::manifest::utils::slugify;

pub fn detect_go_candidate(
    project_root: &Path,
    go_mod_path: &Path,
    repo: &RepoLayout,
) -> Result<Option<DetectedCandidate>> {
    let raw = fs::read_to_string(go_mod_path).context("failed to read go.mod")?;
    let package_dir = go_mod_path.parent().unwrap_or(project_root);
    let (relative, source_key) = relative_source_key(project_root, package_dir);
    let module_name = parse_go_module_name(&raw).unwrap_or_else(|| {
        package_dir
            .file_name()
            .and_then(|segment| segment.to_str())
            .unwrap_or("go-app")
            .to_string()
    });
    let service_name = slugify(module_name.rsplit('/').next().unwrap_or(&module_name));

    if relative.is_none() && repo.has_go_workspace && !package_dir.join("main.go").exists() {
        return Ok(Some(DetectedCandidate::hidden(
            service_name,
            source_key,
            CandidateKind::WorkspaceRoot,
            DetectionConfidence::Low,
        )));
    }

    let source_haystack = go_source_haystack(package_dir);
    let lower = format!("{raw}\n{source_haystack}").to_ascii_lowercase();
    let has_main = source_haystack.contains("package main");
    if !has_main {
        return Ok(Some(DetectedCandidate::hidden(
            service_name,
            source_key,
            CandidateKind::Library,
            DetectionConfidence::Low,
        )));
    }

    let is_http = lower.contains("github.com/gin-gonic/gin")
        || lower.contains("github.com/gofiber/fiber")
        || lower.contains("github.com/labstack/echo")
        || lower.contains("github.com/go-chi/chi")
        || (lower.contains("\"net/http\"") && lower.contains("listenandserve"));
    if !is_http {
        return Ok(Some(DetectedCandidate::hidden(
            service_name,
            source_key,
            CandidateKind::Tooling,
            DetectionConfidence::Low,
        )));
    }

    Ok(Some(DetectedCandidate::http(
        service_name.clone(),
        source_key,
        DetectionConfidence::High,
        build_http_service(
            "go run .".to_string(),
            relative,
            "go-http".to_string(),
            service_name,
            "go".to_string(),
        ),
    )))
}
