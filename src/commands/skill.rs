use anyhow::{bail, Context, Result};
use console::style;
use std::collections::HashMap;
use std::io::IsTerminal;
use std::os::unix::fs as unix_fs;
use std::path::{Path, PathBuf};

use crate::commands::subagent;
use crate::config::state::Config;
use crate::config::ConfigLock;
use crate::git;
use crate::paths;
use crate::skill_ref::{RepoRef, SkillRef};

pub fn add(skill_refs: &[String], interactive: bool) -> Result<()> {
    let lock = ConfigLock::acquire()?;

    // Handle interactive mode
    if interactive {
        return add_interactive(&lock, skill_refs);
    }

    // Parse all skill references
    let mut skills: Vec<SkillRef> = Vec::new();
    for skill_ref in skill_refs {
        match SkillRef::parse(skill_ref) {
            Ok(skill) => skills.push(skill),
            Err(_) => {
                // SkillRef::parse failed - check if this is a valid repository URL
                // If so, provide a helpful error message suggesting -i flag or sm repo add
                if let Ok(repo_ref) = RepoRef::parse(skill_ref) {
                    // It's a valid repository URL but not a skill reference
                    eprintln!(
                        "Error: '{}' looks like a repository URL, not a skill reference.",
                        skill_ref
                    );
                    eprintln!();
                    eprintln!("To add a repository and select skills interactively, use:");
                    eprintln!("  sm add -i {}", skill_ref);
                    eprintln!();
                    eprintln!("Or add the repository first, then enable skills:");
                    eprintln!("  sm repo add {}", repo_ref.repo_id);
                    eprintln!("  sm skills enable <skill-name>");
                } else {
                    // Neither a valid skill reference nor a valid repo URL
                    eprintln!(
                        "Error: '{}' is not a valid skill reference.",
                        skill_ref
                    );
                    eprintln!();
                    eprintln!("Skill references should include the skill path, e.g.:");
                    eprintln!("  github.com/owner/repo/skill-name");
                    eprintln!("  gitlab.com/owner/repo/skill-name");
                    eprintln!("  git@github.com:owner/repo.git/skill-name");
                }
                std::process::exit(1);
            }
        }
    }

    // Group skills by repository
    let mut skills_by_repo: HashMap<String, Vec<SkillRef>> = HashMap::new();
    for skill in skills {
        let repo_id = skill.repo_id();
        skills_by_repo.entry(repo_id).or_insert_with(Vec::new).push(skill);
    }

    let mut had_errors = false;
    let mut repos_to_check_updates: Vec<(String, bool)> = Vec::new(); // (repo_id, is_pinned)

    // Process each repository
    for (repo_id, repo_skills) in skills_by_repo {
        let config = lock.read_config()?;
        let first_skill = &repo_skills[0]; // Use first skill to get repo info

        let git_cache = paths::git_cache_dir()?;
        let repo_cache_path = git_cache.join(&first_skill.owner).join(&first_skill.repo);

        // Check if repository exists
        let repo_exists = config.has_repository(&repo_id);

        if repo_exists {
            // Repository in cache - use existing
            println!("Repository {} in cache", style(&repo_id).cyan());

            // Mark for update check later
            let is_pinned = config.repositories.get(&repo_id)
                .and_then(|r| r.pinned_sha.as_ref())
                .is_some();
            repos_to_check_updates.push((repo_id.clone(), is_pinned));
        } else {
            // New repository - check for conflicts with same git repo, different path
            let git_repo_key = format!("{}/{}", first_skill.owner, first_skill.repo);
            for (existing_repo_id, _) in &config.repositories {
                if existing_repo_id.starts_with(&format!("github.com/{}", git_repo_key))
                    && existing_repo_id != &repo_id {
                    bail!(
                        "Cannot add {}. A different path from the same git repository is already registered: {}\nOnly one skill repository per git repository is allowed.",
                        repo_id,
                        existing_repo_id
                    );
                }
            }

            // Clone the git repository
            if !repo_cache_path.exists() {
                git::clone_repo(&first_skill.git_url(), &repo_cache_path)?;
            }

            // Get current SHA
            let current_sha = git::get_current_sha(&repo_cache_path).ok();

            // Scan for all skills in repo
            // Need to scan the parent directory, not the skill directory itself
            let scan_path = if first_skill.path.is_empty() {
                repo_cache_path.clone()
            } else if first_skill.path == first_skill.skill_name {
                // Path is just the skill name (e.g., "git-commit"), scan repo root
                repo_cache_path.clone()
            } else {
                // Path contains parent dirs (e.g., "skills/git-commit"), scan parent
                let parent_path = first_skill.path.rsplitn(2, '/').nth(1).unwrap_or("");
                if parent_path.is_empty() {
                    repo_cache_path.clone()
                } else {
                    repo_cache_path.join(parent_path)
                }
            };

            let available_skills = scan_for_skills(&scan_path)?;

            // Determine the repository base path (parent of skills)
            let repo_base_path = if first_skill.path.is_empty() {
                String::new()
            } else if first_skill.path == first_skill.skill_name {
                // Path is just the skill name, repo base is empty
                String::new()
            } else {
                // Path contains parent dirs, extract parent
                first_skill.path.rsplitn(2, '/').nth(1).unwrap_or("").to_string()
            };

            // Add repository to config
            lock.update(|config| {
                config.add_repository(
                    repo_id.clone(),
                    first_skill.git_url(),
                    repo_base_path.clone(),
                    current_sha.clone(),
                    None,
                );

                // Register all detected skills as disabled
                for skill_name in &available_skills {
                    let skill_path = if repo_base_path.is_empty() {
                        skill_name.clone()
                    } else {
                        format!("{}/{}", repo_base_path, skill_name)
                    };

                    if !config.has_skill(skill_name) {
                        config.add_skill(skill_name.clone(), repo_id.clone(), skill_path);
                        config.disable_skill(skill_name).ok();
                    }
                }

                Ok(())
            })?;

            println!(
                "Added repository {} ({} skills)",
                style(&repo_id).cyan(),
                available_skills.len()
            );
        }

        // Now enable the requested skills
        for skill in &repo_skills {
            let source_path = get_skill_source_path(skill)?;

            // Verify skill path exists
            if !source_path.exists() {
                eprintln!(
                    "Error: Skill path '{}' does not exist in repository {}",
                    skill.path,
                    repo_id
                );
                had_errors = true;
                continue;
            }

            // Verify SKILL.md exists
            let skill_md_path = source_path.join("SKILL.md");
            if !skill_md_path.exists() {
                eprintln!(
                    "Error: SKILL.md not found in {}. Skills must contain a SKILL.md file.",
                    source_path.display()
                );
                had_errors = true;
                continue;
            }

            // Check if skill already exists
            let config = lock.read_config()?;

            if let Some(existing_skill) = config.skills.get(&skill.skill_name) {
                // Skill exists - check if it's already enabled
                if existing_skill.enabled {
                    println!("Skill {} is already enabled", style(&skill.skill_name).cyan());
                    continue;
                }

                // Skill exists but disabled - enable it
                lock.update(|config| config.enable_skill(&skill.skill_name))?;

                // Create symlinks in all integrations
                let config = lock.read_config()?;
                create_skill_symlinks_for_all_integrations(&source_path, &skill.skill_name, &config)?;

                println!("Enabled {}", style(&skill.skill_name).cyan());
            } else {
                // Skill doesn't exist - check for name clash
                if config.has_skill(&skill.skill_name) {
                    eprintln!(
                        "Error: Skill '{}' already exists. Skill names must be unique across all repositories.",
                        skill.skill_name
                    );
                    had_errors = true;
                    continue;
                }

                // Check that integrations are configured
                require_integrations(&config)?;

                // Create symlinks in all integrations
                create_skill_symlinks_for_all_integrations(&source_path, &skill.skill_name, &config)?;

                lock.update(|config| {
                    config.add_skill(skill.skill_name.clone(), repo_id.clone(), skill.path.clone());
                    Ok(())
                })?;

                println!("Enabled {}", style(&skill.skill_name).cyan());
            }
        }
    }

    // Check for updates on cached repositories
    let config = lock.read_config()?;
    for (repo_id, is_pinned) in repos_to_check_updates {
        if let Some(has_updates) = check_for_updates(&repo_id, &config)? {
            if has_updates {
                println!();
                if is_pinned {
                    println!(
                        "Repository {} has updates available (pinned)",
                        style(&repo_id).cyan()
                    );
                } else {
                    println!(
                        "Repository {} has updates available",
                        style(&repo_id).cyan()
                    );
                }
                println!("Run: sm repo upgrade {}", repo_id);
            }
        }
    }

    if had_errors {
        std::process::exit(1);
    }

    Ok(())
}

