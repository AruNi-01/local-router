use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};

use crate::models::Workspace;

use super::utils::{slugify, stable_id};

pub fn project_identity(canonical: &Path) -> String {
    git_output(canonical, &["rev-parse", "--git-common-dir"])
        .and_then(|raw| canonicalize_reported_path(canonical, &raw))
        .unwrap_or_else(|| canonical.to_string_lossy().to_string())
}

pub fn detect_workspaces(project_id: &str, canonical: &Path) -> Vec<Workspace> {
    let Some(raw) = git_output(canonical, &["worktree", "list", "--porcelain"]) else {
        return vec![standalone_workspace(project_id, canonical)];
    };

    let mut workspaces = parse_git_worktrees(&raw)
        .into_iter()
        .map(|candidate| workspace_from_candidate(project_id, candidate))
        .collect::<Vec<_>>();

    workspaces.sort_by(|left, right| left.path.cmp(&right.path));
    workspaces.dedup_by(|left, right| left.path == right.path);

    if workspaces.is_empty() {
        vec![standalone_workspace(project_id, canonical)]
    } else {
        workspaces
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

#[derive(Debug, Clone)]
struct WorktreeCandidate {
    path: PathBuf,
    branch: String,
    detached: bool,
}

fn standalone_workspace(project_id: &str, canonical: &Path) -> Workspace {
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

    Workspace {
        id: stable_id(
            "ws",
            &format!("{project_id}:{}:{name}", canonical.display()),
        ),
        project_id: project_id.to_string(),
        name: name.clone(),
        branch,
        path: canonical.to_string_lossy().to_string(),
        is_active: true,
        slug: slugify(&name),
    }
}

fn workspace_from_candidate(project_id: &str, candidate: WorktreeCandidate) -> Workspace {
    let path_label = candidate
        .path
        .file_name()
        .and_then(|segment| segment.to_str())
        .unwrap_or("workspace");
    let name = if candidate.detached || candidate.branch.trim().is_empty() {
        path_label.to_string()
    } else {
        candidate.branch.clone()
    };

    Workspace {
        id: stable_id(
            "ws",
            &format!("{project_id}:{}:{name}", candidate.path.display()),
        ),
        project_id: project_id.to_string(),
        name: name.clone(),
        branch: candidate.branch,
        path: candidate.path.to_string_lossy().to_string(),
        is_active: true,
        slug: slugify(&name),
    }
}

fn canonicalize_reported_path(base: &Path, raw: &str) -> Option<String> {
    let reported = Path::new(raw);
    let resolved = if reported.is_absolute() {
        reported.to_path_buf()
    } else {
        base.join(reported)
    };
    fs::canonicalize(resolved)
        .ok()
        .map(|path| path.to_string_lossy().to_string())
}

fn parse_git_worktrees(raw: &str) -> Vec<WorktreeCandidate> {
    let mut parsed = Vec::new();
    let mut current: Option<WorktreeCandidate> = None;

    for line in raw.lines() {
        if line.is_empty() {
            if let Some(candidate) = current.take() {
                parsed.push(candidate);
            }
            continue;
        }

        if let Some(path) = line.strip_prefix("worktree ") {
            if let Some(candidate) = current.take() {
                parsed.push(candidate);
            }
            current = Some(WorktreeCandidate {
                path: PathBuf::from(path),
                branch: String::new(),
                detached: false,
            });
            continue;
        }

        let Some(candidate) = current.as_mut() else {
            continue;
        };

        if let Some(branch) = line.strip_prefix("branch ") {
            candidate.branch = branch
                .strip_prefix("refs/heads/")
                .unwrap_or(branch)
                .to_string();
        } else if line == "detached" {
            candidate.detached = true;
        }
    }

    if let Some(candidate) = current {
        parsed.push(candidate);
    }

    parsed
}

#[cfg(test)]
mod tests {
    use super::{parse_git_worktrees, project_identity};
    use std::{fs, path::Path, process::Command};
    use tempfile::tempdir;

    #[test]
    fn parses_git_worktree_porcelain_output() {
        let parsed = parse_git_worktrees(
            "worktree /tmp/demo\nHEAD deadbeef\nbranch refs/heads/main\n\nworktree /tmp/demo-wt\nHEAD cafebabe\ndetached\n",
        );

        assert_eq!(parsed.len(), 2);
        assert_eq!(parsed[0].branch, "main");
        assert!(!parsed[0].detached);
        assert_eq!(parsed[1].branch, "");
        assert!(parsed[1].detached);
        assert_eq!(parsed[1].path.to_string_lossy(), "/tmp/demo-wt");
    }

    #[test]
    fn project_identity_uses_git_common_dir_for_worktrees() {
        let temp = tempdir().unwrap();
        let root = temp.path().join("repo");
        fs::create_dir_all(&root).unwrap();

        run_git(&root, &["init"]);
        fs::write(root.join("README.md"), "demo\n").unwrap();
        run_git(&root, &["add", "README.md"]);
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
            &["worktree", "add", worktree.to_str().unwrap(), "HEAD"],
        );

        assert_eq!(project_identity(&root), project_identity(&worktree));
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
