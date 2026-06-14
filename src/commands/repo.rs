use anyhow::{Context, Result, bail};
use console::style;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use crate::commands::skill::remove_skill_symlinks_from_all_integrations;
use crate::commands::subagent::remove_agent_symlinks_from_all_integrations;
use crate::config::ConfigLock;
use crate::git;
use crate::paths;
use crate::skill_ref::RepoRef;

pub fn add(urls: &[String]) -> Result<()> {
    if urls.is_empty() {
        bail!("At least one repository URL is required");
    }

    let lock = ConfigLock::acquire()?;
    let mut had_errors = false;
    let mut success_count = 0;

    for url in urls {
        if let Err(e) = add_single(&lock, url) {
            eprintln!("Error adding {}: {}", url, e);
            had_errors = true;
        } else {
            success_count += 1;
        }
    }

    // Show summary
    if urls.len() > 1 && had_errors {
        eprintln!("Added {}/{} repositories", success_count, urls.len());
    }

    if had_errors {
        std::process::exit(1);
    }

    Ok(())
}

fn add_single(lock: &ConfigLock, url: &str) -> Result<()> {
    let repo_ref = RepoRef::parse(url)?;

    // Check if repository already exists
    let config = lock.read_config()?;
    if config.has_repository(&repo_ref.repo_id) {
        bail!(
            "Repository '{}' is already registered. Use 'sm repo list' to see all repositories.",
            repo_ref.repo_id
        );
    }

    // Check for conflicts with same git repo, different path
    // Only check for remote repos - local paths are always unique
    if repo_ref.source_type != crate::skill_ref::GitSourceType::Local {
        // Extract the base repo (without subpath) for conflict checking
        let base_repo_id = repo_ref
            .repo_id
            .split('/')
            .take(3)
            .collect::<Vec<_>>()
            .join("/");
        for existing_repo_id in config.repositories.keys() {
            if existing_repo_id.starts_with(&base_repo_id) && existing_repo_id != &repo_ref.repo_id
            {
                bail!(
                    "Cannot add {}. A different path from the same git repository is already registered: {}\nOnly one skill repository per git repository is allowed.",
                    repo_ref.repo_id,
                    existing_repo_id
                );
            }
        }
    }

    let is_local = repo_ref.source_type == crate::skill_ref::GitSourceType::Local;

    // For local repos, use the path directly; for remote repos, clone to cache
    let (repo_cache_path, current_sha, pinned_sha) = if is_local {
        let local_path = PathBuf::from(&repo_ref.git_url);
        if !local_path.exists() {
            bail!("Local path '{}' does not exist", repo_ref.git_url);
        }
        (local_path, None, None)
    } else {
        // Determine where to clone the repo
        let repo_cache_path = paths::repo_cache_path(&repo_ref)?;

        // Clone the repository if not already cloned
        if !repo_cache_path.exists() {
            println!("Cloning repository {}...", style(&repo_ref.repo_id).cyan());
            git::clone_repo(repo_ref.clone_url(), &repo_cache_path)?;
        }

        // Handle SHA pinning if specified
        let (current_sha, pinned_sha) = if let Some(sha) = &repo_ref.sha {
            // Checkout the specific SHA
            println!("Checking out commit {}...", style(sha).yellow());
            git::checkout_sha(&repo_cache_path, sha)?;
            let actual_sha = git::get_current_sha(&repo_cache_path)?;
            println!(
                "{} Pinned to {}",
                style("✓").green(),
                style(&actual_sha).cyan()
            );
            (Some(actual_sha.clone()), Some(actual_sha))
        } else {
            // No SHA specified, just get current SHA
            let sha = git::get_current_sha(&repo_cache_path)?;
            (Some(sha), None)
        };

        (repo_cache_path, current_sha, pinned_sha)
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
            repo_ref.repo_id
        );
    }

    // Scan for skills and agents in the repository
    let available_skills = scan_for_skills(&full_path)?;
    let available_agents = scan_for_agents(&full_path)?;

    // Add repository and register all skills and agents as disabled
    lock.update(|config| {
        config.add_repository(
            repo_ref.repo_id.clone(),
            repo_ref.clone_url().to_string(),
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
                config.add_skill(skill_name.clone(), repo_ref.repo_id.clone(), skill_path);
                // Immediately disable it (add_skill enables by default)
                config.disable_skill(skill_name).ok();
            }
        }

        // Register all detected agents as disabled
        for agent_name in &available_agents {
            let agent_path = if repo_ref.path.is_empty() {
                agent_name.clone()
            } else {
                format!("{}/{}", repo_ref.path, agent_name)
            };

            // Only add if not already registered
            if !config.has_agent(agent_name) {
                config.add_agent(agent_name.clone(), repo_ref.repo_id.clone(), agent_path);
                // Immediately disable it (add_agent enables by default)
                config.disable_agent(agent_name).ok();
            }
        }

        Ok(())
    })?;

    // Format the output message based on what was discovered
    let items_discovered = match (available_skills.len(), available_agents.len()) {
        (0, 0) => "no skills or agents".to_string(),
        (s, 0) => format!("{} skill{}", s, if s == 1 { "" } else { "s" }),
        (0, a) => format!("{} agent{}", a, if a == 1 { "" } else { "s" }),
        (s, a) => format!(
            "{} skill{}, {} agent{}",
            s,
            if s == 1 { "" } else { "s" },
            a,
            if a == 1 { "" } else { "s" }
        ),
    };

    println!(
        "Added repository {} ({})",
        style(&repo_ref.repo_id).cyan(),
        items_discovered
    );

    Ok(())
}

