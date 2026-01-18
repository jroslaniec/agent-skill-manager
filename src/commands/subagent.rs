use anyhow::{bail, Context, Result};
use console::style;
use std::os::unix::fs as unix_fs;
use std::path::PathBuf;

use crate::config::state::Config;
use crate::config::ConfigLock;
use crate::paths;
use crate::skill_ref::RepoRef;

/// Enable one or more subagents
pub fn enable(agent_names_or_refs: &[String]) -> Result<()> {
    let lock = ConfigLock::acquire()?;
    enable_with_lock(&lock, agent_names_or_refs)
}

pub fn enable_with_lock(lock: &ConfigLock, agent_names_or_refs: &[String]) -> Result<()> {
    // Migrate integration configs to add missing agents_dir for built-in integrations
    // This handles configs created before agent support was added
    lock.update(|config| {
        config.migrate_integration_agents_dirs();
        Ok(())
    })?;

    for agent_name_or_ref in agent_names_or_refs {
        let config = lock.read_config()?;

        // Check if this is just an agent name (already registered)
        if config.has_agent(agent_name_or_ref) {
            // Re-enable existing agent by name
            if config.agents.get(agent_name_or_ref).unwrap().enabled {
                println!(
                    "Agent {} is already enabled",
                    style(agent_name_or_ref).cyan()
                );
                continue;
            }

            lock.update(|config| config.enable_agent(agent_name_or_ref))?;

            // Create symlinks in all integrations
            let config = lock.read_config()?;
            let agent_info = config.agents.get(agent_name_or_ref).unwrap();

            // Get the source path to the AGENT.md file (resolve handles legacy paths)
            let repo_ref = RepoRef::parse(&agent_info.repository)?;
            let repo_cache_path = paths::resolve_repo_cache_path(&repo_ref)?;
            let source_path = repo_cache_path.join(&agent_info.agent_path).join("AGENT.md");

            create_agent_symlinks_for_all_integrations(&source_path, agent_name_or_ref, &config)?;

            println!(
                "Enabled agent {}",
                style(agent_name_or_ref).cyan()
            );
            continue;
        }

        // Not found by name - agent must be registered first via sm repo add
        bail!(
            "Agent '{}' not found. Use 'sm repo add' to add a repository first, then enable agents by name.\nUse 'sm subagents list --all' to see available agents.",
            agent_name_or_ref
        );
    }

    Ok(())
}

/// Disable one or more subagents
pub fn disable(agent_names_or_refs: &[String]) -> Result<()> {
    let lock = ConfigLock::acquire()?;
    disable_with_lock(&lock, agent_names_or_refs)
}

pub fn disable_with_lock(lock: &ConfigLock, agent_names_or_refs: &[String]) -> Result<()> {
    for agent_name_or_ref in agent_names_or_refs {
        let config = lock.read_config()?;

        if !config.has_agent(agent_name_or_ref) {
            bail!(
                "Agent '{}' not found. Use 'sm subagents list --all' to see registered agents.",
                agent_name_or_ref
            );
        }

        let existing_agent = config.agents.get(agent_name_or_ref).unwrap();
        if !existing_agent.enabled {
            println!(
                "Agent {} is already disabled",
                style(agent_name_or_ref).dim()
            );
            continue;
        }

        // Remove symlinks from all integrations
        remove_agent_symlinks_from_all_integrations(agent_name_or_ref, &config);

        // Update config
        lock.update(|config| config.disable_agent(agent_name_or_ref))?;

        println!(
            "Disabled agent {}",
            style(agent_name_or_ref).cyan()
        );
    }

    Ok(())
}

/// List subagents
pub fn list(all: bool, status: Option<&str>, name_only: bool) -> Result<()> {
    let lock = ConfigLock::acquire()?;
    let config = lock.read_config()?;

    if config.agents.is_empty() {
        println!("No subagents registered.");
        return Ok(());
    }

    // Validate status filter if provided
    if let Some(s) = status {
        if s != "enabled" && s != "disabled" {
            bail!("Invalid status '{}'. Use 'enabled' or 'disabled'.", s);
        }
    }

    // Collect and sort agents by name
    let mut agents: Vec<_> = config.agents.iter().collect();
    agents.sort_by_key(|(name, _)| *name);

    // Filter agents based on flags
    let filtered_agents: Vec<_> = agents
        .into_iter()
        .filter(|(_, agent)| {
            // If --all is set, show all
            if all {
                return true;
            }

            // If --status is set, filter by status
            if let Some(s) = status {
                return if s == "enabled" {
                    agent.enabled
                } else {
                    !agent.enabled
                };
            }

            // Default: show only enabled agents
            agent.enabled
        })
        .collect();

    if filtered_agents.is_empty() {
        if status.is_some() {
            println!("No {} subagents found.", status.unwrap());
        } else if all {
            println!("No subagents found.");
        } else {
            println!("No enabled subagents found.");
            println!();
            println!("{}", style("To see all subagents use: sm subagents list --all").dim());
        }
        return Ok(());
    }

    // Output format: name-only or table
    if name_only {
        for (name, _) in filtered_agents {
            println!("{}", name);
        }
    } else {
        // Print header
        println!(
            "{:<30}  {:<10}  {}",
            style("SUBAGENT").bold(),
            style("STATUS").bold(),
            style("REPOSITORY").bold()
        );

        // Print separator
        println!("{}", "-".repeat(80));

        // Print each agent
        for (name, agent) in filtered_agents {
            let status = if agent.enabled {
                style("enabled").green()
            } else {
                style("disabled").dim()
            };

            println!(
                "{:<30}  {:<10}  {}",
                style(name).cyan(),
                status,
                style(&agent.repository).dim()
            );
        }

        // Show helper message when default view (enabled only) is shown
        if !all && status.is_none() {
            println!();
            println!(
                "{}",
                style("To see all subagents use: sm subagents list --all").dim()
            );
        }
    }

    Ok(())
}

