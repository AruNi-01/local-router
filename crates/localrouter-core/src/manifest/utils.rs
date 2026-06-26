use std::path::{Path, PathBuf};

use sha1::{Digest, Sha1};

pub fn stable_id(prefix: &str, input: &str) -> String {
    let mut hasher = Sha1::new();
    hasher.update(input.as_bytes());
    let digest = hasher.finalize();
    format!("{prefix}-{:x}", digest)[..16].to_string()
}

pub fn slugify(input: &str) -> String {
    let mut out = String::new();
    let mut last_dash = false;
    for ch in input.chars() {
        let mapped = if ch.is_ascii_alphanumeric() {
            Some(ch.to_ascii_lowercase())
        } else if matches!(ch, '-' | '_' | '/' | ' ' | '.') {
            Some('-')
        } else {
            None
        };
        if let Some(mapped) = mapped {
            if mapped == '-' {
                if !out.is_empty() && !last_dash {
                    out.push('-');
                }
                last_dash = true;
            } else {
                out.push(mapped);
                last_dash = false;
            }
        }
    }
    out.trim_matches('-').to_string()
}

pub fn normalize_service_env_name(service_name: &str) -> String {
    let mut output = String::new();
    let mut previous_was_separator = false;

    for ch in service_name.chars() {
        if ch.is_ascii_alphanumeric() {
            output.push(ch.to_ascii_uppercase());
            previous_was_separator = false;
        } else if !previous_was_separator && !output.is_empty() {
            output.push('_');
            previous_was_separator = true;
        }
    }

    output.trim_matches('_').to_string()
}

pub fn resolve_service_cwd(project_path: &str, cwd: Option<&str>) -> PathBuf {
    cwd.map(|rel| Path::new(project_path).join(rel))
        .unwrap_or_else(|| PathBuf::from(project_path))
}

pub fn relative_cwd(project_root: &Path, service_dir: &Path) -> Option<String> {
    let stripped = service_dir.strip_prefix(project_root).ok()?;
    if stripped.as_os_str().is_empty() {
        None
    } else {
        Some(stripped.to_string_lossy().to_string())
    }
}

pub fn is_valid_dns_label(label: &str) -> bool {
    !label.is_empty()
        && label.len() <= 63
        && label
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '-')
        && !label.starts_with('-')
        && !label.ends_with('-')
}
