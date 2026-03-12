use std::{fs, path::Path};

use anyhow::{Context, Result};

use super::{
    repo::RepoLayout,
    shared::{
        CandidateKind, DetectedCandidate, DetectionConfidence, build_http_service,
        build_non_http_service, is_rust_http_service, parse_cargo_package_name,
        relative_source_key,
    },
};
use crate::manifest::utils::slugify;

pub fn detect_cargo_candidate(
    project_root: &Path,
    cargo_toml_path: &Path,
    repo: &RepoLayout,
) -> Result<Option<DetectedCandidate>> {
    let raw = fs::read_to_string(cargo_toml_path).context("failed to read Cargo.toml")?;
    let package_dir = cargo_toml_path.parent().unwrap_or(project_root);
    let (relative, source_key) = relative_source_key(project_root, package_dir);

    if !raw.contains("[package]") {
        if raw.contains("[workspace]") || (relative.is_none() && repo.has_rust_workspace) {
            return Ok(Some(DetectedCandidate::hidden(
                slugify(
                    package_dir
                        .file_name()
                        .and_then(|segment| segment.to_str())
                        .unwrap_or("workspace"),
                ),
                source_key,
                CandidateKind::WorkspaceRoot,
                DetectionConfidence::Low,
            )));
        }
        return Ok(None);
    }

    let package_name = parse_cargo_package_name(&raw).unwrap_or_else(|| {
        package_dir
            .file_name()
            .and_then(|segment| segment.to_str())
            .unwrap_or("app")
            .to_string()
    });
    let service_name = slugify(&package_name);
    let has_main = package_dir.join("src/main.rs").exists()
        || package_dir.join("src/bin").exists()
        || raw.contains("[[bin]]");
    if !has_main {
        return Ok(Some(DetectedCandidate::hidden(
            service_name,
            source_key,
            CandidateKind::Library,
            DetectionConfidence::Low,
        )));
    }

    let lower = raw.to_ascii_lowercase();
    if lower.contains("tauri") {
        return Ok(Some(DetectedCandidate::non_http(
            service_name,
            source_key,
            DetectionConfidence::High,
            build_non_http_service(
                "cargo tauri dev".to_string(),
                relative,
                "tauri".to_string(),
                "rust".to_string(),
            ),
        )));
    }

    if is_rust_http_service(&lower) {
        return Ok(Some(DetectedCandidate::http(
            service_name.clone(),
            source_key,
            DetectionConfidence::High,
            build_http_service(
                "cargo run".to_string(),
                relative,
                "cargo-bin".to_string(),
                service_name,
                "rust".to_string(),
            ),
        )));
    }

    Ok(Some(DetectedCandidate::hidden(
        service_name,
        source_key,
        CandidateKind::Tooling,
        DetectionConfidence::Low,
    )))
}