// Helper functions

/// Create a symlink for an agent (file symlink pointing to AGENT.md)
fn create_agent_symlink(source: &PathBuf, link: &PathBuf) -> Result<()> {
    if link.exists() || link.symlink_metadata().is_ok() {
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

/// Create symlinks for an agent in all registered integrations
/// Agent symlinks are file symlinks: {agents_dir}/{name}.md -> cached AGENT.md
fn create_agent_symlinks_for_all_integrations(
    source: &PathBuf,
    agent_name: &str,
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
        // Skip integrations that don't have an agents directory
        let agents_dir_str = match &integration.agents_dir {
            Some(dir) => dir,
            None => continue,
        };

        // Expand tilde in agents_dir path
        let agents_dir = paths::expand_tilde(agents_dir_str)?;

        // Create directory if it doesn't exist
        if !agents_dir.exists() {
            if let Err(e) = std::fs::create_dir_all(&agents_dir) {
                errors.push((name.clone(), format!("Failed to create directory: {}", e)));
                continue;
            }
        }

        // Agent symlinks use .md extension: {agents_dir}/{name}.md
        let link_path = agents_dir.join(format!("{}.md", agent_name));
        match create_agent_symlink(source, &link_path) {
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

/// Remove symlinks for an agent from all registered integrations
pub fn remove_agent_symlinks_from_all_integrations(
    agent_name: &str,
    config: &Config,
) {
    for (name, integration) in &config.integrations {
        // Skip integrations that don't have an agents directory
        let agents_dir_str = match &integration.agents_dir {
            Some(dir) => dir,
            None => continue,
        };

        // Expand tilde in agents_dir path
        let agents_dir = match paths::expand_tilde(agents_dir_str) {
            Ok(dir) => dir,
            Err(_) => continue,
        };

        // Agent symlinks use .md extension: {agents_dir}/{name}.md
        let link_path = agents_dir.join(format!("{}.md", agent_name));

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

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_create_agent_symlink() {
        let temp_dir = TempDir::new().unwrap();

        // Create a source AGENT.md file
        let source_dir = temp_dir.path().join("source");
        std::fs::create_dir_all(&source_dir).unwrap();
        let source_file = source_dir.join("AGENT.md");
        std::fs::write(&source_file, "# Test Agent\n").unwrap();

        // Create the symlink
        let link_path = temp_dir.path().join("test-agent.md");
        create_agent_symlink(&source_file, &link_path).unwrap();

        // Verify symlink was created
        assert!(link_path.is_symlink());

        // Verify symlink points to correct file
        let target = std::fs::read_link(&link_path).unwrap();
        assert_eq!(target, source_file);

        // Verify we can read through the symlink
        let content = std::fs::read_to_string(&link_path).unwrap();
        assert!(content.contains("Test Agent"));
    }

    #[test]
    fn test_create_agent_symlink_replaces_existing() {
        let temp_dir = TempDir::new().unwrap();

        // Create first source file
        let source1 = temp_dir.path().join("source1.md");
        std::fs::write(&source1, "# First\n").unwrap();

        // Create second source file
        let source2 = temp_dir.path().join("source2.md");
        std::fs::write(&source2, "# Second\n").unwrap();

        let link_path = temp_dir.path().join("test-agent.md");

        // Create symlink to first source
        create_agent_symlink(&source1, &link_path).unwrap();
        assert!(link_path.is_symlink());

        // Replace with symlink to second source
        create_agent_symlink(&source2, &link_path).unwrap();

        // Verify symlink now points to second source
        let target = std::fs::read_link(&link_path).unwrap();
        assert_eq!(target, source2);
    }

    #[test]
    fn test_create_agent_symlink_fails_on_regular_file() {
        let temp_dir = TempDir::new().unwrap();

        // Create source
        let source = temp_dir.path().join("source.md");
        std::fs::write(&source, "# Source\n").unwrap();

        // Create a regular file at the link path (not a symlink)
        let link_path = temp_dir.path().join("existing-file.md");
        std::fs::write(&link_path, "# Existing content\n").unwrap();

        // Should fail because target exists and is not a symlink
        let result = create_agent_symlink(&source, &link_path);
        assert!(result.is_err());
    }
}