pub fn enable(skill_names_or_refs: &[String]) -> Result<()> {
    let lock = ConfigLock::acquire()?;
    enable_with_lock(&lock, skill_names_or_refs)
}

fn enable_with_lock(lock: &ConfigLock, skill_names_or_refs: &[String]) -> Result<()> {
    for skill_name_or_ref in skill_names_or_refs {
        let config = lock.read_config()?;

        // Check if this is just a skill name (already registered)
        if config.has_skill(skill_name_or_ref) {
            // Re-enable existing skill by name
            if config.skills.get(skill_name_or_ref).unwrap().enabled {
                println!(
                    "Skill {} is already enabled",
                    style(skill_name_or_ref).cyan()
                );
                continue;
            }

            lock.update(|config| config.enable_skill(skill_name_or_ref))?;

            // Create symlinks in all integrations
            let config = lock.read_config()?;
            let skill_info = config.skills.get(skill_name_or_ref).unwrap();

            // Parse repository reference and resolve cache path (handles legacy paths)
            let repo_ref = RepoRef::parse(&skill_info.repository)?;
            let repo_cache_path = paths::resolve_repo_cache_path(&repo_ref)?;
            let source_path = repo_cache_path.join(&skill_info.skill_path);
            create_skill_symlinks_for_all_integrations(&source_path, skill_name_or_ref, &config)?;

            println!(
                "Enabled skill {}",
                style(skill_name_or_ref).cyan()
            );
            continue;
        }

        // Not found by name, try parsing as full reference
        let skill = SkillRef::parse(skill_name_or_ref)?;

        // Check if skill already exists and is enabled
        if let Some(existing_skill) = config.skills.get(&skill.skill_name) {
            if existing_skill.enabled {
                println!(
                    "Skill {} is already enabled",
                    style(&skill.skill_name).cyan()
                );
                continue;
            } else {
                // Re-enable the skill
                lock.update(|config| config.enable_skill(&skill.skill_name))?;

                // Create symlinks in all integrations
                let config = lock.read_config()?;
                let source_path = get_skill_source_path(&skill)?;
                create_skill_symlinks_for_all_integrations(&source_path, &skill.skill_name, &config)?;

                println!(
                    "Enabled skill {}",
                    style(&skill.skill_name).cyan()
                );
                continue;
            }
        }

        // Skill doesn't exist yet - need to add it

        // First, ensure the repository is registered
        let repo_id = skill.repo_id();
        if !config.has_repository(&repo_id) {
            println!(
                "Repository {} not found. Adding it first...",
                style(&repo_id).cyan()
            );

            // Clone the repository
            let git_cache = paths::git_cache_dir()?;
            let repo_cache_path = git_cache.join(&skill.owner).join(&skill.repo);

            if !repo_cache_path.exists() {
                git::clone_repo(&skill.git_url(), &repo_cache_path)?;
            }

            // Get current SHA
            let current_sha = git::get_current_sha(&repo_cache_path).ok();

            // Add repository to config and persist
            lock.update(|config| {
                config.add_repository(repo_id.clone(), skill.git_url(), String::new(), current_sha, None);
                Ok(())
            })?;
        }

        // Verify the skill path exists
        let source_path = get_skill_source_path(&skill)?;
        if !source_path.exists() {
            bail!(
                "Skill path '{}' does not exist in repository {}",
                skill.path,
                skill.repo_id()
            );
        }

        // Verify SKILL.md exists
        let skill_md_path = source_path.join("SKILL.md");
        if !skill_md_path.exists() {
            bail!(
                "SKILL.md not found in {}. Skills must contain a SKILL.md file.",
                source_path.display()
            );
        }

        // Check for skill name clash
        if config.has_skill(&skill.skill_name) {
            bail!(
                "Skill '{}' already exists. Skill names must be unique across all repositories.",
                skill.skill_name
            );
        }

        // Check that integrations are configured
        require_integrations(&config)?;

        // Create symlinks in all integrations
        create_skill_symlinks_for_all_integrations(&source_path, &skill.skill_name, &config)?;

        // Add skill to config
        lock.update(|config| {
            config.add_skill(skill.skill_name.clone(), repo_id.clone(), skill.path.clone());
            Ok(())
        })?;

        println!(
            "Enabled skill {}",
            style(&skill.skill_name).cyan()
        );
    }

    Ok(())
}

