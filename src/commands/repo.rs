use anyhow::{bail, Context, Result};
use console::style;
use std::io::{self, Write};
use std::path::Path;

use crate::config::ConfigLock;
use crate::git;
use crate::paths;
use crate::skill_ref::RepoRef;

pub fn add(url: &str) -> Result<()> {
    let repo_ref = RepoRef::parse(url)?;
    let lock = ConfigLock::acquire()?;

    // Check if repository already exists
    let config = lock.read_config()?;
    if config.has_repository(&repo_ref.repo_id()) {
        bail!(
            "Repository '{}' is already registered. Use 'sm repo list' to see all repositories.",
            repo_ref.repo_id()
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

    // Scan for skills in the repository
    let available_skills = scan_for_skills(&full_path)?;

    // Add repository and register all skills as disabled
    lock.update(|config| {
        config.add_repository(
            repo_ref.repo_id(),
            repo_ref.git_url(),
            repo_ref.path.clone(),
        );

        // Register all detected skills as disabled
        for skill_name in &available_skills {
            let skill_path = if repo_ref.path.is_empty() {
                skill_name.clone()
            } else {
                format!("{}/{}", repo_ref.path, skill_name)
            };

            // Only add if not already registered
            if !config.has_skill(skill_name) {
                config.add_skill(
                    skill_name.clone(),
                    repo_ref.repo_id(),
                    skill_path,
                );
                // Immediately disable it (add_skill enables by default)
                config.disable_skill(skill_name).ok();
            }
        }

        Ok(())
    })?;

    println!(
        "Added repository {} ({} skills)",
        style(&repo_ref.repo_id()).cyan(),
        available_skills.len()
    );

    Ok(())
}

pub fn delete(url: &str, force: bool) -> Result<()> {
    let repo_ref = RepoRef::parse(url)?;
    let lock = ConfigLock::acquire()?;

    let config = lock.read_config()?;

    // Check if repository exists
    if !config.has_repository(&repo_ref.repo_id()) {
        bail!(
            "Repository '{}' not found. Use 'sm repo list' to see registered repositories.",
            repo_ref.repo_id()
        );
    }

    // Check if any skills are using this repository
    let skills = config.skills_for_repo(&repo_ref.repo_id());
    let enabled_count = skills.iter().filter(|s| s.enabled).count();

    if enabled_count > 0 && !force {
        println!(
            "{} {} enabled skill(s) from {} are registered.",
            style("Warning:").yellow(),
            enabled_count,
            style(&repo_ref.repo_id()).cyan()
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
        config.remove_repository(&repo_ref.repo_id());

        Ok(())
    })?;

    println!(
        "Removed repository {} and {} skill(s)",
        style(&repo_ref.repo_id()).cyan(),
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
        // Get all skills for this repository
        let skills = config.skills_for_repo(repo_id);
        let total_count = skills.len();
        let enabled_count = skills.iter().filter(|s| s.enabled).count();
        let disabled_count = skills.iter().filter(|s| !s.enabled).count();

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

/// Scan a directory for skills (directories containing SKILL.md)
fn scan_for_skills(path: &Path) -> Result<Vec<String>> {
    let mut skills = Vec::new();

    if !path.exists() {
        return Ok(skills);
    }

    // Read directory entries
    let entries = std::fs::read_dir(path).context("Failed to read directory")?;

    for entry in entries {
        let entry = entry?;
        let entry_path = entry.path();

        // Check if it's a directory
        if entry_path.is_dir() {
            // Check if it contains SKILL.md
            let skill_md = entry_path.join("SKILL.md");
            if skill_md.exists() {
                if let Some(skill_name) = entry_path.file_name() {
                    skills.push(skill_name.to_string_lossy().to_string());
                }
            }
        }
    }

    skills.sort();
    Ok(skills)
}
