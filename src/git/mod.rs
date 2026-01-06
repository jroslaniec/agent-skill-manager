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