pub fn disable(skill_names_or_refs: &[String]) -> Result<()> {
    let lock = ConfigLock::acquire()?;
    disable_with_lock(&lock, skill_names_or_refs)
}

fn disable_with_lock(lock: &ConfigLock, skill_names_or_refs: &[String]) -> Result<()> {
    for skill_name_or_ref in skill_names_or_refs {
        let config = lock.read_config()?;

        // Try to find skill by exact name first, otherwise parse as reference
        let skill_name = if config.has_skill(skill_name_or_ref) {
            skill_name_or_ref.to_string()
        } else {
            // Try parsing as a full reference
            let skill = SkillRef::parse(skill_name_or_ref)?;
            skill.skill_name
        };

        if !config.has_skill(&skill_name) {
            bail!(
                "Skill '{}' not found. Use 'sm skills list' to see registered skills.",
                skill_name
            );
        }

        let existing_skill = config.skills.get(&skill_name).unwrap();
        if !existing_skill.enabled {
            println!(
                "Skill {} is already disabled",
                style(&skill_name).dim()
            );
            continue;
        }

        // Remove symlinks from all integrations
        remove_skill_symlinks_from_all_integrations(&skill_name, &config);

        // Update config
        lock.update(|config| config.disable_skill(&skill_name))?;

        println!(
            "Disabled skill {}",
            style(&skill_name).cyan()
        );
    }

    Ok(())
}

