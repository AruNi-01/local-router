use std::{fs, path::Path};

use anyhow::{Context, Result};

use super::{
    repo::RepoLayout,
    shared::{
        CandidateKind, DetectedCandidate, DetectionConfidence, build_http_service,
        relative_source_key,
    },
};
use crate::manifest::utils::slugify;

pub fn detect_java_candidate(
    project_root: &Path,
    build_file_path: &Path,
    repo: &RepoLayout,
) -> Result<Option<DetectedCandidate>> {
    let raw = fs::read_to_string(build_file_path).context("failed to read Java build file")?;
    let file_name = build_file_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("");
    let package_dir = build_file_path.parent().unwrap_or(project_root);
    let (relative, source_key) = relative_source_key(project_root, package_dir);
    let lower = raw.to_ascii_lowercase();

    let is_maven = file_name == "pom.xml";
    let is_gradle = file_name.starts_with("build.gradle");

    let service_name = slugify(
        package_dir
            .file_name()
            .and_then(|segment| segment.to_str())
            .unwrap_or("java-app"),
    );

    if is_maven && lower.contains("<modules>") && relative.is_none() && repo.has_maven_workspace {
        return Ok(Some(DetectedCandidate::hidden(
            service_name,
            source_key,
            CandidateKind::WorkspaceRoot,
            DetectionConfidence::Low,
        )));
    }
    if is_gradle && relative.is_none() && repo.has_gradle_workspace {
        let has_subprojects = lower.contains("subprojects") || lower.contains("allprojects");
        if has_subprojects {
            return Ok(Some(DetectedCandidate::hidden(
                service_name,
                source_key,
                CandidateKind::WorkspaceRoot,
                DetectionConfidence::Low,
            )));
        }
    }

    let is_spring_web =
        lower.contains("spring-boot-starter-web") || lower.contains("spring-boot-starter-webflux");

    if is_spring_web {
        let command = if is_maven {
            "mvn spring-boot:run -Dserver.port=${PORT}".to_string()
        } else {
            "./gradlew bootRun --args=--server.port=${PORT}".to_string()
        };
        return Ok(Some(DetectedCandidate::http(
            service_name.clone(),
            source_key,
            DetectionConfidence::High,
            build_http_service(
                command,
                relative,
                "spring-boot".to_string(),
                service_name,
                "java".to_string(),
            ),
        )));
    }

    let is_quarkus_web = lower.contains("quarkus-resteasy") || lower.contains("quarkus-vertx-web");

    if is_quarkus_web {
        let command = if is_maven {
            "mvn quarkus:dev -Dquarkus.http.port=${PORT}".to_string()
        } else {
            "./gradlew quarkusDev -Dquarkus.http.port=${PORT}".to_string()
        };
        return Ok(Some(DetectedCandidate::http(
            service_name.clone(),
            source_key,
            DetectionConfidence::High,
            build_http_service(
                command,
                relative,
                "quarkus".to_string(),
                service_name,
                "java".to_string(),
            ),
        )));
    }

    Ok(None)
}
