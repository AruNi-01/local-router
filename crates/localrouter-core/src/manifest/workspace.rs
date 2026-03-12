use std::{path::Path, process::Command};

use crate::models::Workspace;

use super::utils::{slugify, stable_id};

pub fn detect_workspace(project_id: &str, canonical: &Path) -> Workspace {
    let branch =
        git_output(canonical, &["branch", "--show-current"]).unwrap_or_else(|| "main".to_string());
    let path_label = canonical
        .file_name()
        .and_then(|segment| segment.to_str())
        .unwrap_or("workspace");
    let name = if branch.trim().is_empty() {
        path_label.to_string()
    } else {
        branch.clone()
    };
    let slug = slugify(&name);

    Workspace {
        id: stable_id("ws", &format!("{project_id}:{canonical:?}:{name}")),
        project_id: project_id.to_string(),
        name,
        branch,
        path: canonical.to_string_lossy().to_string(),
        is_active: true,
        slug,
    }
}

fn git_output(dir: &Path, args: &[&str]) -> Option<String> {
    Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .output()
        .ok()
        .filter(|output| output.status.success())
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .map(|stdout| stdout.trim().to_string())
        .filter(|stdout| !stdout.is_empty())
}