pub fn delete(urls: &[String], force: bool) -> Result<()> {
    if urls.is_empty() {
        bail!("At least one repository URL is required");
    }

    let lock = ConfigLock::acquire()?;
    let mut had_errors = false;
    let mut success_count = 0;

    for url in urls {
        if let Err(e) = delete_single(&lock, url, force) {
            eprintln!("Error removing {}: {}", url, e);
            had_errors = true;
        } else {
            success_count += 1;
        }
    }

    // Show summary
    if urls.len() > 1 {
        eprintln!("Removed {}/{} repositories", success_count, urls.len());
    }

    if had_errors {
        std::process::exit(1);
    }

    Ok(())
}

fn delete_single(lock: &ConfigLock, url: &str, force: bool) -> Result<()> {
    let repo_ref = RepoRef::parse(url)?;

    let config = lock.read_config()?;

    // Check if repository exists
    if !config.has_repository(&repo_ref.repo_id) {
        bail!(
            "Repository '{}' not found. Use 'sm repo list' to see registered repositories.",
            repo_ref.repo_id
        );
    }

    // Check if any skills or agents are using this repository
    let skills = config.skills_for_repo(&repo_ref.repo_id);
    let agents = config.agents_for_repo(&repo_ref.repo_id);
    let enabled_skills_count = skills.iter().filter(|s| s.enabled).count();
    let enabled_agents_count = agents.iter().filter(|a| a.enabled).count();
    let total_enabled = enabled_skills_count + enabled_agents_count;

    if total_enabled > 0 && !force {
        // Build warning message showing what's enabled
        let mut enabled_items = Vec::new();
        if enabled_skills_count > 0 {
            enabled_items.push(format!(
                "{} skill{}",
                enabled_skills_count,
                if enabled_skills_count == 1 { "" } else { "s" }
            ));
        }
        if enabled_agents_count > 0 {
            enabled_items.push(format!(
                "{} agent{}",
                enabled_agents_count,
                if enabled_agents_count == 1 { "" } else { "s" }
            ));
        }

        println!(
            "{} {} enabled from {} are registered.",
            style("Warning:").yellow(),
            enabled_items.join(" and "),
            style(&repo_ref.repo_id).cyan()
        );
        print!("Are you sure you want to remove them? (yes/no): ");
        io::stdout().flush()?;

        let mut response = String::new();
        io::stdin().read_line(&mut response)?;

        if response.trim().to_lowercase() != "yes" {
            println!("Cancelled.");
            return Ok(());
        }
    }

    // Collect skill names for this repository
    let skill_names: Vec<String> = skills
        .iter()
        .filter_map(|s| {
            config
                .skills
                .iter()
                .find(|(_, skill)| *skill == *s)
                .map(|(name, _)| name.clone())
        })
        .collect();

    // Collect agent names for this repository
    let agent_names: Vec<String> = agents
        .iter()
        .filter_map(|a| {
            config
                .agents
                .iter()
                .find(|(_, agent)| *agent == *a)
                .map(|(name, _)| name.clone())
        })
        .collect();

    // Remove skill symlinks from all integrations
    for skill_name in &skill_names {
        remove_skill_symlinks_from_all_integrations(skill_name, &config);
    }

    // Remove agent symlinks from all integrations
    for agent_name in &agent_names {
        remove_agent_symlinks_from_all_integrations(agent_name, &config);
    }

    lock.update(|config| {
        // Remove all skills
        for skill_name in &skill_names {
            config.remove_skill(skill_name);
        }

        // Remove all agents
        for agent_name in &agent_names {
            config.remove_agent(agent_name);
        }

        // Remove repository
        config.remove_repository(&repo_ref.repo_id);

        Ok(())
    })?;

    // Format output message based on what was removed
    let items_removed = match (skill_names.len(), agent_names.len()) {
        (0, 0) => "".to_string(),
        (s, 0) => format!(" and {} skill{}", s, if s == 1 { "" } else { "s" }),
        (0, a) => format!(" and {} agent{}", a, if a == 1 { "" } else { "s" }),
        (s, a) => format!(
            " and {} skill{}, {} agent{}",
            s,
            if s == 1 { "" } else { "s" },
            a,
            if a == 1 { "" } else { "s" }
        ),
    };

    println!(
        "Removed repository {}{}",
        style(&repo_ref.repo_id).cyan(),
        items_removed
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
        "{:<8}  {:<40}  {:>10}  {:>10}  {:<10}  {:<10}",
        style("SOURCE").bold(),
        style("REPOSITORY").bold(),
        style("SKILLS").bold(),
        style("AGENTS").bold(),
        style("CURRENT").bold(),
        style("PIN").bold()
    );

    // Print separator
    println!("{}", "-".repeat(100));

    // Print each repository
    for (repo_id, repo) in &config.repositories {
        // Determine source type from the URL
        let source_type = crate::skill_ref::GitSourceType::from_url(&repo.url).label();
        let source_display = format!("[{}]", source_type);

        // Get all skills for this repository
        let skills = config.skills_for_repo(repo_id);
        let skills_total = skills.len();
        let skills_enabled = skills.iter().filter(|s| s.enabled).count();

        // Get all agents for this repository
        let agents = config.agents_for_repo(repo_id);
        let agents_total = agents.len();
        let agents_enabled = agents.iter().filter(|a| a.enabled).count();

        // Format as "enabled/total" ratio
        let skills_display = format!("{}/{}", skills_enabled, skills_total);
        let agents_display = format!("{}/{}", agents_enabled, agents_total);

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
            "{:<8}  {:<40}  {:>10}  {:>10}  {:<10}  {}",
            style(&source_display).magenta(),
            style(repo_id).cyan(),
            style(&skills_display).green(),
            style(&agents_display).blue(),
            style(&current_sha_display).dim(),
            pinned_sha_display
        );
    }

    Ok(())
}

