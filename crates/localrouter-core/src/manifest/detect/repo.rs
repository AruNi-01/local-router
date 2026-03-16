use std::{
    fs,
    path::{Path, PathBuf},
};

use serde_json::Value;
use walkdir::WalkDir;

#[derive(Debug, Clone, Copy)]
pub enum PackageManager {
    Bun,
    Pnpm,
    Yarn,
    Npm,
}

impl PackageManager {
    pub fn run_script(self, script_name: &str) -> String {
        match self {
            Self::Bun => format!("bun run {script_name}"),
            Self::Pnpm => format!("pnpm run {script_name}"),
            Self::Yarn => format!("yarn {script_name}"),
            Self::Npm => format!("npm run {script_name}"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct RepoLayout {
    pub package_manager: PackageManager,
    pub has_node_workspace: bool,
    pub has_rust_workspace: bool,
    pub has_go_workspace: bool,
    pub has_python_workspace: bool,
    pub has_gradle_workspace: bool,
    pub has_maven_workspace: bool,
    pub _has_dotnet_workspace: bool,
}

pub fn classify_repo(project_root: &Path) -> RepoLayout {
    let root_package = read_json(project_root.join("package.json"));
    let has_root_workspaces = root_package
        .as_ref()
        .and_then(|parsed| parsed.get("workspaces"))
        .is_some();
    let root_cargo = fs::read_to_string(project_root.join("Cargo.toml")).unwrap_or_default();
    let root_pyproject =
        fs::read_to_string(project_root.join("pyproject.toml")).unwrap_or_default();

    RepoLayout {
        package_manager: detect_package_manager(project_root),
        has_node_workspace: project_root.join("pnpm-workspace.yaml").exists()
            || project_root.join("turbo.json").exists()
            || project_root.join("nx.json").exists()
            || has_root_workspaces,
        has_rust_workspace: root_cargo.contains("[workspace]"),
        has_go_workspace: project_root.join("go.work").exists()
            || count_named_files(project_root, "go.mod", 4) > 1,
        has_python_workspace: root_pyproject.contains("[tool.uv.workspace]")
            || root_pyproject.contains("[tool.poetry.workspace]")
            || root_pyproject.contains("[tool.pdm]")
            || root_pyproject.contains("[tool.hatch")
            || count_named_files(project_root, "pyproject.toml", 4) > 1,
        has_gradle_workspace: project_root.join("settings.gradle").exists()
            || project_root.join("settings.gradle.kts").exists(),
        has_maven_workspace: fs::read_to_string(project_root.join("pom.xml"))
            .map(|raw| raw.contains("<modules>"))
            .unwrap_or(false),
        _has_dotnet_workspace: count_files_with_extension(project_root, "sln", 2) > 0,
    }
}

fn detect_package_manager(project_root: &Path) -> PackageManager {
    if project_root.join("bun.lock").exists() || project_root.join("bun.lockb").exists() {
        PackageManager::Bun
    } else if project_root.join("pnpm-lock.yaml").exists() {
        PackageManager::Pnpm
    } else if project_root.join("yarn.lock").exists() {
        PackageManager::Yarn
    } else {
        PackageManager::Npm
    }
}

fn read_json(path: PathBuf) -> Option<Value> {
    fs::read_to_string(path)
        .ok()
        .and_then(|raw| serde_json::from_str(&raw).ok())
}

fn count_named_files(project_root: &Path, file_name: &str, max_depth: usize) -> usize {
    WalkDir::new(project_root)
        .max_depth(max_depth)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|entry| entry.file_name() == file_name)
        .count()
}

fn count_files_with_extension(project_root: &Path, extension: &str, max_depth: usize) -> usize {
    WalkDir::new(project_root)
        .max_depth(max_depth)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|entry| entry.path().extension().and_then(|ext| ext.to_str()) == Some(extension))
        .count()
}