pub fn list(all: bool, status: Option<&str>, name_only: bool) -> Result<()> {
    list_skills_only(all, status, name_only)
}

/// List only skills (used by `sm skills list`)
pub fn list_skills_only(all: bool, status: Option<&str>, name_only: bool) -> Result<()> {
    let lock = ConfigLock::acquire()?;
    let config = lock.read_config()?;

    if config.skills.is_empty() {
        println!("No skills registered.");
        return Ok(());
    }

    // Validate status filter if provided
    if let Some(s) = status {
        if s != "enabled" && s != "disabled" {
            bail!("Invalid status '{}'. Use 'enabled' or 'disabled'.", s);
        }
    }

    // Collect and sort skills by name
    let mut skills: Vec<_> = config.skills.iter().collect();
    skills.sort_by_key(|(name, _)| *name);

    // Filter skills based on flags
    let filtered_skills: Vec<_> = skills
        .into_iter()
        .filter(|(_, skill)| {
            // If --all is set, show all
            if all {
                return true;
            }

            // If --status is set, filter by status
            if let Some(s) = status {
                return if s == "enabled" {
                    skill.enabled
                } else {
                    !skill.enabled
                };
            }

            // Default: show only enabled skills
            skill.enabled
        })
        .collect();

    if filtered_skills.is_empty() {
        if status.is_some() {
            println!("No {} skills found.", status.unwrap());
        } else if all {
            println!("No skills found.");
        } else {
            println!("No enabled skills found.");
            println!();
            println!("{}", style("To see all skills use: sm skills list --all").dim());
        }
        return Ok(());
    }

    // Output format: name-only or table
    if name_only {
        for (name, _) in filtered_skills {
            println!("{}", name);
        }
    } else {
        // Print header
        println!(
            "{:<30}  {:<10}  {}",
            style("SKILL").bold(),
            style("STATUS").bold(),
            style("REPOSITORY").bold()
        );

        // Print separator
        println!("{}", "-".repeat(80));

        // Print each skill
        for (name, skill) in filtered_skills {
            let status = if skill.enabled {
                style("enabled").green()
            } else {
                style("disabled").dim()
            };

            println!(
                "{:<30}  {:<10}  {}",
                style(name).cyan(),
                status,
                style(&skill.repository).dim()
            );
        }

        // Show helper message when default view (enabled only) is shown
        if !all && status.is_none() {
            println!();
            println!(
                "{}",
                style("To see all skills use: sm skills list --all").dim()
            );
        }
    }

    Ok(())
}

/// Combined list of skills and agents (used by `sm list`)
pub fn list_combined(all: bool, status: Option<&str>, name_only: bool, skills_only: bool, agents_only: bool) -> Result<()> {
    let lock = ConfigLock::acquire()?;
    let config = lock.read_config()?;

    let has_skills = !config.skills.is_empty();
    let has_agents = !config.agents.is_empty();

    if !has_skills && !has_agents {
        println!("No skills or agents registered.");
        return Ok(());
    }

    // Validate status filter if provided
    if let Some(s) = status {
        if s != "enabled" && s != "disabled" {
            bail!("Invalid status '{}'. Use 'enabled' or 'disabled'.", s);
        }
    }

    // Collect items: (type, name, enabled, repository)
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
    enum ItemType {
        Agent,
        Skill,
    }

    struct Item {
        item_type: ItemType,
        name: String,
        enabled: bool,
        repository: String,
    }

    let mut items: Vec<Item> = Vec::new();

    // Determine what to show: if both flags are set, show both (like no flags)
    let show_skills = !agents_only || (skills_only && agents_only);
    let show_agents = !skills_only || (skills_only && agents_only);

    // Add skills (unless --agents flag is set, unless both flags are set)
    if show_skills {
        for (name, skill) in &config.skills {
            items.push(Item {
                item_type: ItemType::Skill,
                name: name.clone(),
                enabled: skill.enabled,
                repository: skill.repository.clone(),
            });
        }
    }

    // Add agents (unless --skills flag is set, unless both flags are set)
    if show_agents {
        for (name, agent) in &config.agents {
            items.push(Item {
                item_type: ItemType::Agent,
                name: name.clone(),
                enabled: agent.enabled,
                repository: agent.repository.clone(),
            });
        }
    }

    // Sort by name (then by type for stability)
    items.sort_by(|a, b| {
        a.name.cmp(&b.name).then_with(|| a.item_type.cmp(&b.item_type))
    });

    // Filter items based on flags
    let filtered_items: Vec<_> = items
        .into_iter()
        .filter(|item| {
            // If --all is set, show all
            if all {
                return true;
            }

            // If --status is set, filter by status
            if let Some(s) = status {
                return if s == "enabled" {
                    item.enabled
                } else {
                    !item.enabled
                };
            }

            // Default: show only enabled items
            item.enabled
        })
        .collect();

    if filtered_items.is_empty() {
        let type_desc = if skills_only {
            "skills"
        } else if agents_only {
            "agents"
        } else {
            "skills or agents"
        };

        if status.is_some() {
            println!("No {} {} found.", status.unwrap(), type_desc);
        } else if all {
            println!("No {} found.", type_desc);
        } else {
            println!("No enabled {} found.", type_desc);
            println!();
            println!("{}", style("To see all items use: sm list --all").dim());
        }
        return Ok(());
    }

    // Output format: name-only or table
    if name_only {
        for item in filtered_items {
            println!("{}", item.name);
        }
    } else {
        // Print header
        println!(
            "{:<10}  {:<25}  {:<10}  {}",
            style("TYPE").bold(),
            style("NAME").bold(),
            style("STATUS").bold(),
            style("REPOSITORY").bold()
        );

        // Print separator
        println!("{}", "-".repeat(80));

        // Print each item
        for item in filtered_items {
            let type_str = match item.item_type {
                ItemType::Skill => style("[skill]").blue(),
                ItemType::Agent => style("[agent]").magenta(),
            };

            let status_str = if item.enabled {
                style("enabled").green()
            } else {
                style("disabled").dim()
            };

            println!(
                "{:<10}  {:<25}  {:<10}  {}",
                type_str,
                style(&item.name).cyan(),
                status_str,
                style(&item.repository).dim()
            );
        }

        // Show helper message when default view (enabled only) is shown
        if !all && status.is_none() {
            println!();
            println!(
                "{}",
                style("To see all items use: sm list --all").dim()
            );
        }
    }

    Ok(())
}