pub fn pin(url: &str) -> Result<()> {
    let repo_ref = RepoRef::parse(url)?;

    if repo_ref.source_type == crate::skill_ref::GitSourceType::Local {
        bail!("Local repositories cannot be pinned");
    }

    let lock = ConfigLock::acquire()?;

    // Check if repository exists
    let config = lock.read_config()?;
    if !config.has_repository(&repo_ref.repo_id) {
        bail!(
            "Repository '{}' is not registered. Use 'sm repo list' to see all repositories.",
            repo_ref.repo_id
        );
    }

    // Get current SHA from git working directory
    // Use resolve_repo_cache_path to handle legacy cache paths
    let repo_cache_path = paths::resolve_repo_cache_path(&repo_ref)?;

    let current_sha = git::get_current_sha(&repo_cache_path)?;

    // Update repository to pin it
    lock.update(|config| {
        if let Some(repo) = config.repositories.get_mut(&repo_ref.repo_id) {
            repo.pinned_sha = Some(current_sha.clone());
            repo.current_sha = Some(current_sha.clone());
        }
        Ok(())
    })?;

    println!(
        "Pinned {} at {}",
        style(&repo_ref.repo_id).cyan(),
        style(&current_sha).yellow()
    );

    Ok(())
}

pub fn unpin(url: &str) -> Result<()> {
    let repo_ref = RepoRef::parse(url)?;

    if repo_ref.source_type == crate::skill_ref::GitSourceType::Local {
        bail!("Local repositories cannot be unpinned");
    }

    let lock = ConfigLock::acquire()?;

    // Check if repository exists
    let config = lock.read_config()?;
    if !config.has_repository(&repo_ref.repo_id) {
        bail!(
            "Repository '{}' is not registered. Use 'sm repo list' to see all repositories.",
            repo_ref.repo_id
        );
    }

    let current_sha = config
        .repositories
        .get(&repo_ref.repo_id)
        .and_then(|r| r.current_sha.clone())
        .unwrap_or_else(|| "unknown".to_string());

    // Update repository to unpin it
    lock.update(|config| {
        if let Some(repo) = config.repositories.get_mut(&repo_ref.repo_id) {
            repo.pinned_sha = None;
        }
        Ok(())
    })?;

    println!(
        "Unpinned {} (currently at {})",
        style(&repo_ref.repo_id).cyan(),
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

    // Safety net: an empty scan while skills are already registered almost always
    // means the scan ran against the wrong directory (e.g. a missing or incorrect
    // subdirectory) rather than a genuine upstream deletion of every skill. Skip
    // the mass-removal and warn instead of silently wiping the repo's skills.
    if new_skills.is_empty() && !existing_skills.is_empty() {
        eprintln!(
            "{} Skipped removing {} skill(s) from '{}': none found on disk (check the repository path).",
            style("⚠").yellow(),
            existing_skills.len(),
            repo_id
        );
        return Ok((removed_skills, added_skills));
    }

    // Find deleted skills (exist in config but not in filesystem)
    for (skill_name, _skill) in &existing_skills {
        if !new_skills_set.contains(skill_name) {
            // Skill was deleted from repository
            removed_skills.push(skill_name.clone());

            // Remove symlinks from all integrations
            for integration in config.integrations.values() {
                if let Some(ref skills_dir_str) = integration.skills_dir {
                    let skills_dir = PathBuf::from(skills_dir_str);
                    let link_path = skills_dir.join(skill_name);
                    if link_path.exists() || link_path.symlink_metadata().is_ok() {
                        std::fs::remove_file(&link_path).ok();
                    }
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

            config.add_skill(skill_name.clone(), repo_id.to_string(), skill_path);
            config.disable_skill(skill_name).ok();
        }
    }

    Ok((removed_skills, added_skills))
}

/// Reconcile agents after repository upgrade - handle deleted/new agents
fn reconcile_agents(
    config: &mut crate::config::state::Config,
    repo_id: &str,
    new_agents: &[String],
    repo_path: &str,
) -> Result<(Vec<String>, Vec<String>)> {
    use std::collections::HashSet;

    let new_agents_set: HashSet<_> = new_agents.iter().collect();

    // Get existing agents for this repository
    let existing_agents: Vec<_> = config
        .agents
        .iter()
        .filter(|(_, agent)| agent.repository == repo_id)
        .map(|(name, agent)| (name.clone(), agent.clone()))
        .collect();

    let mut removed_agents = Vec::new();
    let mut added_agents = Vec::new();

    // Safety net (see reconcile_skills): an empty scan with agents already
    // registered almost always means the scan hit the wrong directory, not a
    // genuine deletion. Skip mass-removal and warn rather than wipe.
    if new_agents.is_empty() && !existing_agents.is_empty() {
        eprintln!(
            "{} Skipped removing {} agent(s) from '{}': none found on disk (check the repository path).",
            style("⚠").yellow(),
            existing_agents.len(),
            repo_id
        );
        return Ok((removed_agents, added_agents));
    }

    // Find deleted agents (exist in config but not in filesystem)
    for (agent_name, _agent) in &existing_agents {
        if !new_agents_set.contains(agent_name) {
            // Agent was deleted from repository
            removed_agents.push(agent_name.clone());

            // Remove symlinks from all integrations
            for integration in config.integrations.values() {
                if let Some(ref agents_dir_str) = integration.agents_dir {
                    let agents_dir = PathBuf::from(agents_dir_str);
                    let link_path = agents_dir.join(format!("{}.md", agent_name));
                    if link_path.exists() || link_path.symlink_metadata().is_ok() {
                        std::fs::remove_file(&link_path).ok();
                    }
                }
            }

            // Remove from config entirely
            config.remove_agent(agent_name);
        }
    }

    // Find new agents (exist in filesystem but not in config)
    for agent_name in new_agents {
        if !existing_agents.iter().any(|(name, _)| name == agent_name) {
            added_agents.push(agent_name.clone());

            // Add to config as disabled
            let agent_path = if repo_path.is_empty() {
                agent_name.clone()
            } else {
                format!("{}/{}", repo_path, agent_name)
            };

            config.add_agent(agent_name.clone(), repo_id.to_string(), agent_path);
            config.disable_agent(agent_name).ok();
        }
    }

    Ok((removed_agents, added_agents))
}

pub fn upgrade(url: &str) -> Result<()> {
    // Parse the user input only to resolve the repo id and an optional @sha
    // override; the clone url and subdirectory path come from the stored record
    // so the configured subdirectory is never lost (see RepoRef::from_stored).
    let parsed = RepoRef::parse(url)?;
    let lock = ConfigLock::acquire()?;

    let mut repo_ref = {
        let config = lock.read_config()?;
        let repo = config.repositories.get(&parsed.repo_id).ok_or_else(|| {
            anyhow::anyhow!(
                "Repository '{}' is not registered. Use 'sm repo list' to see all repositories.",
                parsed.repo_id
            )
        })?;
        RepoRef::from_stored(&parsed.repo_id, &repo.url, &repo.path)
    };
    repo_ref.sha = parsed.sha;

    upgrade_with_lock(&repo_ref, &lock)
}

/// Aggregated skill/agent additions and removals from a repository refresh.
struct RepoChanges {
    removed_skills: Vec<String>,
    added_skills: Vec<String>,
    removed_agents: Vec<String>,
    added_agents: Vec<String>,
}

impl RepoChanges {
    fn has_changes(&self) -> bool {
        !self.removed_skills.is_empty()
            || !self.added_skills.is_empty()
            || !self.removed_agents.is_empty()
            || !self.added_agents.is_empty()
    }
}

/// Join a repository root with its (possibly empty) subdirectory path.
fn join_subdir(root: &Path, subpath: &str) -> PathBuf {
    if subpath.is_empty() {
        root.to_path_buf()
    } else {
        root.join(subpath)
    }
}

/// Re-scan a repository's skills/agents directory and reconcile the config to
/// match what is on disk. Shared by every upgrade path (and, later, auto-upgrade)
/// so the subdirectory handling and reconciliation live in exactly one place.
fn refresh_repo_items(
    config: &mut crate::config::state::Config,
    repo_id: &str,
    repo_root: &Path,
    subpath: &str,
) -> Result<RepoChanges> {
    let full_path = join_subdir(repo_root, subpath);
    let new_skills = scan_for_skills(&full_path)?;
    let new_agents = scan_for_agents(&full_path)?;

    let (removed_skills, added_skills) = reconcile_skills(config, repo_id, &new_skills, subpath)?;
    let (removed_agents, added_agents) = reconcile_agents(config, repo_id, &new_agents, subpath)?;

    Ok(RepoChanges {
        removed_skills,
        added_skills,
        removed_agents,
        added_agents,
    })
}

/// Print the added/removed skills and agents from a refresh in a consistent way.
fn print_repo_changes(changes: &RepoChanges) {
    let removed: Vec<_> = changes
        .removed_skills
        .iter()
        .map(|s| format!("[skill] {}", s))
        .chain(
            changes
                .removed_agents
                .iter()
                .map(|a| format!("[agent] {}", a)),
        )
        .collect();
    if !removed.is_empty() {
        println!(
            "{} Removed: {} (no longer available)",
            style("⚠").yellow(),
            removed.join(", ")
        );
    }

    let added: Vec<_> = changes
        .added_skills
        .iter()
        .map(|s| format!("[skill] {}", s))
        .chain(
            changes
                .added_agents
                .iter()
                .map(|a| format!("[agent] {}", a)),
        )
        .collect();
    if !added.is_empty() {
        println!("{} New: {}", style("✓").green(), added.join(", "));
    }
}

/// Internal upgrade logic that uses an existing lock.
///
/// `repo_ref` must describe an already-registered repository — build it with
/// [`RepoRef::from_stored`] so its `path` reflects the configured subdirectory.
/// Reconstructing it via `RepoRef::parse` would drop the subdirectory and make
/// the rescan look at the wrong place.
fn upgrade_with_lock(repo_ref: &RepoRef, lock: &ConfigLock) -> Result<()> {
    // Check if repository exists
    let config = lock.read_config()?;
    if !config.has_repository(&repo_ref.repo_id) {
        bail!(
            "Repository '{}' is not registered. Use 'sm repo list' to see all repositories.",
            repo_ref.repo_id
        );
    }

    let is_local = repo_ref.source_type == crate::skill_ref::GitSourceType::Local;

    // For local repos: skip git operations, just re-scan and reconcile
    if is_local {
        let repo_root = PathBuf::from(&repo_ref.git_url);

        println!("Scanning {}...", style(&repo_ref.repo_id).cyan());

        let mut changes = None;
        lock.update(|config| {
            changes = Some(refresh_repo_items(
                config,
                &repo_ref.repo_id,
                &repo_root,
                &repo_ref.path,
            )?);
            Ok(())
        })?;
        let changes = changes.expect("refresh_repo_items runs inside lock.update");

        println!(
            "{} Scanned {}",
            style("✓").green(),
            style(&repo_ref.repo_id).cyan()
        );
        print_repo_changes(&changes);
        if !changes.has_changes() {
            println!("{} No changes detected", style("→").dim());
        }

        return Ok(());
    }

    // Use resolve_repo_cache_path to handle legacy cache paths (repos cloned before universal git support)
    let repo_cache_path = paths::resolve_repo_cache_path(repo_ref)?;

    if let Some(target_sha) = &repo_ref.sha {
        // Mode B: Upgrade to specific SHA and pin
        println!(
            "Upgrading {} to {}...",
            style(&repo_ref.repo_id).cyan(),
            style(target_sha).yellow()
        );

        git::checkout_sha(&repo_cache_path, target_sha)?;
        let actual_sha = git::get_current_sha(&repo_cache_path)?;

        let mut changes = None;
        lock.update(|config| {
            if let Some(repo) = config.repositories.get_mut(&repo_ref.repo_id) {
                repo.current_sha = Some(actual_sha.clone());
                repo.pinned_sha = Some(actual_sha.clone());
            }
            changes = Some(refresh_repo_items(
                config,
                &repo_ref.repo_id,
                &repo_cache_path,
                &repo_ref.path,
            )?);
            Ok(())
        })?;
        let changes = changes.expect("refresh_repo_items runs inside lock.update");

        println!(
            "{} Upgraded and pinned to {}",
            style("✓").green(),
            style(&actual_sha).cyan()
        );
        print_repo_changes(&changes);
    } else {
        // Mode A: Upgrade to latest
        let repo = config.repositories.get(&repo_ref.repo_id).unwrap();

        if repo.pinned_sha.is_some() {
            bail!(
                "Repository '{}' is pinned. Use 'sm repo unpin' first or specify a commit with @SHA.",
                repo_ref.repo_id
            );
        }

        let old_sha = repo
            .current_sha
            .clone()
            .unwrap_or_else(|| "unknown".to_string());

        println!("Upgrading {}...", style(&repo_ref.repo_id).cyan());

        let new_sha = git::pull_to_latest(&repo_cache_path)?;

        if old_sha == new_sha {
            println!(
                "{} Already at latest ({})",
                style("→").dim(),
                style(&new_sha).dim()
            );
            return Ok(());
        }

        let mut changes = None;
        lock.update(|config| {
            if let Some(repo) = config.repositories.get_mut(&repo_ref.repo_id) {
                repo.current_sha = Some(new_sha.clone());
            }
            changes = Some(refresh_repo_items(
                config,
                &repo_ref.repo_id,
                &repo_cache_path,
                &repo_ref.path,
            )?);
            Ok(())
        })?;
        let changes = changes.expect("refresh_repo_items runs inside lock.update");

        // old_sha may be the literal "unknown" (no stored sha); guard the slice.
        let old_display = if old_sha.len() >= 8 {
            &old_sha[..8]
        } else {
            old_sha.as_str()
        };
        println!(
            "{} Upgraded {} → {}",
            style("✓").green(),
            style(old_display).dim(),
            style(&new_sha).cyan()
        );
        print_repo_changes(&changes);
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

    for (repo_id, repo) in unpinned_repos {
        // Build the ref from the stored record so the configured subdirectory is
        // preserved (a parsed repo id has no subpath).
        let repo_ref = RepoRef::from_stored(repo_id, &repo.url, &repo.path);
        match upgrade_with_lock(&repo_ref, &lock) {
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
            if skill_md.exists()
                && let Some(skill_name) = entry_path.file_name()
            {
                skills.push(skill_name.to_string_lossy().to_string());
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
            if agent_md.exists()
                && let Some(agent_name) = entry_path.file_name()
            {
                agents.push(agent_name.to_string_lossy().to_string());
            }
        }
    }

    agents.sort();
    Ok(agents)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn create_skill_dir(parent: &Path, name: &str) {
        let skill_dir = parent.join(name);
        std::fs::create_dir_all(&skill_dir).unwrap();
        std::fs::write(skill_dir.join("SKILL.md"), "# Test Skill\n").unwrap();
    }

    fn create_agent_dir(parent: &Path, name: &str) {
        let agent_dir = parent.join(name);
        std::fs::create_dir_all(&agent_dir).unwrap();
        std::fs::write(agent_dir.join("AGENT.md"), "# Test Agent\n").unwrap();
    }

    #[test]
    fn test_scan_for_agents_empty_dir() {
        let temp_dir = TempDir::new().unwrap();
        let agents = scan_for_agents(temp_dir.path()).unwrap();
        assert!(agents.is_empty());
    }

    #[test]
    fn test_scan_for_agents_finds_agents() {
        let temp_dir = TempDir::new().unwrap();

        create_agent_dir(temp_dir.path(), "code-reviewer");
        create_agent_dir(temp_dir.path(), "debugger");

        let agents = scan_for_agents(temp_dir.path()).unwrap();
        assert_eq!(agents.len(), 2);
        assert!(agents.contains(&"code-reviewer".to_string()));
        assert!(agents.contains(&"debugger".to_string()));
    }

    #[test]
    fn test_scan_for_agents_ignores_skills() {
        let temp_dir = TempDir::new().unwrap();

        create_skill_dir(temp_dir.path(), "git-commit");
        create_agent_dir(temp_dir.path(), "code-reviewer");

        let agents = scan_for_agents(temp_dir.path()).unwrap();
        assert_eq!(agents.len(), 1);
        assert!(agents.contains(&"code-reviewer".to_string()));
        assert!(!agents.contains(&"git-commit".to_string()));
    }

    #[test]
    fn test_scan_for_agents_ignores_dirs_without_agent_md() {
        let temp_dir = TempDir::new().unwrap();

        // Create a directory without AGENT.md
        let plain_dir = temp_dir.path().join("plain-dir");
        std::fs::create_dir_all(&plain_dir).unwrap();
        std::fs::write(plain_dir.join("README.md"), "# Plain directory\n").unwrap();

        create_agent_dir(temp_dir.path(), "code-reviewer");

        let agents = scan_for_agents(temp_dir.path()).unwrap();
        assert_eq!(agents.len(), 1);
        assert!(agents.contains(&"code-reviewer".to_string()));
    }

    #[test]
    fn test_scan_for_agents_nonexistent_path() {
        let agents = scan_for_agents(Path::new("/nonexistent/path/12345")).unwrap();
        assert!(agents.is_empty());
    }

    #[test]
    fn test_scan_for_agents_sorted() {
        let temp_dir = TempDir::new().unwrap();

        create_agent_dir(temp_dir.path(), "zebra-agent");
        create_agent_dir(temp_dir.path(), "alpha-agent");
        create_agent_dir(temp_dir.path(), "middle-agent");

        let agents = scan_for_agents(temp_dir.path()).unwrap();
        assert_eq!(agents.len(), 3);
        assert_eq!(agents[0], "alpha-agent");
        assert_eq!(agents[1], "middle-agent");
        assert_eq!(agents[2], "zebra-agent");
    }

    #[test]
    fn test_scan_for_skills_finds_skills() {
        let temp_dir = TempDir::new().unwrap();

        create_skill_dir(temp_dir.path(), "git-commit");
        create_skill_dir(temp_dir.path(), "pdf-reader");

        let skills = scan_for_skills(temp_dir.path()).unwrap();
        assert_eq!(skills.len(), 2);
        assert!(skills.contains(&"git-commit".to_string()));
        assert!(skills.contains(&"pdf-reader".to_string()));
    }

    #[test]
    fn test_scan_for_skills_ignores_agents() {
        let temp_dir = TempDir::new().unwrap();

        create_skill_dir(temp_dir.path(), "git-commit");
        create_agent_dir(temp_dir.path(), "code-reviewer");

        let skills = scan_for_skills(temp_dir.path()).unwrap();
        assert_eq!(skills.len(), 1);
        assert!(skills.contains(&"git-commit".to_string()));
        assert!(!skills.contains(&"code-reviewer".to_string()));
    }

    #[test]
    fn test_mixed_skills_and_agents() {
        let temp_dir = TempDir::new().unwrap();

        create_skill_dir(temp_dir.path(), "git-commit");
        create_skill_dir(temp_dir.path(), "pdf-reader");
        create_agent_dir(temp_dir.path(), "code-reviewer");
        create_agent_dir(temp_dir.path(), "debugger");

        let skills = scan_for_skills(temp_dir.path()).unwrap();
        let agents = scan_for_agents(temp_dir.path()).unwrap();

        assert_eq!(skills.len(), 2);
        assert_eq!(agents.len(), 2);

        assert!(skills.contains(&"git-commit".to_string()));
        assert!(skills.contains(&"pdf-reader".to_string()));
        assert!(agents.contains(&"code-reviewer".to_string()));
        assert!(agents.contains(&"debugger".to_string()));
    }

    #[test]
    fn test_dir_with_both_skill_and_agent_md() {
        // Test edge case: a directory containing both SKILL.md and AGENT.md
        // Should be detected as both a skill and an agent
        let temp_dir = TempDir::new().unwrap();

        let dual_dir = temp_dir.path().join("dual-purpose");
        std::fs::create_dir_all(&dual_dir).unwrap();
        std::fs::write(dual_dir.join("SKILL.md"), "# Dual Skill\n").unwrap();
        std::fs::write(dual_dir.join("AGENT.md"), "# Dual Agent\n").unwrap();

        let skills = scan_for_skills(temp_dir.path()).unwrap();
        let agents = scan_for_agents(temp_dir.path()).unwrap();

        assert_eq!(skills.len(), 1);
        assert_eq!(agents.len(), 1);
        assert!(skills.contains(&"dual-purpose".to_string()));
        assert!(agents.contains(&"dual-purpose".to_string()));
    }

    // Regression tests for the subdirectory-aware refresh (issue #1).

    fn config_with_repo_and_skills(
        repo_id: &str,
        subpath: &str,
        names: &[&str],
    ) -> crate::config::state::Config {
        let mut config = crate::config::state::Config::new();
        config.add_repository(
            repo_id.to_string(),
            "https://example.com/owner/repo.git".to_string(),
            subpath.to_string(),
            Some("abcdef012345".to_string()),
            None,
        );
        for name in names {
            let skill_path = if subpath.is_empty() {
                name.to_string()
            } else {
                format!("{}/{}", subpath, name)
            };
            config.add_skill(name.to_string(), repo_id.to_string(), skill_path);
        }
        config
    }

    #[test]
    fn test_join_subdir() {
        let root = Path::new("/cache/repo");
        assert_eq!(join_subdir(root, ""), PathBuf::from("/cache/repo"));
        assert_eq!(
            join_subdir(root, "skills"),
            PathBuf::from("/cache/repo/skills")
        );
    }

    #[test]
    fn test_refresh_preserves_skills_in_subdirectory() {
        // Skills live under a `skills/` subdirectory inside the repo.
        let temp_dir = TempDir::new().unwrap();
        let sub = temp_dir.path().join("skills");
        create_skill_dir(&sub, "alpha");
        create_skill_dir(&sub, "beta");

        let repo_id = "example.com/owner/repo";
        let mut config = config_with_repo_and_skills(repo_id, "skills", &["alpha", "beta"]);

        // Refresh with the correct subpath: skills must survive (issue #1 regression).
        let changes = refresh_repo_items(&mut config, repo_id, temp_dir.path(), "skills").unwrap();
        assert!(
            changes.removed_skills.is_empty(),
            "no skills should be removed"
        );
        assert!(config.has_skill("alpha"));
        assert!(config.has_skill("beta"));
    }

    #[test]
    fn test_safety_net_skips_mass_removal_on_empty_scan() {
        // An empty scan (e.g. wrong directory) must NOT wipe registered skills.
        let temp_dir = TempDir::new().unwrap(); // no skill dirs inside
        let repo_id = "example.com/owner/repo";
        let mut config = config_with_repo_and_skills(repo_id, "skills", &["alpha", "beta"]);

        let changes = refresh_repo_items(&mut config, repo_id, temp_dir.path(), "skills").unwrap();
        assert!(
            changes.removed_skills.is_empty(),
            "safety net must skip removal"
        );
        assert!(config.has_skill("alpha"));
        assert!(config.has_skill("beta"));
    }

    #[test]
    fn test_refresh_removes_genuinely_deleted_skill() {
        // When the scan finds some (but not all) skills, the missing one is removed.
        let temp_dir = TempDir::new().unwrap();
        let sub = temp_dir.path().join("skills");
        create_skill_dir(&sub, "alpha"); // beta is gone upstream

        let repo_id = "example.com/owner/repo";
        let mut config = config_with_repo_and_skills(repo_id, "skills", &["alpha", "beta"]);

        let changes = refresh_repo_items(&mut config, repo_id, temp_dir.path(), "skills").unwrap();
        assert_eq!(changes.removed_skills, vec!["beta".to_string()]);
        assert!(config.has_skill("alpha"));
        assert!(!config.has_skill("beta"));
    }
}
