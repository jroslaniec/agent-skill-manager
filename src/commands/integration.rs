use anyhow::{Context, Result, bail};
use console::style;
use std::path::PathBuf;

use crate::commands::skill::create_skill_symlink;
use crate::config::ConfigLock;
use crate::paths;
use crate::skill_ref::RepoRef;

/// Add a new integration
pub fn add(name: &str, custom_path: Option<&str>, custom_agents_path: Option<&str>) -> Result<()> {
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

    // Determine skills and agents directories
    let (skills_dir, agents_dir) = match (custom_path, custom_agents_path) {
        (Some(path), Some(agents_path)) => {
            // Both custom paths provided
            (
                Some(paths::expand_tilde(path)?),
                Some(paths::expand_tilde(agents_path)?),
            )
        }
        (Some(path), None) => {
            // Only skills path provided - for custom integrations
            (Some(paths::expand_tilde(path)?), None)
        }
        (None, Some(agents_path)) => {
            // Only agents path provided - for custom integrations
            (None, Some(paths::expand_tilde(agents_path)?))
        }
        (None, None) => {
            // No custom paths - use built-in defaults
            match (
                paths::get_builtin_skills_dir(&normalized_name),
                paths::get_builtin_agents_dir(&normalized_name),
            ) {
                (None, None) => {
                    bail!(
                        "Unknown integration '{}'. Use --path and/or --agents-path to specify directories.\n\
                         Known integrations: {}",
                        name,
                        paths::builtin_integrations()
                            .iter()
                            .map(|bi| format!(
                                "{} (skills: {}, agents: {})",
                                bi.name,
                                bi.skills_dir.unwrap_or("-"),
                                bi.agents_dir.unwrap_or("-")
                            ))
                            .collect::<Vec<_>>()
                            .join(", ")
                    );
                }
                (skills, agents) => (skills, agents),
            }
        }
    };

    // Create directories if they don't exist
    if let Some(ref dir) = skills_dir
        && !dir.exists()
    {
        std::fs::create_dir_all(dir).context(format!(
            "Failed to create skills directory: {}",
            dir.display()
        ))?;
        println!("Created skills directory: {}", style(dir.display()).dim());
    }

    if let Some(ref dir) = agents_dir
        && !dir.exists()
    {
        std::fs::create_dir_all(dir).context(format!(
            "Failed to create agents directory: {}",
            dir.display()
        ))?;
        println!("Created agents directory: {}", style(dir.display()).dim());
    }

    // Add integration to config
    let skills_dir_str = skills_dir.map(|p| p.to_string_lossy().to_string());
    let agents_dir_str = agents_dir.map(|p| p.to_string_lossy().to_string());
    lock.update(|config| {
        config.add_integration(
            normalized_name.clone(),
            skills_dir_str.clone(),
            agents_dir_str.clone(),
        );
        Ok(())
    })?;

    // Build display string for directories
    let dirs_display = match (&skills_dir_str, &agents_dir_str) {
        (Some(s), Some(a)) => format!("skills: {}, agents: {}", s, a),
        (Some(s), None) => format!("skills: {}", s),
        (None, Some(a)) => format!("agents: {}", a),
        (None, None) => "no directories configured".to_string(),
    };

    println!(
        "Added integration {} ({})",
        style(&normalized_name).green(),
        style(&dirs_display).dim()
    );

    // Create symlinks for all enabled skills (if skills_dir is set)
    if let Some(ref skills_dir_path) = skills_dir_str {
        let skills_dir = PathBuf::from(skills_dir_path);
        let config = lock.read_config()?;
        let enabled_skills = config.enabled_skills();

        if !enabled_skills.is_empty() {
            println!("Syncing {} enabled skill(s)...", enabled_skills.len());

            for (skill_name, skill) in &enabled_skills {
                // Get source path from git cache
                if let Ok(repo_ref) = RepoRef::parse(&skill.repository) {
                    let git_cache = paths::git_cache_dir()?;
                    let repo_path = git_cache.join(repo_ref.cache_path());
                    let source_path = repo_path.join(&skill.skill_path);

                    if source_path.exists() {
                        let link_path = skills_dir.join(skill_name);
                        match create_skill_symlink(&source_path, &link_path) {
                            Ok(_) => println!("  {} {}", style("*").green(), skill_name),
                            Err(e) => println!("  {} {}: {}", style("!").yellow(), skill_name, e),
                        }
                    }
                }
            }
        }
    }

    Ok(())
}

