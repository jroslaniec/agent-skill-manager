use anyhow::{Context, Result};
use console::style;
use std::io::{self, Write};

use crate::config::{Config, ConfigLock};
use crate::paths;

pub fn purge(force: bool) -> Result<()> {
    let lock = ConfigLock::acquire()?;
    let config = lock.read_config()?;

    // Show what will be deleted and get confirmation
    if !force {
        let repo_count = config.repositories.len();
        let skill_count = config.skills.len();
        let enabled_count = config.skills.values().filter(|s| s.enabled).count();

        if repo_count == 0 && skill_count == 0 {
            println!("Nothing to purge.");
            return Ok(());
        }

        println!("{} This will:", style("Warning:").yellow());
        println!("  - Delete {} cached repositories", repo_count);
        println!("  - Remove {} skills ({} enabled)", skill_count, enabled_count);
        println!("  - Clear all configuration");
        print!("Are you sure you want to continue? (yes/no): ");
        io::stdout().flush()?;

        let mut response = String::new();
        io::stdin().read_line(&mut response)?;

        if response.trim().to_lowercase() != "yes" {
            println!("Cancelled.");
            return Ok(());
        }
    }

    let repo_count = config.repositories.len();
    let skill_count = config.skills.len();

    // 1. Remove all skill symlinks
    for skill_name in config.skills.keys() {
        let skill_link_path = paths::claude_skills_dir()?.join(skill_name);
        if skill_link_path.exists() {
            std::fs::remove_file(&skill_link_path)
                .context("Failed to remove skill symlink")?;
        }
    }

    // 2. Delete git cache directory
    let git_cache = paths::git_cache_dir()?;
    if git_cache.exists() {
        std::fs::remove_dir_all(&git_cache)
            .context("Failed to delete git cache directory")?;
    }

    // 3. Clear config file
    lock.write_config(&Config::new())?;

    println!(
        "Purged {} repositories and {} skills",
        style(repo_count).cyan(),
        style(skill_count).cyan()
    );

    Ok(())
}
