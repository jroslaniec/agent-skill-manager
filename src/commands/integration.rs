use anyhow::{bail, Context, Result};
use console::style;
use std::path::PathBuf;

use crate::commands::skill::create_skill_symlink;
use crate::config::ConfigLock;
use crate::paths;
use crate::skill_ref::RepoRef;

/// Add a new integration
pub fn add(name: &str, custom_path: Option<&str>) -> Result<()> {
    let lock = ConfigLock::acquire()?;

    let normalized_name = paths::normalize_integration_name(name);

    // Check if already registered
    let config = lock.read_config()?;
    if config.has_integration(&normalized_name) {
        println!(
            "Integration {} is already registered",
            style(&normalized_name).cyan()
        );
        return Ok(());
    }

    // Determine skills directory
    let skills_dir = match custom_path {
        Some(path) => paths::expand_tilde(path)?,
        None => {
            match paths::get_builtin_skills_dir(&normalized_name) {
                Some(path) => path,
                None => {
                    bail!(
                        "Unknown integration '{}'. Use --path to specify the skills directory.\n\
                         Known integrations: {}",
                        name,
                        paths::builtin_integrations()
                            .iter()
                            .map(|(n, p)| format!("{} ({})", n, p))
                            .collect::<Vec<_>>()
                            .join(", ")
                    );
                }
            }
        }
    };

    // Create directory if it doesn't exist
    if !skills_dir.exists() {
        std::fs::create_dir_all(&skills_dir)
            .context(format!("Failed to create skills directory: {}", skills_dir.display()))?;
        println!(
            "Created skills directory: {}",
            style(skills_dir.display()).dim()
        );
    }

    // Add integration to config
    let skills_dir_str = skills_dir.to_string_lossy().to_string();
    lock.update(|config| {
        config.add_integration(normalized_name.clone(), skills_dir_str.clone());
        Ok(())
    })?;

    println!(
        "Added integration {} ({})",
        style(&normalized_name).green(),
        style(&skills_dir_str).dim()
    );

    // Create symlinks for all enabled skills
    let config = lock.read_config()?;
    let enabled_skills = config.enabled_skills();

    if !enabled_skills.is_empty() {
        println!("Syncing {} enabled skill(s)...", enabled_skills.len());

        for (skill_name, skill) in &enabled_skills {
            // Get source path from git cache
            if let Ok(repo_ref) = RepoRef::parse(&skill.repository) {
                let git_cache = paths::git_cache_dir()?;
                let repo_path = git_cache.join(&repo_ref.owner).join(&repo_ref.repo);
                let source_path = repo_path.join(&skill.skill_path);

                if source_path.exists() {
                    let link_path = skills_dir.join(skill_name);
                    match create_skill_symlink(&source_path, &link_path) {
                        Ok(_) => println!("  {} {}", style("*").green(), skill_name),
                        Err(e) => println!(
                            "  {} {}: {}",
                            style("!").yellow(),
                            skill_name,
                            e
                        ),
                    }
                }
            }
        }
    }

    Ok(())
}

/// Remove an integration
pub fn remove(name: &str) -> Result<()> {
    let lock = ConfigLock::acquire()?;

    let normalized_name = paths::normalize_integration_name(name);

    let config = lock.read_config()?;
    let integration = match config.integrations.get(&normalized_name) {
        Some(int) => int.clone(),
        None => {
            bail!(
                "Integration '{}' is not registered.\nRun 'sm integrations list' to see registered integrations.",
                name
            );
        }
    };

    let skills_dir = PathBuf::from(&integration.skills_dir);

    // Remove symlinks for all enabled skills
    let enabled_skills = config.enabled_skills();
    if !enabled_skills.is_empty() && skills_dir.exists() {
        println!("Removing symlinks from {}...", skills_dir.display());

        for (skill_name, _) in &enabled_skills {
            let link_path = skills_dir.join(skill_name);
            if link_path.is_symlink() {
                match std::fs::remove_file(&link_path) {
                    Ok(_) => println!("  {} {}", style("-").red(), skill_name),
                    Err(e) => println!(
                        "  {} {}: {}",
                        style("!").yellow(),
                        skill_name,
                        e
                    ),
                }
            }
        }
    }

    // Remove from config
    lock.update(|config| {
        config.remove_integration(&normalized_name);
        Ok(())
    })?;

    println!(
        "Removed integration {}",
        style(&normalized_name).red()
    );

    Ok(())
}