/// Migrate legacy per-tool integrations (`codex`, `gemini-cli`, `opencode`) to
/// the unified `agents` integration that targets the shared `~/.agents/skills`
/// location those tools now read. Best-effort and idempotent: a fast no-op once
/// there are no legacy integrations left.
///
/// Returns `Ok(true)` if a migration was performed.
pub fn migrate_legacy_integrations(lock: &ConfigLock) -> Result<bool> {
    const LEGACY: [&str; 3] = ["codex", "gemini-cli", "opencode"];

    let config = lock.read_config()?;
    let present: Vec<&str> = LEGACY
        .iter()
        .copied()
        .filter(|name| config.has_integration(name))
        .collect();
    if present.is_empty() {
        return Ok(false);
    }

    let enabled_skills: Vec<String> = config
        .enabled_skills()
        .iter()
        .map(|(name, _)| (*name).clone())
        .collect();

    // 1. Remove this manager's skill symlinks from the legacy integration dirs
    //    (only entries that are symlinks — never touch user-placed files).
    for name in &present {
        if let Some(integration) = config.integrations.get(*name)
            && let Some(dir) = integration.skills_dir.as_ref()
        {
            let dir = PathBuf::from(dir);
            for skill in &enabled_skills {
                let link = dir.join(skill);
                if link.is_symlink() {
                    std::fs::remove_file(&link).ok();
                }
            }
        }
    }

    // 2. Config: replace the legacy integrations with the unified `agents` one.
    let agents_dir =
        paths::get_builtin_skills_dir("agents").map(|p| p.to_string_lossy().to_string());
    let mut migrated: Vec<String> = Vec::new();
    lock.update(|config| {
        migrated = config.unify_legacy_integrations(agents_dir.clone());
        Ok(())
    })?;

    // 3. Symlink all enabled skills into the unified skills directory.
    if let Some(dir) = agents_dir.as_ref() {
        let dir = PathBuf::from(dir);
        std::fs::create_dir_all(&dir).ok();
        let config = lock.read_config()?;
        for skill_name in &enabled_skills {
            if let Some(skill) = config.skills.get(skill_name)
                && let Ok(repo_ref) = RepoRef::parse(&skill.repository)
                && let Ok(cache) = paths::resolve_repo_cache_path(&repo_ref)
            {
                let source = cache.join(&skill.skill_path);
                if source.exists() {
                    create_skill_symlink(&source, &dir.join(skill_name)).ok();
                }
            }
        }
    }

    eprintln!(
        "{} Migrated {} to the unified `agents` integration ({}). Codex, Gemini CLI, and OpenCode all read this location now.",
        style("Note:").yellow(),
        migrated.join(", "),
        agents_dir.as_deref().unwrap_or("~/.agents/skills")
    );

    Ok(true)
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

    // Remove symlinks for all enabled skills from skills_dir
    if let Some(ref skills_dir_str) = integration.skills_dir {
        let skills_dir = PathBuf::from(skills_dir_str);
        let enabled_skills = config.enabled_skills();
        if !enabled_skills.is_empty() && skills_dir.exists() {
            println!("Removing skill symlinks from {}...", skills_dir.display());

            for (skill_name, _) in &enabled_skills {
                let link_path = skills_dir.join(skill_name);
                if link_path.is_symlink() {
                    match std::fs::remove_file(&link_path) {
                        Ok(_) => println!("  {} {}", style("-").red(), skill_name),
                        Err(e) => println!("  {} {}: {}", style("!").yellow(), skill_name, e),
                    }
                }
            }
        }
    }

    // Remove symlinks for all enabled agents from agents_dir
    if let Some(ref agents_dir_str) = integration.agents_dir {
        let agents_dir = PathBuf::from(agents_dir_str);
        let enabled_agents = config.enabled_agents();
        if !enabled_agents.is_empty() && agents_dir.exists() {
            println!("Removing agent symlinks from {}...", agents_dir.display());

            for (agent_name, _) in &enabled_agents {
                let link_path = agents_dir.join(format!("{}.md", agent_name));
                if link_path.is_symlink() {
                    match std::fs::remove_file(&link_path) {
                        Ok(_) => println!("  {} {}", style("-").red(), agent_name),
                        Err(e) => println!("  {} {}: {}", style("!").yellow(), agent_name, e),
                    }
                }
            }
        }
    }

    // Remove from config
    lock.update(|config| {
        config.remove_integration(&normalized_name);
        Ok(())
    })?;

    println!("Removed integration {}", style(&normalized_name).red());

    Ok(())
}

