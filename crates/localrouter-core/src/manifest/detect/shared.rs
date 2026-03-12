use std::{collections::BTreeMap, fs, path::Path};

use anyhow::Result;
use serde_json::Value;
use walkdir::WalkDir;

use crate::manifest::{
    schema::ManifestService,
    utils::{relative_cwd, slugify},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CandidateKind {
    HttpService,
    RunnableNonHttp,
    Library,
    Tooling,
    WorkspaceRoot,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum DetectionConfidence {
    Low,
    Review,
    High,
}

#[derive(Debug, Clone)]
pub struct DetectedCandidate {
    pub name: String,
    pub source_key: String,
    pub kind: CandidateKind,
    pub confidence: DetectionConfidence,
    pub service: Option<ManifestService>,
}

impl DetectedCandidate {
    pub fn http(
        name: String,
        source_key: String,
        confidence: DetectionConfidence,
        service: ManifestService,
    ) -> Self {
        Self {
            name,
            source_key,
            kind: CandidateKind::HttpService,
            confidence,
            service: Some(service),
        }
    }

    pub fn non_http(
        name: String,
        source_key: String,
        confidence: DetectionConfidence,
        service: ManifestService,
    ) -> Self {
        Self {
            name,
            source_key,
            kind: CandidateKind::RunnableNonHttp,
            confidence,
            service: Some(service),
        }
    }

    pub fn hidden(
        name: String,
        source_key: String,
        kind: CandidateKind,
        confidence: DetectionConfidence,
    ) -> Self {
        Self {
            name,
            source_key,
            kind,
            confidence,
            service: None,
        }
    }

    pub fn into_manifest_service(self) -> Option<(String, ManifestService)> {
        if self.confidence == DetectionConfidence::Low {
            return None;
        }
        match self.kind {
            CandidateKind::HttpService | CandidateKind::RunnableNonHttp => {
                let mut service = self.service?;
                if self.confidence == DetectionConfidence::Review {
                    service.disabled = Some(true);
                }
                Some((self.name, service))
            }
            CandidateKind::Library | CandidateKind::Tooling | CandidateKind::WorkspaceRoot => None,
        }
    }
}

pub fn dedupe_candidates(candidates: Vec<DetectedCandidate>) -> Vec<DetectedCandidate> {
    let mut deduped = BTreeMap::new();
    for candidate in candidates {
        match deduped.entry(candidate.source_key.clone()) {
            std::collections::btree_map::Entry::Vacant(entry) => {
                entry.insert(candidate);
            }
            std::collections::btree_map::Entry::Occupied(mut entry) => {
                if candidate_rank(&candidate) > candidate_rank(entry.get()) {
                    entry.insert(candidate);
                }
            }
        }
    }
    deduped.into_values().collect()
}

pub fn dependency_haystack(parsed: &Value) -> String {
    let mut names = Vec::new();
    for section in ["dependencies", "devDependencies", "peerDependencies"] {
        if let Some(deps) = parsed.get(section).and_then(Value::as_object) {
            names.extend(deps.keys().cloned());
        }
    }
    names.join(" ")
}

pub fn normalize_package_name(package_name: &str, dir_name: &str) -> String {
    let raw = package_name
        .rsplit('/')
        .next()
        .filter(|segment| !segment.is_empty())
        .unwrap_or(dir_name);
    let slug = slugify(raw);
    if slug.is_empty() {
        slugify(dir_name)
    } else {
        slug
    }
}

pub fn pick_node_script(scripts: &serde_json::Map<String, Value>) -> Option<(String, String)> {
    for key in ["dev", "start", "serve"] {
        if let Some(script) = scripts.get(key).and_then(Value::as_str) {
            return Some((key.to_string(), script.to_string()));
        }
    }
    None
}

pub fn is_library_candidate(relative: Option<&str>, service_name: &str, lower: &str) -> bool {
    if lower.contains("storybook") || lower.contains("vitest") || lower.contains("jest") {
        return true;
    }

    let rel = relative.unwrap_or_default();
    rel.starts_with("packages/")
        || rel.starts_with("crates/")
        || rel.starts_with("libs/")
        || rel.starts_with("lib/")
        || matches!(
            service_name,
            "ui" | "shared" | "config" | "types" | "utils" | "eslint" | "tsconfig"
        )
}

pub fn build_http_service(
    command: String,
    cwd: Option<String>,
    adapter: String,
    route: String,
    language: String,
) -> ManifestService {
    ManifestService {
        command,
        cwd,
        protocol: Some("http".to_string()),
        adapter: Some(adapter),
        route: Some(route),
        healthcheck: Some("http://127.0.0.1:${PORT}".to_string()),
        env: BTreeMap::new(),
        depends_on: Vec::new(),
        disabled: Some(false),
        language: Some(language),
    }
}

pub fn build_non_http_service(
    command: String,
    cwd: Option<String>,
    adapter: String,
    language: String,
) -> ManifestService {
    ManifestService {
        command,
        cwd,
        protocol: Some("none".to_string()),
        adapter: Some(adapter),
        route: Some("none".to_string()),
        healthcheck: Some(String::new()),
        env: BTreeMap::new(),
        depends_on: Vec::new(),
        disabled: Some(false),
        language: Some(language),
    }
}

pub fn detect_python_entry(dir: &Path) -> Option<String> {
    for candidate in ["main.py", "app.py", "src/main.py", "src/app.py"] {
        let path = dir.join(candidate);
        if path.exists() {
            let stem = Path::new(candidate)
                .file_stem()
                .and_then(|segment| segment.to_str())?;
            return Some(stem.to_string());
        }
    }
    None
}

pub fn parse_go_module_name(raw: &str) -> Option<String> {
    raw.lines().find_map(|line| {
        let trimmed = line.trim();
        trimmed
            .strip_prefix("module ")
            .map(|module| module.trim().to_string())
    })
}

pub fn go_source_haystack(dir: &Path) -> String {
    WalkDir::new(dir)
        .max_depth(3)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|entry| entry.path().extension().and_then(|ext| ext.to_str()) == Some("go"))
        .filter_map(|entry| fs::read_to_string(entry.path()).ok())
        .collect::<Vec<_>>()
        .join("\n")
}

pub fn parse_cargo_package_name(raw: &str) -> Option<String> {
    let mut in_package = false;
    for line in raw.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') {
            in_package = trimmed == "[package]";
            continue;
        }
        if in_package && trimmed.starts_with("name") {
            let (_, value) = trimmed.split_once('=')?;
            return Some(value.trim().trim_matches('"').to_string());
        }
    }
    None
}

pub fn is_rust_http_service(lower: &str) -> bool {
    lower.contains("axum")
        || lower.contains("actix-web")
        || lower.contains("warp")
        || lower.contains("rocket")
        || lower.contains("poem")
        || lower.contains("tide")
}

pub fn relative_source_key(project_root: &Path, package_dir: &Path) -> (Option<String>, String) {
    let relative = relative_cwd(project_root, package_dir);
    let source_key = relative.clone().unwrap_or_else(|| ".".to_string());
    (relative, source_key)
}

fn candidate_rank(candidate: &DetectedCandidate) -> (u8, u8, u8) {
    let kind_rank = match candidate.kind {
        CandidateKind::HttpService => 5,
        CandidateKind::RunnableNonHttp => 4,
        CandidateKind::WorkspaceRoot => 3,
        CandidateKind::Library => 2,
        CandidateKind::Tooling => 1,
    };
    let confidence_rank = match candidate.confidence {
        DetectionConfidence::High => 3,
        DetectionConfidence::Review => 2,
        DetectionConfidence::Low => 1,
    };
    let has_service = u8::from(candidate.service.is_some());
    (kind_rank, confidence_rank, has_service)
}