/// List all registered integrations
pub fn list() -> Result<()> {
    let lock = ConfigLock::acquire()?;
    let config = lock.read_config()?;

    // Print header
    println!(
        "{:<15}  {:<10}  {}",
        style("INTEGRATION").bold(),
        style("STATUS").bold(),
        style("SKILLS DIRECTORY").bold(),
    );

    // Print separator
    println!("{}", "-".repeat(80));

    // Always show all built-in integrations
    let builtins = paths::builtin_integrations();

    for (name, default_path) in &builtins {
        if let Some(integration) = config.integrations.get(*name) {
            // Registered integration
            let path_exists = PathBuf::from(&integration.skills_dir).exists();
            let status = if path_exists {
                style("enabled").green()
            } else {
                style("missing").yellow()
            };
            println!(
                "{:<15}  {:<10}  {}",
                style(*name).cyan(),
                status,
                &integration.skills_dir
            );
        } else {
            // Not registered - show as disabled
            println!(
                "{:<15}  {:<10}  {}",
                style(*name).dim(),
                style("-").dim(),
                style(*default_path).dim()
            );
        }
    }

    // Show any custom integrations (not in builtins)
    let builtin_names: Vec<&str> = builtins.iter().map(|(n, _)| *n).collect();
    let mut custom: Vec<_> = config
        .integrations
        .iter()
        .filter(|(name, _)| !builtin_names.contains(&name.as_str()))
        .collect();
    custom.sort_by(|a, b| a.0.cmp(b.0));

    for (name, integration) in custom {
        let path_exists = PathBuf::from(&integration.skills_dir).exists();
        let status = if path_exists {
            style("enabled").green()
        } else {
            style("missing").yellow()
        };
        println!(
            "{:<15}  {:<10}  {}",
            style(name).cyan(),
            status,
            &integration.skills_dir
        );
    }

    Ok(())
}

/// Interactive configuration - select integrations with MultiSelect
pub fn configure() -> Result<()> {
    let lock = ConfigLock::acquire()?;
    let config = lock.read_config()?;

    let builtin = paths::builtin_integrations();

    // Build options
    let options: Vec<String> = builtin
        .iter()
        .map(|(name, path)| format!("{} ({})", name, path))
        .collect();

    // Get currently enabled integrations
    let default_indices: Vec<usize> = builtin
        .iter()
        .enumerate()
        .filter(|(_, (name, _))| config.has_integration(*name))
        .map(|(i, _)| i)
        .collect();

    // Show MultiSelect prompt
    let selected = inquire::MultiSelect::new("Select integrations to enable:", options.clone())
        .with_default(&default_indices)
        .with_help_message("Space to toggle, Enter to confirm, Esc to cancel")
        .prompt();

    let selected_options = match selected {
        Ok(selections) => selections,
        Err(_) => {
            println!("Cancelled");
            return Ok(());
        }
    };

    // Determine what changed
    let mut to_add: Vec<&str> = Vec::new();
    let mut to_remove: Vec<&str> = Vec::new();

    for (i, (name, _)) in builtin.iter().enumerate() {
        let formatted = &options[i];
        let is_selected = selected_options.contains(formatted);
        let is_enabled = config.has_integration(*name);

        if is_selected && !is_enabled {
            to_add.push(*name);
        } else if !is_selected && is_enabled {
            to_remove.push(*name);
        }
    }

    // Check if there are any changes before proceeding
    let has_changes = !to_add.is_empty() || !to_remove.is_empty();

    // Drop lock before calling add/remove (they acquire their own locks)
    drop(lock);

    // Apply changes
    for name in to_add {
        add(name, None)?;
    }

    for name in to_remove {
        remove(name)?;
    }

    if !has_changes {
        println!("No changes made.");
    }

    Ok(())
}