/// List all registered integrations
pub fn list() -> Result<()> {
    let lock = ConfigLock::acquire()?;
    let config = lock.read_config()?;

    // Print header
    println!(
        "{:<15}  {:<10}  {:<30}  {}",
        style("INTEGRATION").bold(),
        style("STATUS").bold(),
        style("SKILLS DIRECTORY").bold(),
        style("AGENTS DIRECTORY").bold(),
    );

    // Print separator
    println!("{}", "-".repeat(100));

    // Helper to check if any configured directory exists
    fn check_status(
        skills_dir: &Option<String>,
        agents_dir: &Option<String>,
    ) -> console::StyledObject<&'static str> {
        let skills_ok = skills_dir
            .as_ref()
            .map(|p| PathBuf::from(p).exists())
            .unwrap_or(true);
        let agents_ok = agents_dir
            .as_ref()
            .map(|p| PathBuf::from(p).exists())
            .unwrap_or(true);
        if skills_ok && agents_ok {
            style("enabled").green()
        } else {
            style("missing").yellow()
        }
    }

    // Always show all built-in integrations
    let builtins = paths::builtin_integrations();

    for bi in &builtins {
        if let Some(integration) = config.integrations.get(bi.name) {
            // Registered integration
            let status = check_status(&integration.skills_dir, &integration.agents_dir);
            let skills = integration.skills_dir.as_deref().unwrap_or("-");
            let agents = integration.agents_dir.as_deref().unwrap_or("-");
            println!(
                "{:<15}  {:<10}  {:<30}  {}",
                style(bi.name).cyan(),
                status,
                skills,
                agents
            );
        } else {
            // Not registered - show defaults as disabled
            let skills = bi.skills_dir.unwrap_or("-");
            let agents = bi.agents_dir.unwrap_or("-");
            println!(
                "{:<15}  {:<10}  {:<30}  {}",
                style(bi.name).dim(),
                style("-").dim(),
                style(skills).dim(),
                style(agents).dim()
            );
        }
    }

    // Show any custom integrations (not in builtins)
    let builtin_names: Vec<&str> = builtins.iter().map(|bi| bi.name).collect();
    let mut custom: Vec<_> = config
        .integrations
        .iter()
        .filter(|(name, _)| !builtin_names.contains(&name.as_str()))
        .collect();
    custom.sort_by(|a, b| a.0.cmp(b.0));

    for (name, integration) in custom {
        let status = check_status(&integration.skills_dir, &integration.agents_dir);
        let skills = integration.skills_dir.as_deref().unwrap_or("-");
        let agents = integration.agents_dir.as_deref().unwrap_or("-");
        println!(
            "{:<15}  {:<10}  {:<30}  {}",
            style(name).cyan(),
            status,
            skills,
            agents
        );
    }

    Ok(())
}

/// Interactive configuration - select integrations with MultiSelect
pub fn configure() -> Result<()> {
    let lock = ConfigLock::acquire()?;
    let config = lock.read_config()?;

    let builtin = paths::builtin_integrations();

    // Build options showing the skills directory (and agents dir when present),
    // plus a short description of what each integration covers.
    let options: Vec<String> = builtin
        .iter()
        .map(|bi| {
            let mut label = format!("{} (skills: {}", bi.name, bi.skills_dir.unwrap_or("-"));
            if let Some(agents) = bi.agents_dir {
                label.push_str(&format!(", agents: {}", agents));
            }
            label.push(')');
            if let Some(desc) = bi.description {
                label.push_str(&format!("  —  {}", desc));
            }
            label
        })
        .collect();

    // Get currently enabled integrations
    let default_indices: Vec<usize> = builtin
        .iter()
        .enumerate()
        .filter(|(_, bi)| config.has_integration(bi.name))
        .map(|(i, _)| i)
        .collect();

    // Show MultiSelect prompt
    let selected = inquire::MultiSelect::new("Select integrations to enable:", options.clone())
        .with_default(&default_indices)
        .with_help_message("Type to search, Space to toggle, Enter to confirm, Esc to cancel")
        .with_scorer(&|input, _, string_value, index| {
            crate::interactive::substring_score(input, string_value, index)
        })
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

    for (i, bi) in builtin.iter().enumerate() {
        let formatted = &options[i];
        let is_selected = selected_options.contains(formatted);
        let is_enabled = config.has_integration(bi.name);

        if is_selected && !is_enabled {
            to_add.push(bi.name);
        } else if !is_selected && is_enabled {
            to_remove.push(bi.name);
        }
    }

    // Check if there are any changes before proceeding
    let has_changes = !to_add.is_empty() || !to_remove.is_empty();

    // Drop lock before calling add/remove (they acquire their own locks)
    drop(lock);

    // Apply changes
    for name in to_add {
        add(name, None, None)?;
    }

    for name in to_remove {
        remove(name)?;
    }

    if !has_changes {
        println!("No changes made.");
    }

    Ok(())
}
