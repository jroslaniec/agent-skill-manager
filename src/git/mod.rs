use anyhow::{Context, Result};
use indicatif::{ProgressBar, ProgressStyle};
use std::path::Path;
use std::process::Command;

/// Clone a git repository from any source (HTTPS, SSH, or local path)
///
/// # Arguments
/// * `url` - The git URL or local path to clone from. Supports:
///   - HTTPS URLs: `https://github.com/owner/repo.git`
///   - SSH URLs: `git@github.com:owner/repo.git`
///   - Local paths: `/path/to/repo`, `~/projects/repo`
/// * `target_path` - The directory to clone into
///
/// # Errors
/// Returns an error if git clone fails, with the original git error output included
pub fn clone_repo(url: &str, target_path: &Path) -> Result<()> {
    let pb = ProgressBar::new_spinner();
    pb.set_style(
        ProgressStyle::default_spinner()
            .template("{spinner:.green} {msg}")
            .unwrap(),
    );
    pb.set_message(format!("Cloning {}...", url));

    let output = Command::new("git")
        .arg("clone")
        .arg("--quiet")
        .arg(url)
        .arg(target_path)
        .output()
        .context("Failed to execute git clone command")?;

    if output.status.success() {
        pb.finish_with_message(format!("Cloned {}", url));
        Ok(())
    } else {
        pb.finish_with_message(format!("Failed to clone {}", url));
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(anyhow::anyhow!(
            "Git clone failed for '{}'\n\nGit error:\n{}",
            url,
            stderr.trim()
        ))
    }
}

/// Get the current commit SHA (first 12 characters)
///
/// # Errors
/// Returns an error if the git command fails, with the original git error output included
pub fn get_current_sha(repo_path: &Path) -> Result<String> {
    let output = Command::new("git")
        .arg("rev-parse")
        .arg("--short=12")
        .arg("HEAD")
        .current_dir(repo_path)
        .output()
        .context("Failed to execute git rev-parse command")?;

    if output.status.success() {
        let sha = String::from_utf8_lossy(&output.stdout).trim().to_string();
        Ok(sha)
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(anyhow::anyhow!(
            "Failed to get current SHA in '{}'\n\nGit error:\n{}",
            repo_path.display(),
            stderr.trim()
        ))
    }
}

/// Checkout a specific commit SHA or tag
///
/// This function first fetches from origin to ensure the commit exists locally,
/// then checks out the specified reference.
///
/// # Errors
/// Returns an error if git fetch or checkout fails, with the original git error output included
pub fn checkout_sha(repo_path: &Path, sha: &str) -> Result<()> {
    // First, fetch to ensure we have all commits
    let fetch_output = Command::new("git")
        .arg("fetch")
        .arg("origin")
        .arg("--quiet")
        .current_dir(repo_path)
        .output()
        .context("Failed to execute git fetch command")?;

    if !fetch_output.status.success() {
        let stderr = String::from_utf8_lossy(&fetch_output.stderr);
        return Err(anyhow::anyhow!(
            "Git fetch failed in '{}'\n\nGit error:\n{}",
            repo_path.display(),
            stderr.trim()
        ));
    }

    // Checkout the specific SHA
    let checkout_output = Command::new("git")
        .arg("checkout")
        .arg("--quiet")
        .arg(sha)
        .current_dir(repo_path)
        .output()
        .context("Failed to execute git checkout command")?;

    if checkout_output.status.success() {
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&checkout_output.stderr);
        Err(anyhow::anyhow!(
            "Git checkout of '{}' failed in '{}'\n\nGit error:\n{}",
            sha,
            repo_path.display(),
            stderr.trim()
        ))
    }
}

