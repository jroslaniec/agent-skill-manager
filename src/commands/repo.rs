use anyhow::{bail, Context, Result};
use console::style;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use crate::commands::skill::remove_skill_symlinks_from_all_integrations;
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

    // Check for conflicts with same git repo, different path
    let git_repo_key = format!("{}/{}", repo_ref.owner, repo_ref.repo);
    for (existing_repo_id, _) in &config.repositories {
        if existing_repo_id.starts_with(&format!("github.com/{}", git_repo_key))
            && existing_repo_id != &repo_ref.repo_id() {
            bail!(
                "Cannot add {}. A different path from the same git repository is already registered: {}\nOnly one skill repository per git repository is allowed.",
                repo_ref.repo_id(),
                existing_repo_id
            );
        }
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

    // Handle SHA pinning if specified
    let (current_sha, pinned_sha) = if let Some(sha) = &repo_ref.sha {
        // Checkout the specific SHA
        println!(
            "Checking out commit {}...",
            style(sha).yellow()
        );
        git::checkout_sha(&repo_cache_path, sha)?;
        let actual_sha = git::get_current_sha(&repo_cache_path)?;
        println!("{} Pinned to {}", style("✓").green(), style(&actual_sha).cyan());
        (Some(actual_sha.clone()), Some(actual_sha))
    } else {
        // No SHA specified, just get current SHA
        let sha = git::get_current_sha(&repo_cache_path)?;
        (Some(sha), None)
    };

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
            current_sha.clone(),
            pinned_sha.clone(),
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

    // Remove symlinks from all integrations
    for skill_name in &skill_names {
        remove_skill_symlinks_from_all_integrations(skill_name, &config);
    }

    lock.update(|config| {
        // Remove all skills
        for skill_name in &skill_names {
            config.remove_skill(skill_name);
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
        "{:<40}  {:>6}  {:>7}  {:>8}  {:<10}  {:<10}",
        style("REPOSITORY").bold(),
        style("TOTAL").bold(),
        style("ENABLED").bold(),
        style("DISABLED").bold(),
        style("CURRENT").bold(),
        style("PIN").bold()
    );

    // Print separator
    println!("{}", "-".repeat(100));

    // Print each repository
    for (repo_id, repo) in &config.repositories {
        // Get all skills for this repository
        let skills = config.skills_for_repo(repo_id);
        let total_count = skills.len();
        let enabled_count = skills.iter().filter(|s| s.enabled).count();
        let disabled_count = skills.iter().filter(|s| !s.enabled).count();

        let current_sha_display = repo
            .current_sha
            .as_ref()
            .map(|s| {
                let short = if s.len() > 8 { &s[..8] } else { s };
                short.to_string()
            })
            .unwrap_or_else(|| "-".to_string());

        let pinned_sha_display = repo
            .pinned_sha
            .as_ref()
            .map(|s| {
                let short = if s.len() > 8 { &s[..8] } else { s };
                style(short).yellow().to_string()
            })
            .unwrap_or_else(|| "-".to_string());

        println!(
            "{:<40}  {:>6}  {:>7}  {:>8}  {:<10}  {}",
            style(repo_id).cyan(),
            total_count,
            style(enabled_count).green(),
            style(disabled_count).dim(),
            style(&current_sha_display).dim(),
            pinned_sha_display
        );
    }

    Ok(())
}

pub fn pin(url: &str) -> Result<()> {
    let repo_ref = RepoRef::parse(url)?;
    let lock = ConfigLock::acquire()?;

    // Check if repository exists
    let config = lock.read_config()?;
    if !config.has_repository(&repo_ref.repo_id()) {
        bail!(
            "Repository '{}' is not registered. Use 'sm repo list' to see all repositories.",
            repo_ref.repo_id()
        );
    }

    // Get current SHA from git working directory
    let git_cache = paths::git_cache_dir()?;
    let repo_cache_path = git_cache
        .join(&repo_ref.owner)
        .join(&repo_ref.repo);

    let current_sha = git::get_current_sha(&repo_cache_path)?;

    // Update repository to pin it
    lock.update(|config| {
        if let Some(repo) = config.repositories.get_mut(&repo_ref.repo_id()) {
            repo.pinned_sha = Some(current_sha.clone());
            repo.current_sha = Some(current_sha.clone());
        }
        Ok(())
    })?;

    println!(
        "Pinned {} at {}",
        style(&repo_ref.repo_id()).cyan(),
        style(&current_sha).yellow()
    );

    Ok(())
}

pub fn unpin(url: &str) -> Result<()> {
    let repo_ref = RepoRef::parse(url)?;
    let lock = ConfigLock::acquire()?;

    // Check if repository exists
    let config = lock.read_config()?;
    if !config.has_repository(&repo_ref.repo_id()) {
        bail!(
            "Repository '{}' is not registered. Use 'sm repo list' to see all repositories.",
            repo_ref.repo_id()
        );
    }

    let current_sha = config
        .repositories
        .get(&repo_ref.repo_id())
        .and_then(|r| r.current_sha.clone())
        .unwrap_or_else(|| "unknown".to_string());

    // Update repository to unpin it
    lock.update(|config| {
        if let Some(repo) = config.repositories.get_mut(&repo_ref.repo_id()) {
            repo.pinned_sha = None;
        }
        Ok(())
    })?;

    println!(
        "Unpinned {} (currently at {})",
        style(&repo_ref.repo_id()).cyan(),
        style(&current_sha).yellow()
    );

    Ok(())
}

/// Reconcile skills after repository upgrade - handle deleted/new skills
fn reconcile_skills(
    config: &mut crate::config::state::Config,
    repo_id: &str,
    new_skills: &[String],
    repo_path: &str,
) -> Result<(Vec<String>, Vec<String>)> {
    use std::collections::HashSet;

    let new_skills_set: HashSet<_> = new_skills.iter().collect();

    // Get existing skills for this repository
    let existing_skills: Vec<_> = config
        .skills
        .iter()
        .filter(|(_, skill)| skill.repository == repo_id)
        .map(|(name, skill)| (name.clone(), skill.clone()))
        .collect();

    let mut removed_skills = Vec::new();
    let mut added_skills = Vec::new();

    // Find deleted skills (exist in config but not in filesystem)
    for (skill_name, _skill) in &existing_skills {
        if !new_skills_set.contains(skill_name) {
            // Skill was deleted from repository
            removed_skills.push(skill_name.clone());

            // Remove symlinks from all integrations
            for (_int_name, integration) in &config.integrations {
                let skills_dir = PathBuf::from(&integration.skills_dir);
                let link_path = skills_dir.join(skill_name);
                if link_path.exists() || link_path.symlink_metadata().is_ok() {
                    std::fs::remove_file(&link_path).ok();
                }
            }

            // Remove from config entirely
            config.remove_skill(skill_name);
        }
    }

    // Find new skills (exist in filesystem but not in config)
    for skill_name in new_skills {
        if !existing_skills.iter().any(|(name, _)| name == skill_name) {
            added_skills.push(skill_name.clone());

            // Add to config as disabled
            let skill_path = if repo_path.is_empty() {
                skill_name.clone()
            } else {
                format!("{}/{}", repo_path, skill_name)
            };

            config.add_skill(
                skill_name.clone(),
                repo_id.to_string(),
                skill_path,
            );
            config.disable_skill(skill_name).ok();
        }
    }

    Ok((removed_skills, added_skills))
}

pub fn upgrade(url: &str) -> Result<()> {
    let repo_ref = RepoRef::parse(url)?;
    let lock = ConfigLock::acquire()?;
    upgrade_with_lock(&repo_ref, &lock)
}

/// Internal upgrade logic that uses an existing lock
fn upgrade_with_lock(repo_ref: &RepoRef, lock: &ConfigLock) -> Result<()> {
    // Check if repository exists
    let config = lock.read_config()?;
    if !config.has_repository(&repo_ref.repo_id()) {
        bail!(
            "Repository '{}' is not registered. Use 'sm repo list' to see all repositories.",
            repo_ref.repo_id()
        );
    }

    let git_cache = paths::git_cache_dir()?;
    let repo_cache_path = git_cache
        .join(&repo_ref.owner)
        .join(&repo_ref.repo);

    if let Some(target_sha) = &repo_ref.sha {
        // Mode B: Upgrade to specific SHA and pin
        println!(
            "Upgrading {} to {}...",
            style(&repo_ref.repo_id()).cyan(),
            style(target_sha).yellow()
        );

        git::checkout_sha(&repo_cache_path, target_sha)?;
        let actual_sha = git::get_current_sha(&repo_cache_path)?;

        // Rescan skills
        let full_path = if repo_ref.path.is_empty() {
            repo_cache_path.clone()
        } else {
            repo_cache_path.join(&repo_ref.path)
        };
        let new_skills = scan_for_skills(&full_path)?;

        // Update repository and reconcile skills
        let mut removed = Vec::new();
        let mut added = Vec::new();

        lock.update(|config| {
            if let Some(repo) = config.repositories.get_mut(&repo_ref.repo_id()) {
                repo.current_sha = Some(actual_sha.clone());
                repo.pinned_sha = Some(actual_sha.clone());
            }

            // Reconcile skills - handle deleted and new skills
            let (r, a) = reconcile_skills(config, &repo_ref.repo_id(), &new_skills, &repo_ref.path)?;
            removed = r;
            added = a;

            Ok(())
        })?;

        // Report results
        println!(
            "{} Upgraded and pinned to {}",
            style("✓").green(),
            style(&actual_sha).cyan()
        );

        if !removed.is_empty() {
            println!("{} Removed: {} (no longer available)",
                style("⚠").yellow(),
                removed.join(", ")
            );
        }
        if !added.is_empty() {
            println!("{} New: {}",
                style("✓").green(),
                added.join(", ")
            );
        }
    } else {
        // Mode A: Upgrade to latest
        let repo = config.repositories.get(&repo_ref.repo_id()).unwrap();

        if repo.pinned_sha.is_some() {
            bail!(
                "Repository '{}' is pinned. Use 'sm repo unpin' first or specify a commit with @SHA.",
                repo_ref.repo_id()
            );
        }

        let old_sha = repo.current_sha.clone().unwrap_or_else(|| "unknown".to_string());

        println!(
            "Upgrading {}...",
            style(&repo_ref.repo_id()).cyan()
        );

        let new_sha = git::pull_to_latest(&repo_cache_path)?;

        if old_sha == new_sha {
            println!(
                "{} Already at latest ({})",
                style("→").dim(),
                style(&new_sha).dim()
            );
            return Ok(());
        }

        // Rescan skills
        let full_path = if repo_ref.path.is_empty() {
            repo_cache_path.clone()
        } else {
            repo_cache_path.join(&repo_ref.path)
        };
        let new_skills = scan_for_skills(&full_path)?;

        // Update repository and reconcile skills
        let mut removed = Vec::new();
        let mut added = Vec::new();

        lock.update(|config| {
            if let Some(repo) = config.repositories.get_mut(&repo_ref.repo_id()) {
                repo.current_sha = Some(new_sha.clone());
            }

            // Reconcile skills - handle deleted and new skills
            let (r, a) = reconcile_skills(config, &repo_ref.repo_id(), &new_skills, &repo_ref.path)?;
            removed = r;
            added = a;

            Ok(())
        })?;

        // Report results
        println!(
            "{} Upgraded {} → {}",
            style("✓").green(),
            style(&old_sha[..8]).dim(),
            style(&new_sha).cyan()
        );

        if !removed.is_empty() {
            println!("{} Removed: {} (no longer available)",
                style("⚠").yellow(),
                removed.join(", ")
            );
        }
        if !added.is_empty() {
            println!("{} New: {}",
                style("✓").green(),
                added.join(", ")
            );
        }
    }

    Ok(())
}

pub fn upgrade_all(force: bool) -> Result<()> {
    let lock = ConfigLock::acquire()?;
    let config = lock.read_config()?;

    // Filter unpinned repositories
    let unpinned_repos: Vec<_> = config
        .repositories
        .iter()
        .filter(|(_, repo)| repo.pinned_sha.is_none())
        .collect();

    if unpinned_repos.is_empty() {
        println!("No repositories to upgrade (all are pinned)");
        return Ok(());
    }

    // Show what will be upgraded
    println!(
        "Will upgrade {} repositories:",
        style(unpinned_repos.len()).cyan()
    );
    for (repo_id, _) in &unpinned_repos {
        println!("  - {}", repo_id);
    }

    // Confirm unless --force
    if !force {
        print!("\nContinue? (yes/no): ");
        io::stdout().flush()?;

        let mut response = String::new();
        io::stdin().read_line(&mut response)?;

        if response.trim().to_lowercase() != "yes" {
            println!("Cancelled.");
            return Ok(());
        }
    }

    println!();

    // Upgrade each repository
    let mut success_count = 0;
    let mut failed_count = 0;

    for (repo_id, _) in unpinned_repos {
        match RepoRef::parse(repo_id).and_then(|repo_ref| upgrade_with_lock(&repo_ref, &lock)) {
            Ok(_) => {
                // Check if it was actually changed by looking at the output
                // For now, just count as success
                success_count += 1;
            }
            Err(e) => {
                eprintln!("{} Failed to upgrade {}: {}", style("✗").red(), repo_id, e);
                failed_count += 1;
            }
        }
    }

    println!();
    println!(
        "Upgrade complete: {} succeeded, {} failed",
        style(success_count).green(),
        style(failed_count).red()
    );

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
