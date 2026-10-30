//! Reads the checked-out git branch of a folder straight from `.git/HEAD`, so git
//! itself doesn't need to be installed.

use std::fs;
use std::path::{Path, PathBuf};

/// Current branch of the repository containing `dir`. A detached HEAD yields the
/// short commit hash. `None` if `dir` is not inside a git repository.
pub fn current_branch(dir: &Path) -> Option<String> {
    let head = fs::read_to_string(git_dir(dir)?.join("HEAD")).ok()?;
    let head = head.trim();
    match head.strip_prefix("ref:") {
        Some(r) => {
            let r = r.trim();
            Some(r.strip_prefix("refs/heads/").unwrap_or(r).to_string())
        }
        None if head.len() >= 7 && head.chars().all(|c| c.is_ascii_hexdigit()) => Some(head[..7].to_string()),
        None => None,
    }
}

/// Top-level folder of the repository containing `dir` (the one holding `.git`).
pub fn repo_root(dir: &Path) -> Option<&Path> {
    dir.ancestors().find(|d| d.join(".git").exists())
}

/// Finds the git directory for `dir` or its nearest ancestor. Handles worktrees and
/// submodules, where `.git` is a file containing `gitdir: <path>`.
fn git_dir(dir: &Path) -> Option<PathBuf> {
    for d in dir.ancestors() {
        let dot_git = d.join(".git");
        if dot_git.is_dir() {
            return Some(dot_git);
        }
        if dot_git.is_file() {
            let content = fs::read_to_string(&dot_git).ok()?;
            let target = content.trim().strip_prefix("gitdir:")?.trim();
            return Some(d.join(target)); // join keeps absolute targets as they are
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("hivemind-git-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn reads_branch_from_head() {
        let dir = temp_dir("branch");
        fs::create_dir_all(dir.join(".git")).unwrap();
        fs::write(dir.join(".git/HEAD"), "ref: refs/heads/feature/x\n").unwrap();
        fs::create_dir_all(dir.join("sub/deeper")).unwrap();
        assert_eq!(current_branch(&dir).as_deref(), Some("feature/x"));
        assert_eq!(current_branch(&dir.join("sub/deeper")).as_deref(), Some("feature/x"));
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn detached_head_gives_short_hash() {
        let dir = temp_dir("detached");
        fs::create_dir_all(dir.join(".git")).unwrap();
        fs::write(dir.join(".git/HEAD"), "2526c52a1b2c3d4e5f60718293a4b5c6d7e8f901\n").unwrap();
        assert_eq!(current_branch(&dir).as_deref(), Some("2526c52"));
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn follows_gitdir_file() {
        let dir = temp_dir("worktree");
        fs::create_dir_all(dir.join("main/.git/worktrees/wt")).unwrap();
        fs::write(dir.join("main/.git/worktrees/wt/HEAD"), "ref: refs/heads/wt-branch\n").unwrap();
        fs::create_dir_all(dir.join("wt")).unwrap();
        fs::write(dir.join("wt/.git"), "gitdir: ../main/.git/worktrees/wt\n").unwrap();
        assert_eq!(current_branch(&dir.join("wt")).as_deref(), Some("wt-branch"));
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn no_repo_gives_none() {
        // Only meaningful if the temp dir itself is not inside a repository.
        let dir = temp_dir("none");
        if git_dir(&std::env::temp_dir()).is_none() {
            assert_eq!(current_branch(&dir), None);
        }
        fs::remove_dir_all(&dir).unwrap();
    }
}
