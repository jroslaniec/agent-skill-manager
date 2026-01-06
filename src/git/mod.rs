use anyhow::{Context, Result};
use indicatif::{ProgressBar, ProgressStyle};
use std::path::Path;
use std::process::Command;

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
        .context("Failed to execute git command")?;

    if output.status.success() {
        pb.finish_with_message(format!("Cloned {}", url));
        Ok(())
    } else {
        pb.finish_with_message(format!("Failed to clone {}", url));
        Err(anyhow::anyhow!(
            "Git clone failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ))
    }
}

/// Get the current commit SHA (first 12 characters)
pub fn get_current_sha(repo_path: &Path) -> Result<String> {
    let output = Command::new("git")
        .arg("rev-parse")
        .arg("--short=12")
        .arg("HEAD")
        .current_dir(repo_path)
        .output()
        .context("Failed to execute git rev-parse")?;

    if output.status.success() {
        let sha = String::from_utf8_lossy(&output.stdout).trim().to_string();
        Ok(sha)
    } else {
        Err(anyhow::anyhow!(
            "Failed to get current SHA: {}",
            String::from_utf8_lossy(&output.stderr)
        ))
    }
}

/// Checkout a specific commit SHA
pub fn checkout_sha(repo_path: &Path, sha: &str) -> Result<()> {
    // First, fetch to ensure we have all commits
    let fetch_output = Command::new("git")
        .arg("fetch")
        .arg("origin")
        .arg("--quiet")
        .current_dir(repo_path)
        .output()
        .context("Failed to execute git fetch")?;

    if !fetch_output.status.success() {
        return Err(anyhow::anyhow!(
            "Git fetch failed: {}",
            String::from_utf8_lossy(&fetch_output.stderr)
        ));
    }

    // Checkout the specific SHA
    let checkout_output = Command::new("git")
        .arg("checkout")
        .arg("--quiet")
        .arg(sha)
        .current_dir(repo_path)
        .output()
        .context("Failed to execute git checkout")?;

    if checkout_output.status.success() {
        Ok(())
    } else {
        Err(anyhow::anyhow!(
            "Git checkout failed: {}",
            String::from_utf8_lossy(&checkout_output.stderr)
        ))
    }
}

/// Pull the latest changes and return the new SHA
pub fn pull_to_latest(repo_path: &Path) -> Result<String> {
    // First fetch
    let fetch_output = Command::new("git")
        .arg("fetch")
        .arg("origin")
        .arg("--quiet")
        .current_dir(repo_path)
        .output()
        .context("Failed to execute git fetch")?;

    if !fetch_output.status.success() {
        return Err(anyhow::anyhow!(
            "Git fetch failed: {}",
            String::from_utf8_lossy(&fetch_output.stderr)
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
        .context("Failed to checkout branch")?;

    if !checkout_output.status.success() {
        return Err(anyhow::anyhow!(
            "Git checkout failed: {}",
            String::from_utf8_lossy(&checkout_output.stderr)
        ));
    }

    // Then pull with fast-forward only
    let pull_output = Command::new("git")
        .arg("pull")
        .arg("--ff-only")
        .arg("--quiet")
        .current_dir(repo_path)
        .output()
        .context("Failed to execute git pull")?;

    if !pull_output.status.success() {
        return Err(anyhow::anyhow!(
            "Git pull failed: {}",
            String::from_utf8_lossy(&pull_output.stderr)
        ));
    }

    // Get the new SHA
    get_current_sha(repo_path)
}