pub fn manage() -> Result<()> {
    // Check if running in an interactive terminal
    if !std::io::stdin().is_terminal() {
        bail!("Interactive mode requires a TTY. Please run this command in an interactive terminal.");
    }

    let lock = ConfigLock::acquire()?;
    let config = lock.read_config()?;

    // Check if there are any repositories
    if config.repositories.is_empty() {
        println!("No repositories registered.");
        println!();
        println!("Use 'sm repo add <url>' to add a repository first.");
        return Ok(());
    }

    // Item type for combined skills and agents
    #[derive(Debug, Clone, PartialEq, Eq)]
    enum ItemType {
        Skill,
        Agent,
    }

    #[derive(Debug, Clone)]
    struct Item {
        item_type: ItemType,
        name: String,
        repo_id: String,
        enabled: bool,
    }

    let mut all_items: Vec<Item> = Vec::new();

    for (repo_id, repo_info) in &config.repositories {
        // Parse repo reference to get cache path
        let repo_ref = match RepoRef::parse(repo_id) {
            Ok(r) => r,
            Err(_) => {
                // Try parsing the url from config
                match RepoRef::parse(&repo_info.url) {
                    Ok(r) => r,
                    Err(_) => continue,
                }
            }
        };

        // Use resolve_repo_cache_path to check both new-style and legacy paths
        let repo_cache_path = paths::resolve_repo_cache_path(&repo_ref)?;
        if !repo_cache_path.exists() {
            continue;
        }

        // Determine scan path based on repo's path (subdirectory within repo)
        let scan_path = if repo_info.path.is_empty() {
            repo_cache_path.clone()
        } else {
            repo_cache_path.join(&repo_info.path)
        };

        // Scan for skills
        let skills = scan_for_skills(&scan_path)?;
        for skill_name in skills {
            let enabled = config.skills.get(&skill_name)
                .map(|s| s.enabled)
                .unwrap_or(false);

            all_items.push(Item {
                item_type: ItemType::Skill,
                name: skill_name,
                repo_id: repo_id.clone(),
                enabled,
            });
        }

        // Scan for agents
        let agents = scan_for_agents(&scan_path)?;
        for agent_name in agents {
            let enabled = config.agents.get(&agent_name)
                .map(|a| a.enabled)
                .unwrap_or(false);

            all_items.push(Item {
                item_type: ItemType::Agent,
                name: agent_name,
                repo_id: repo_id.clone(),
                enabled,
            });
        }
    }

    if all_items.is_empty() {
        println!("No skills or agents found in registered repositories.");
        return Ok(());
    }

    // Sort by name, then by type for stability
    all_items.sort_by(|a, b| {
        a.name.cmp(&b.name).then_with(|| {
            match (&a.item_type, &b.item_type) {
                (ItemType::Skill, ItemType::Agent) => std::cmp::Ordering::Less,
                (ItemType::Agent, ItemType::Skill) => std::cmp::Ordering::Greater,
                _ => std::cmp::Ordering::Equal,
            }
        })
    });

    // Build options for multi-select with type prefixes
    let options: Vec<String> = all_items
        .iter()
        .map(|item| {
            let type_prefix = match item.item_type {
                ItemType::Skill => "[skill]",
                ItemType::Agent => "[agent]",
            };
            format!("{} {} ({})", type_prefix, item.name, item.repo_id)
        })
        .collect();

    // Get indices of currently enabled items
    let default_indices: Vec<usize> = all_items
        .iter()
        .enumerate()
        .filter(|(_, item)| item.enabled)
        .map(|(i, _)| i)
        .collect();

    // Show multi-select prompt
    let selected = inquire::MultiSelect::new("Select skills and agents:", options.clone())
        .with_default(&default_indices)
        .with_help_message("↑↓ to move, Space to toggle, Enter to save, Esc to cancel")
        .with_formatter(&|_| String::new())
        .without_filtering()
        .prompt();

    let selected_strings = match selected {
        Ok(selections) => selections,
        Err(_) => {
            println!("Cancelled");
            return Ok(());
        }
    };

    // Determine what changed by comparing formatted strings
    let mut skills_to_enable: Vec<String> = Vec::new();
    let mut skills_to_disable: Vec<String> = Vec::new();
    let mut agents_to_enable: Vec<String> = Vec::new();
    let mut agents_to_disable: Vec<String> = Vec::new();

    for (idx, item) in all_items.iter().enumerate() {
        let formatted = &options[idx];
        let should_be_enabled = selected_strings.contains(formatted);

        match item.item_type {
            ItemType::Skill => {
                if should_be_enabled && !item.enabled {
                    skills_to_enable.push(item.name.clone());
                } else if !should_be_enabled && item.enabled {
                    skills_to_disable.push(item.name.clone());
                }
            }
            ItemType::Agent => {
                if should_be_enabled && !item.enabled {
                    agents_to_enable.push(item.name.clone());
                } else if !should_be_enabled && item.enabled {
                    agents_to_disable.push(item.name.clone());
                }
            }
        }
    }

    // Apply skill changes
    if !skills_to_enable.is_empty() {
        enable_with_lock(&lock, &skills_to_enable)?;
    }
    if !skills_to_disable.is_empty() {
        disable_with_lock(&lock, &skills_to_disable)?;
    }

    // Apply agent changes
    if !agents_to_enable.is_empty() {
        subagent::enable_with_lock(&lock, &agents_to_enable)?;
    }
    if !agents_to_disable.is_empty() {
        subagent::disable_with_lock(&lock, &agents_to_disable)?;
    }

    // Show summary
    let no_changes = skills_to_enable.is_empty()
        && skills_to_disable.is_empty()
        && agents_to_enable.is_empty()
        && agents_to_disable.is_empty();

    if no_changes {
        println!("No changes made");
    }

    Ok(())
}

