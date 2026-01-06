use anyhow::{bail, Context, Result};
use console::style;
use std::io::{self, Write};

use crate::config::ConfigLock;
use crate::git;
use crate::paths;
use crate::skill_ref::RepoRef;

pub fn add(url: &str) -> Result<()> {
    let repo_ref = RepoRef::parse(url)?;
    let lock = ConfigLock::acquire()?;

    // Check if repository already exists
    let config = lock.read_config()?;
    if config.has_repository(&repo_ref.full_ref()) {
        bail!(
            "Repository '{}' is already registered. Use 'sm repo list' to see all repositories.",
            repo_ref.full_ref()
        );
    }

    // Determine where to clone the repo
    let git_cache = paths::git_cache_dir()?;
    let repo_cache_path = git_cache
        .join(&repo_ref.owner)
        .join(&repo_ref.repo);

    // Clone the repository if not already cloned
    if !repo_cache_path.exists() {
        println!(
            "Cloning repository {}...",
            style(&repo_ref.repo_id()).cyan()
        );
        git::clone_repo(&repo_ref.git_url(), &repo_cache_path)?;
    } else {
        println!(
            "Repository already cloned at {}",
            style(repo_cache_path.display()).dim()
        );
    }

    // Verify the path exists within the repository
    let full_path = if repo_ref.path.is_empty() {
        repo_cache_path.clone()
    } else {
        repo_cache_path.join(&repo_ref.path)
    };

    if !full_path.exists() {
        bail!(
            "Path '{}' does not exist in repository {}",
            repo_ref.path,
            repo_ref.repo_id()
        );
    }

    // Add to config
    lock.update(|config| {
        config.add_repository(
            repo_ref.full_ref(),
            repo_ref.git_url(),
            repo_ref.path.clone(),
        );
        Ok(())
    })?;

    println!(
        "Added repository {}",
        style(&repo_ref.full_ref()).cyan()
    );

    Ok(())
}

pub fn delete(url: &str, force: bool) -> Result<()> {
    let repo_ref = RepoRef::parse(url)?;
    let lock = ConfigLock::acquire()?;

    let config = lock.read_config()?;

    // Check if repository exists
    if !config.has_repository(&repo_ref.full_ref()) {
        bail!(
            "Repository '{}' not found. Use 'sm repo list' to see registered repositories.",
            repo_ref.full_ref()
        );
    }

    // Check if any skills are using this repository
    let skills = config.skills_for_repo(&repo_ref.full_ref());
    let enabled_count = skills.iter().filter(|s| s.enabled).count();

    if enabled_count > 0 && !force {
        println!(
            "{} {} enabled skill(s) from {} are registered.",
            style("Warning:").yellow(),
            enabled_count,
            style(&repo_ref.full_ref()).cyan()
        );
        print!("Are you sure you want to remove all these skills? (yes/no): ");
        io::stdout().flush()?;

        let mut response = String::new();
        io::stdin().read_line(&mut response)?;

        if response.trim().to_lowercase() != "yes" {
            println!("Cancelled.");
            return Ok(());
        }
    }

    // Remove all skills from this repository
    let skill_names: Vec<String> = skills
        .iter()
        .map(|s| {
            config
                .skills
                .iter()
                .find(|(_, skill)| *skill == *s)
                .map(|(name, _)| name.clone())
        })
        .flatten()
        .collect();

    lock.update(|config| {
        // Remove all skills
        for skill_name in &skill_names {
            config.remove_skill(skill_name);

            // Remove symlink if it exists
            let skill_link_path = paths::claude_skills_dir()?.join(skill_name);
            if skill_link_path.exists() {
                std::fs::remove_file(&skill_link_path)
                    .context("Failed to remove skill symlink")?;
            }
        }

        // Remove repository
        config.remove_repository(&repo_ref.full_ref());

        Ok(())
    })?;

    println!(
        "Removed repository {} and {} skill(s)",
        style(&repo_ref.full_ref()).cyan(),
        skill_names.len()
    );

    Ok(())
}

pub fn list() -> Result<()> {
    let lock = ConfigLock::acquire()?;
    let config = lock.read_config()?;

    if config.repositories.is_empty() {
        println!("No repositories registered.");
        return Ok(());
    }

    // Print header
    println!(
        "{:<50}  {:>6}  {:>7}  {:>8}",
        style("REPOSITORY").bold(),
        style("TOTAL").bold(),
        style("ENABLED").bold(),
        style("DISABLED").bold()
    );

    // Print separator
    println!("{}", "-".repeat(80));

    // Print each repository
    for (repo_id, _repo) in &config.repositories {
        let skills = config.skills_for_repo(repo_id);
        let enabled_count = skills.iter().filter(|s| s.enabled).count();
        let total_count = skills.len();
        let disabled_count = total_count - enabled_count;

        println!(
            "{:<50}  {:>6}  {:>7}  {:>8}",
            style(repo_id).cyan(),
            total_count,
            style(enabled_count).green(),
            style(disabled_count).dim()
        );
    }

    Ok(())
}