/// Pull the latest changes and return the new SHA
///
/// This function works with any git source (HTTPS, SSH, local paths).
/// It fetches from origin, determines the default branch, and pulls changes.
///
/// # Errors
/// Returns an error if any git operation fails, with the original git error output included
pub fn pull_to_latest(repo_path: &Path) -> Result<String> {
    // First fetch
    let fetch_output = Command::new("git")
        .arg("fetch")
        .arg("origin")
        .arg("--quiet")
        .current_dir(repo_path)
        .output()
        .context("Failed to execute git fetch command")?;

    if !fetch_output.status.success() {
        let stderr = String::from_utf8_lossy(&fetch_output.stderr);
        return Err(anyhow::anyhow!(
            "Git fetch failed in '{}'\n\nGit error:\n{}",
            repo_path.display(),
            stderr.trim()
        ));
    }

    // Determine the default branch (main or master)
    let branch_output = Command::new("git")
        .arg("symbolic-ref")
        .arg("refs/remotes/origin/HEAD")
        .arg("--short")
        .current_dir(repo_path)
        .output()
        .context("Failed to determine default branch")?;

    let default_branch = if branch_output.status.success() {
        String::from_utf8_lossy(&branch_output.stdout)
            .trim()
            .trim_start_matches("origin/")
            .to_string()
    } else {
        // Fallback: try main, then master
        "main".to_string()
    };

    // Checkout the default branch
    let checkout_output = Command::new("git")
        .arg("checkout")
        .arg("--quiet")
        .arg(&default_branch)
        .current_dir(repo_path)
        .output()
        .context("Failed to execute git checkout command")?;

    if !checkout_output.status.success() {
        let stderr = String::from_utf8_lossy(&checkout_output.stderr);
        return Err(anyhow::anyhow!(
            "Git checkout of branch '{}' failed in '{}'\n\nGit error:\n{}",
            default_branch,
            repo_path.display(),
            stderr.trim()
        ));
    }

    // Then pull with fast-forward only
    let pull_output = Command::new("git")
        .arg("pull")
        .arg("--ff-only")
        .arg("--quiet")
        .current_dir(repo_path)
        .output()
        .context("Failed to execute git pull command")?;

    if !pull_output.status.success() {
        let stderr = String::from_utf8_lossy(&pull_output.stderr);
        return Err(anyhow::anyhow!(
            "Git pull failed in '{}'\n\nGit error:\n{}",
            repo_path.display(),
            stderr.trim()
        ));
    }

    // Get the new SHA
    get_current_sha(repo_path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    /// Helper to create a temporary git repository for testing
    fn create_test_repo() -> (TempDir, std::path::PathBuf) {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let repo_path = temp_dir.path().to_path_buf();

        // Initialize git repo
        Command::new("git")
            .args(["init", "-b", "main"])
            .current_dir(&repo_path)
            .output()
            .expect("Failed to init git repo");

        // Configure git user for commits
        Command::new("git")
            .args(["config", "user.email", "test@example.com"])
            .current_dir(&repo_path)
            .output()
            .expect("Failed to configure git email");

        Command::new("git")
            .args(["config", "user.name", "Test User"])
            .current_dir(&repo_path)
            .output()
            .expect("Failed to configure git name");

        // Create initial file and commit
        fs::write(repo_path.join("README.md"), "# Test Repo\n").expect("Failed to write file");

        Command::new("git")
            .args(["add", "."])
            .current_dir(&repo_path)
            .output()
            .expect("Failed to git add");

        Command::new("git")
            .args(["commit", "-m", "Initial commit"])
            .current_dir(&repo_path)
            .output()
            .expect("Failed to git commit");

        (temp_dir, repo_path)
    }

    #[test]
    fn test_get_current_sha() {
        let (_temp_dir, repo_path) = create_test_repo();

        let sha = get_current_sha(&repo_path).expect("Failed to get SHA");

        // SHA should be 12 characters (short format)
        assert_eq!(sha.len(), 12);
        // SHA should be valid hex
        assert!(sha.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn test_clone_repo_from_local_path() {
        let (_source_dir, source_path) = create_test_repo();
        let target_dir = TempDir::new().expect("Failed to create target dir");
        let target_path = target_dir.path().join("cloned-repo");

        // Clone from local path
        clone_repo(source_path.to_str().unwrap(), &target_path)
            .expect("Failed to clone from local path");

        // Verify clone succeeded
        assert!(target_path.exists());
        assert!(target_path.join(".git").exists());
        assert!(target_path.join("README.md").exists());
    }

    #[test]
    fn test_clone_repo_invalid_url_includes_git_error() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let target_path = temp_dir.path().join("should-not-exist");

        let result = clone_repo(
            "https://invalid.example.com/nonexistent/repo.git",
            &target_path,
        );

        assert!(result.is_err());
        let error = result.unwrap_err().to_string();
        // Error should include the URL and git's error message
        assert!(error.contains("invalid.example.com"));
        assert!(error.contains("Git error:"));
    }

    #[test]
    fn test_get_current_sha_non_git_dir_includes_path_in_error() {
        // Create a directory that exists but is not a git repo
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let result = get_current_sha(temp_dir.path());

        assert!(result.is_err());
        let error = result.unwrap_err().to_string();
        // Error should include the path and git's error message
        assert!(error.contains(temp_dir.path().to_str().unwrap()));
        assert!(error.contains("Git error:"));
    }

    #[test]
    fn test_pull_to_latest_from_local_clone() {
        // Create source repo
        let (_source_dir, source_path) = create_test_repo();

        // Clone it to create a repo with an origin
        let clone_dir = TempDir::new().expect("Failed to create clone dir");
        let clone_path = clone_dir.path().join("clone");
        clone_repo(source_path.to_str().unwrap(), &clone_path).expect("Failed to clone");

        // Get initial SHA
        let initial_sha = get_current_sha(&clone_path).expect("Failed to get SHA");

        // Add a commit to the source
        fs::write(source_path.join("new-file.txt"), "New content\n").expect("Failed to write file");
        Command::new("git")
            .args(["add", "."])
            .current_dir(&source_path)
            .output()
            .expect("Failed to git add");
        Command::new("git")
            .args(["commit", "-m", "Second commit"])
            .current_dir(&source_path)
            .output()
            .expect("Failed to git commit");

        // Pull latest in clone
        let new_sha = pull_to_latest(&clone_path).expect("Failed to pull");

        // SHA should be different
        assert_ne!(initial_sha, new_sha);
        // New file should exist
        assert!(clone_path.join("new-file.txt").exists());
    }

    #[test]
    fn test_checkout_sha() {
        // Create source repo with two commits
        let (_source_dir, source_path) = create_test_repo();

        let first_sha = get_current_sha(&source_path).expect("Failed to get first SHA");

        // Add second commit
        fs::write(source_path.join("second.txt"), "Second\n").expect("Failed to write file");
        Command::new("git")
            .args(["add", "."])
            .current_dir(&source_path)
            .output()
            .expect("Failed to git add");
        Command::new("git")
            .args(["commit", "-m", "Second commit"])
            .current_dir(&source_path)
            .output()
            .expect("Failed to git commit");

        let second_sha = get_current_sha(&source_path).expect("Failed to get second SHA");

        // Clone it
        let clone_dir = TempDir::new().expect("Failed to create clone dir");
        let clone_path = clone_dir.path().join("clone");
        clone_repo(source_path.to_str().unwrap(), &clone_path).expect("Failed to clone");

        // Should be at second commit
        assert_eq!(
            get_current_sha(&clone_path).expect("Failed to get SHA"),
            second_sha
        );
        assert!(clone_path.join("second.txt").exists());

        // Checkout first commit
        checkout_sha(&clone_path, &first_sha).expect("Failed to checkout");

        // Should be at first commit
        assert_eq!(
            get_current_sha(&clone_path).expect("Failed to get SHA"),
            first_sha
        );
        // second.txt should not exist in first commit
        assert!(!clone_path.join("second.txt").exists());
    }

    #[test]
    fn test_checkout_invalid_sha_includes_error_details() {
        let (_temp_dir, repo_path) = create_test_repo();

        // Clone it to create a repo with an origin
        let clone_dir = TempDir::new().expect("Failed to create clone dir");
        let clone_path = clone_dir.path().join("clone");
        clone_repo(repo_path.to_str().unwrap(), &clone_path).expect("Failed to clone");

        let result = checkout_sha(&clone_path, "nonexistent123456");

        assert!(result.is_err());
        let error = result.unwrap_err().to_string();
        // Error should include SHA and path
        assert!(error.contains("nonexistent123456"));
        assert!(error.contains("Git error:"));
    }
}