fn add_interactive(lock: &ConfigLock, skill_refs: &[String]) -> Result<()> {
    // Check if running in an interactive terminal
    if !std::io::stdin().is_terminal() {
        bail!("Interactive mode requires a TTY. Please run this command in an interactive terminal.");
    }

    // Validate exactly one argument (the repository URL)
    if skill_refs.len() != 1 {
        bail!("Interactive mode requires exactly one repository URL.\nUsage: sm add -i <repository-url>");
    }

    let repo_url = &skill_refs[0];

    // Parse as repository reference
    let repo_ref = RepoRef::parse(repo_url)?;
    let repo_id = repo_ref.repo_id.clone();

    let config = lock.read_config()?;
    let git_cache = paths::git_cache_dir()?;
    let repo_cache_path = git_cache.join(repo_ref.cache_path());

    // Add repository if it doesn't exist
    if !config.has_repository(&repo_id) {
        // Check for conflicts with same git repo, different path (only for remote repos)
        if repo_ref.source_type != crate::skill_ref::GitSourceType::Local {
            let base_repo_id = repo_ref.repo_id.split('/').take(3).collect::<Vec<_>>().join("/");
            for (existing_repo_id, _) in &config.repositories {
                if existing_repo_id.starts_with(&base_repo_id)
                    && existing_repo_id != &repo_id {
                    bail!(
                        "Cannot add {}. A different path from the same git repository is already registered: {}\nOnly one skill repository per git repository is allowed.",
                        repo_id,
                        existing_repo_id
                    );
                }
            }
        }

        // Clone the repository if needed
        if !repo_cache_path.exists() {
            println!("Cloning repository {}...", style(&repo_id).cyan());
            git::clone_repo(repo_ref.clone_url(), &repo_cache_path)?;
        }

        // Get current SHA
        let current_sha = git::get_current_sha(&repo_cache_path).ok();

        // Scan for skills before adding repository
        let scan_path = if repo_ref.path.is_empty() {
            repo_cache_path.clone()
        } else {
            repo_cache_path.join(&repo_ref.path)
        };

        let available_skills = scan_for_skills(&scan_path)?;

        if available_skills.is_empty() {
            println!("No skills found in repository");
            return Ok(());
        }

        // Add repository to config and register all skills as disabled
        lock.update(|config| {
            config.add_repository(repo_id.clone(), repo_ref.clone_url().to_string(), repo_ref.path.clone(), current_sha, None);

            // Register all detected skills as disabled
            for skill_name in &available_skills {
                let skill_path = if repo_ref.path.is_empty() {
                    skill_name.clone()
                } else {
                    format!("{}/{}", repo_ref.path, skill_name)
                };

                if !config.has_skill(skill_name) {
                    config.add_skill(skill_name.clone(), repo_id.clone(), skill_path);
                    config.disable_skill(skill_name).ok();
                }
            }

            Ok(())
        })?;

        println!("Added repository {} ({} skills)", style(&repo_id).cyan(), available_skills.len());
    } else {
        println!("Repository {} already registered", style(&repo_id).cyan());
    }

    // Scan for skills to show in interactive UI
    let scan_path = if repo_ref.path.is_empty() {
        repo_cache_path.clone()
    } else {
        repo_cache_path.join(&repo_ref.path)
    };

    let skill_names = scan_for_skills(&scan_path)?;

    // Build skill items for this repository
    #[derive(Debug, Clone)]
    struct SkillItem {
        name: String,
        enabled: bool,
    }

    let config = lock.read_config()?;
    let mut skills: Vec<SkillItem> = Vec::new();

    for skill_name in skill_names {
        let enabled = config.skills.get(&skill_name)
            .map(|s| s.enabled)
            .unwrap_or(false);

        skills.push(SkillItem {
            name: skill_name,
            enabled,
        });
    }

    // Sort skills by name
    skills.sort_by(|a, b| a.name.cmp(&b.name));

    // Build options for multi-select
    let options: Vec<String> = skills
        .iter()
        .map(|s| s.name.clone())
        .collect();

    // Get indices of currently enabled skills
    let default_indices: Vec<usize> = skills
        .iter()
        .enumerate()
        .filter(|(_, s)| s.enabled)
        .map(|(i, _)| i)
        .collect();

    // Show multi-select prompt
    let selected = inquire::MultiSelect::new("Select skills:", options.clone())
        .with_default(&default_indices)
        .with_help_message("↑↓ to move, Space to toggle, Enter to save, Esc to cancel")
        .with_formatter(&|_| String::new())
        .without_filtering()
        .prompt();

    let selected_names = match selected {
        Ok(selections) => selections,
        Err(_) => {
            println!("Cancelled");
            return Ok(());
        }
    };

    // Determine what changed
    let mut to_enable: Vec<String> = Vec::new();
    let mut to_disable: Vec<String> = Vec::new();

    for skill in &skills {
        let should_be_enabled = selected_names.contains(&skill.name);

        if should_be_enabled && !skill.enabled {
            to_enable.push(skill.name.clone());
        } else if !should_be_enabled && skill.enabled {
            to_disable.push(skill.name.clone());
        }
    }

    // Apply changes
    if !to_enable.is_empty() {
        enable_with_lock(lock, &to_enable)?;
    }

    if !to_disable.is_empty() {
        disable_with_lock(lock, &to_disable)?;
    }

    // Show summary
    if to_enable.is_empty() && to_disable.is_empty() {
        println!("No changes made");
    }

    Ok(())
}

