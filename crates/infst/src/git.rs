//! Git integration for automatic score tracking.
//!
//! Provides functions to validate a git repository and commit/push score files.

use std::path::Path;
use std::process::Command;

use anyhow::{Context, Result, bail};
use tracing::{debug, warn};

/// Check if the given path is inside a git repository.
pub fn is_repo(repo_path: &Path) -> Result<bool> {
    let output = Command::new("git")
        .args(["rev-parse", "--is-inside-work-tree"])
        .current_dir(repo_path)
        .output()
        .context("Failed to run git. Is git installed?")?;

    Ok(output.status.success())
}

/// Initialize a new git repository at the given path.
pub fn init_repo(repo_path: &Path) -> Result<()> {
    let output = Command::new("git")
        .args(["init"])
        .current_dir(repo_path)
        .output()
        .context("Failed to run git init")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("git init failed: {}", stderr.trim());
    }

    debug!("Initialized git repository at {}", repo_path.display());
    Ok(())
}

/// Ensure the given path is a git repository, initializing if needed.
pub fn ensure_repo(repo_path: &Path) -> Result<()> {
    if !is_repo(repo_path)? {
        init_repo(repo_path)?;
    }
    Ok(())
}

/// Check if the repository has any remote configured.
fn has_remote(repo_path: &Path) -> Result<bool> {
    let output = Command::new("git")
        .args(["remote"])
        .current_dir(repo_path)
        .output()
        .context("Failed to run git remote")?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    Ok(!stdout.trim().is_empty())
}

/// Stage, commit, and push a file in the given git repository.
///
/// Push failures are logged as warnings but do not cause an error return,
/// since the commit itself succeeded and push will retry on the next play.
pub fn add_commit_push(repo_path: &Path, file: &str, message: &str) -> Result<()> {
    // git add
    let output = Command::new("git")
        .args(["add", file])
        .current_dir(repo_path)
        .output()
        .context("Failed to run git add")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("git add failed: {}", stderr.trim());
    }

    // Check if there are staged changes (avoid empty commits)
    let output = Command::new("git")
        .args(["diff", "--cached", "--quiet"])
        .current_dir(repo_path)
        .output()
        .context("Failed to run git diff")?;

    if output.status.success() {
        debug!("No changes to commit, skipping git commit");
        return Ok(());
    }

    // git commit
    let output = Command::new("git")
        .args(["commit", "-m", message])
        .current_dir(repo_path)
        .output()
        .context("Failed to run git commit")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("git commit failed: {}", stderr.trim());
    }

    debug!("Committed: {}", message);

    // Skip push if no remote is configured
    if !has_remote(repo_path)? {
        debug!("No remote configured, skipping push");
        return Ok(());
    }

    // git push (best-effort)
    let output = Command::new("git")
        .args(["push"])
        .current_dir(repo_path)
        .output()
        .context("Failed to run git push")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        warn!(
            "git push failed (will retry on next play): {}",
            stderr.trim()
        );
    } else {
        debug!("Pushed successfully");
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_is_repo_returns_false_for_non_repo() {
        let tmp = std::env::temp_dir().join("infst-test-not-a-repo");
        let _ = fs::create_dir_all(&tmp);

        assert!(!is_repo(&tmp).unwrap());

        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn test_ensure_repo_initializes_new_repo() {
        let tmp = std::env::temp_dir().join("infst-test-ensure-repo");
        let _ = fs::remove_dir_all(&tmp);
        let _ = fs::create_dir_all(&tmp);

        ensure_repo(&tmp).unwrap();
        assert!(is_repo(&tmp).unwrap());

        let _ = fs::remove_dir_all(&tmp);
    }
}
