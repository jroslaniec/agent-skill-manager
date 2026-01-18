use anyhow::{Context, Result};
use console::style;
use std::io::{self, Write};
use std::path::PathBuf;

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
    let agent_count = config.agents.len();

    // 1. Remove all skill symlinks from all integrations
    for skill_name in config.skills.keys() {
        for (_int_name, integration) in &config.integrations {
            if let Some(ref skills_dir_str) = integration.skills_dir {
                let skills_dir = PathBuf::from(skills_dir_str);
                let link_path = skills_dir.join(skill_name);
                if link_path.exists() {
                    std::fs::remove_file(&link_path).ok();
                }
            }
        }
    }

    // 1b. Remove all agent symlinks from all integrations
    for agent_name in config.agents.keys() {
        for (_int_name, integration) in &config.integrations {
            if let Some(ref agents_dir_str) = integration.agents_dir {
                let agents_dir = PathBuf::from(agents_dir_str);
                let link_path = agents_dir.join(format!("{}.md", agent_name));
                if link_path.exists() {
                    std::fs::remove_file(&link_path).ok();
                }
            }
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
        "Purged {} repositories, {} skills, and {} agents",
        style(repo_count).cyan(),
        style(skill_count).cyan(),
        style(agent_count).cyan()
    );

    Ok(())
}
