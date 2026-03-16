use std::{fs, path::Path};

use anyhow::{Context, Result};

use super::{
    repo::RepoLayout,
    shared::{
        DetectedCandidate, DetectionConfidence, build_http_service, detect_python_entry,
        relative_source_key,
    },
};
use crate::manifest::utils::slugify;

pub fn detect_python_candidate(
    project_root: &Path,
    pyproject_path: &Path,
    repo: &RepoLayout,
) -> Result<Option<DetectedCandidate>> {
    let raw = fs::read_to_string(pyproject_path).context("failed to read pyproject.toml")?;
    let package_dir = pyproject_path.parent().unwrap_or(project_root);
    let (relative, source_key) = relative_source_key(project_root, package_dir);
    let lower = raw.to_ascii_lowercase();
    let service_name = slugify(
        package_dir
            .file_name()
            .and_then(|segment| segment.to_str())
            .unwrap_or("python-app"),
    );

    if relative.is_none()
        && repo.has_python_workspace
        && !lower.contains("fastapi")
        && !lower.contains("flask")
        && !lower.contains("django")
        && !lower.contains("starlette")
    {
        return Ok(Some(DetectedCandidate::hidden(
            service_name,
            source_key,
            super::shared::CandidateKind::WorkspaceRoot,
            DetectionConfidence::Low,
        )));
    }

    if lower.contains("fastapi") || lower.contains("starlette") {
        let detected_entry = detect_python_entry(package_dir);
        let entry = detected_entry
            .clone()
            .map(|entry| format!("{entry}:app"))
            .unwrap_or_else(|| "main:app".to_string());
        let confidence = if detected_entry.is_some() {
            DetectionConfidence::High
        } else {
            DetectionConfidence::Review
        };
        return Ok(Some(DetectedCandidate::http(
            service_name.clone(),
            source_key,
            confidence,
            build_http_service(
                format!("python -m uvicorn {entry} --host ${{HOST}} --port ${{PORT}}"),
                relative,
                if lower.contains("fastapi") {
                    "fastapi".to_string()
                } else {
                    "starlette".to_string()
                },
                service_name,
                "python".to_string(),
            ),
        )));
    }

    if lower.contains("flask") {
        return Ok(Some(DetectedCandidate::http(
            service_name.clone(),
            source_key,
            DetectionConfidence::High,
            build_http_service(
                "flask run --host ${HOST} --port ${PORT}".to_string(),
                relative,
                "flask".to_string(),
                service_name,
                "python".to_string(),
            ),
        )));
    }

    if lower.contains("django") && package_dir.join("manage.py").exists() {
        return Ok(Some(DetectedCandidate::http(
            service_name.clone(),
            source_key,
            DetectionConfidence::High,
            build_http_service(
                "python manage.py runserver ${HOST}:${PORT}".to_string(),
                relative,
                "django".to_string(),
                service_name,
                "python".to_string(),
            ),
        )));
    }

    Ok(None)
}

pub fn detect_requirements_txt_candidate(
    project_root: &Path,
    requirements_path: &Path,
    _repo: &RepoLayout,
) -> Result<Option<DetectedCandidate>> {
    let raw = fs::read_to_string(requirements_path).context("failed to read requirements.txt")?;
    let package_dir = requirements_path.parent().unwrap_or(project_root);
    let (relative, source_key) = relative_source_key(project_root, package_dir);
    let lower = raw.to_ascii_lowercase();
    let service_name = slugify(
        package_dir
            .file_name()
            .and_then(|segment| segment.to_str())
            .unwrap_or("python-app"),
    );

    if lower.contains("fastapi") || lower.contains("starlette") {
        let detected_entry = detect_python_entry(package_dir);
        let entry = detected_entry
            .clone()
            .map(|entry| format!("{entry}:app"))
            .unwrap_or_else(|| "main:app".to_string());
        let confidence = if detected_entry.is_some() {
            DetectionConfidence::High
        } else {
            DetectionConfidence::Review
        };
        return Ok(Some(DetectedCandidate::http(
            service_name.clone(),
            source_key,
            confidence,
            build_http_service(
                format!("python -m uvicorn {entry} --host ${{HOST}} --port ${{PORT}}"),
                relative,
                if lower.contains("fastapi") {
                    "fastapi".to_string()
                } else {
                    "starlette".to_string()
                },
                service_name,
                "python".to_string(),
            ),
        )));
    }

    if lower.contains("flask") {
        return Ok(Some(DetectedCandidate::http(
            service_name.clone(),
            source_key,
            DetectionConfidence::High,
            build_http_service(
                "flask run --host ${HOST} --port ${PORT}".to_string(),
                relative,
                "flask".to_string(),
                service_name,
                "python".to_string(),
            ),
        )));
    }

    if lower.contains("django") && package_dir.join("manage.py").exists() {
        return Ok(Some(DetectedCandidate::http(
            service_name.clone(),
            source_key,
            DetectionConfidence::High,
            build_http_service(
                "python manage.py runserver ${HOST}:${PORT}".to_string(),
                relative,
                "django".to_string(),
                service_name,
                "python".to_string(),
            ),
        )));
    }

    Ok(None)
}

pub fn detect_django_candidate(project_root: &Path, manage_py_path: &Path) -> DetectedCandidate {
    let package_dir = manage_py_path.parent().unwrap_or(project_root);
    let (relative, source_key) = relative_source_key(project_root, package_dir);
    let service_name = slugify(
        package_dir
            .file_name()
            .and_then(|segment| segment.to_str())
            .unwrap_or("django"),
    );
    DetectedCandidate::http(
        service_name.clone(),
        source_key,
        DetectionConfidence::High,
        build_http_service(
            "python manage.py runserver ${HOST}:${PORT}".to_string(),
            relative,
            "django".to_string(),
            service_name,
            "python".to_string(),
        ),
    )
}