// Helper functions

fn get_skill_source_path(skill: &SkillRef) -> Result<PathBuf> {
    let git_cache = paths::git_cache_dir()?;
    let repo_cache_path = git_cache.join(&skill.owner).join(&skill.repo);
    Ok(repo_cache_path.join(&skill.path))
}

pub fn create_skill_symlink(source: &PathBuf, link: &PathBuf) -> Result<()> {
    if link.exists() {
        // Remove existing symlink/file
        if link.is_symlink() {
            std::fs::remove_file(link)?;
        } else {
            bail!(
                "Path {} already exists and is not a symlink",
                style(link.display()).yellow()
            );
        }
    }

    unix_fs::symlink(source, link)
        .context("Failed to create symlink")?;

    Ok(())
}

/// Create symlinks for a skill in all registered integrations
/// Returns true if at least one symlink was created successfully
fn create_skill_symlinks_for_all_integrations(
    source: &PathBuf,
    skill_name: &str,
    config: &Config,
) -> Result<bool> {
    if config.integrations.is_empty() {
        bail!(
            "No integrations configured.\n\
             Run {} to set up integrations, or add one manually:\n  \
             sm integrations add claude-code",
            style("sm configure").cyan()
        );
    }

    let mut success_count = 0;
    let mut errors: Vec<(String, String)> = Vec::new();

    for (name, integration) in &config.integrations {
        // Skip integrations that don't have a skills directory
        let skills_dir_str = match &integration.skills_dir {
            Some(dir) => dir,
            None => continue,
        };
        let skills_dir = PathBuf::from(skills_dir_str);

        // Create directory if it doesn't exist
        if !skills_dir.exists() {
            if let Err(e) = std::fs::create_dir_all(&skills_dir) {
                errors.push((name.clone(), format!("Failed to create directory: {}", e)));
                continue;
            }
        }

        let link_path = skills_dir.join(skill_name);
        match create_skill_symlink(source, &link_path) {
            Ok(_) => success_count += 1,
            Err(e) => errors.push((name.clone(), e.to_string())),
        }
    }

    // Report any errors
    for (name, error) in &errors {
        eprintln!(
            "  {} {}: {}",
            style("!").yellow(),
            name,
            error
        );
    }

    Ok(success_count > 0)
}

/// Remove symlinks for a skill from all registered integrations
pub fn remove_skill_symlinks_from_all_integrations(
    skill_name: &str,
    config: &Config,
) {
    for (name, integration) in &config.integrations {
        // Skip integrations that don't have a skills directory
        let skills_dir_str = match &integration.skills_dir {
            Some(dir) => dir,
            None => continue,
        };
        let skills_dir = PathBuf::from(skills_dir_str);
        let link_path = skills_dir.join(skill_name);

        if link_path.is_symlink() {
            if let Err(e) = std::fs::remove_file(&link_path) {
                eprintln!(
                    "  {} {}: Failed to remove symlink: {}",
                    style("!").yellow(),
                    name,
                    e
                );
            }
        }
    }
}

/// Check that at least one integration is configured
fn require_integrations(config: &Config) -> Result<()> {
    if config.integrations.is_empty() {
        bail!(
            "No integrations configured.\n\
             Run {} to set up integrations, or add one manually:\n  \
             sm integrations add claude-code",
            style("sm configure").cyan()
        );
    }
    Ok(())
}

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

/// Scan a directory for agents (directories containing AGENT.md)
fn scan_for_agents(path: &Path) -> Result<Vec<String>> {
    let mut agents = Vec::new();

    if !path.exists() {
        return Ok(agents);
    }

    // Read directory entries
    let entries = std::fs::read_dir(path).context("Failed to read directory")?;

    for entry in entries {
        let entry = entry?;
        let entry_path = entry.path();

        // Check if it's a directory
        if entry_path.is_dir() {
            // Check if it contains AGENT.md
            let agent_md = entry_path.join("AGENT.md");
            if agent_md.exists() {
                if let Some(agent_name) = entry_path.file_name() {
                    agents.push(agent_name.to_string_lossy().to_string());
                }
            }
        }
    }

    agents.sort();
    Ok(agents)
}

fn check_for_updates(repo_id: &str, config: &crate::config::state::Config) -> Result<Option<bool>> {
    let repo = match config.repositories.get(repo_id) {
        Some(r) => r,
        None => return Ok(None),
    };

    let current_sha = match &repo.current_sha {
        Some(sha) => sha,
        None => return Ok(None),
    };

    // Parse repo_id to get owner/repo
    let parts: Vec<&str> = repo_id.split('/').collect();
    if parts.len() < 2 {
        return Ok(None);
    }

    let git_cache = paths::git_cache_dir()?;
    let repo_cache_path = git_cache.join(parts[0]).join(parts[1]);

    if !repo_cache_path.exists() {
        return Ok(None);
    }

    // Fetch latest from origin
    let fetch_output = std::process::Command::new("git")
        .arg("fetch")
        .arg("origin")
        .arg("--quiet")
        .current_dir(&repo_cache_path)
        .output();

    if fetch_output.is_err() {
        return Ok(None);
    }

    // Get the default branch
    let branch_output = std::process::Command::new("git")
        .arg("symbolic-ref")
        .arg("refs/remotes/origin/HEAD")
        .arg("--short")
        .current_dir(&repo_cache_path)
        .output();

    let default_branch = if let Ok(output) = branch_output {
        if output.status.success() {
            String::from_utf8_lossy(&output.stdout)
                .trim()
                .to_string()
        } else {
            "origin/main".to_string()
        }
    } else {
        "origin/main".to_string()
    };

    // Get SHA of remote branch
    let remote_sha_output = std::process::Command::new("git")
        .arg("rev-parse")
        .arg("--short=12")
        .arg(&default_branch)
        .current_dir(&repo_cache_path)
        .output();

    if let Ok(output) = remote_sha_output {
        if output.status.success() {
            let remote_sha = String::from_utf8_lossy(&output.stdout).trim().to_string();
            return Ok(Some(current_sha != &remote_sha));
        }
    }

    Ok(None)
}
