use anyhow::{bail, Context, Result};
use console::style;
use std::os::unix::fs as unix_fs;
use std::path::PathBuf;

use crate::config::ConfigLock;
use crate::git;
use crate::paths;
use crate::skill_ref::{RepoRef, SkillRef};

pub fn enable(skill_name_or_ref: &str) -> Result<()> {
    let lock = ConfigLock::acquire()?;
    let config = lock.read_config()?;

    // Check if this is just a skill name (already registered)
    if config.has_skill(skill_name_or_ref) {
        // Re-enable existing skill by name
        if config.skills.get(skill_name_or_ref).unwrap().enabled {
            println!(
                "Skill {} is already enabled",
                style(skill_name_or_ref).cyan()
            );
            return Ok(());
        }

        lock.update(|config| config.enable_skill(skill_name_or_ref))?;

        // Create symlink if it doesn't exist
        let skill_link_path = paths::claude_skills_dir()?.join(skill_name_or_ref);
        if !skill_link_path.exists() {
            // We need to get the source path from the config
            let skill_info = config.skills.get(skill_name_or_ref).unwrap();

            // Parse repository reference to get cache path
            let repo_ref = RepoRef::parse(&skill_info.repository)?;
            let git_cache = paths::git_cache_dir()?;
            let repo_cache_path = git_cache.join(&repo_ref.owner).join(&repo_ref.repo);
            let source_path = repo_cache_path.join(&skill_info.skill_path);
            create_skill_symlink(&source_path, &skill_link_path)?;
        }

        println!(
            "Enabled skill {}",
            style(skill_name_or_ref).cyan()
        );
        return Ok(());
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
            return Ok(());
        } else {
            // Re-enable the skill
            lock.update(|config| config.enable_skill(&skill.skill_name))?;

            // Create symlink if it doesn't exist
            let skill_link_path = paths::claude_skills_dir()?.join(&skill.skill_name);
            if !skill_link_path.exists() {
                let source_path = get_skill_source_path(&skill)?;
                create_skill_symlink(&source_path, &skill_link_path)?;
            }

            println!(
                "Enabled skill {}",
                style(&skill.skill_name).cyan()
            );
            return Ok(());
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

        // Add repository to config and persist
        lock.update(|config| {
            config.add_repository(repo_id.clone(), skill.git_url(), String::new());
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

    // Create symlink to Claude skills directory
    let claude_skills_dir = paths::claude_skills_dir()?;
    if !claude_skills_dir.exists() {
        bail!(
            "Claude skills directory not found at {}.\nPlease ensure Claude Code is installed and the ~/.claude directory exists.",
            claude_skills_dir.display()
        );
    }

    let skill_link_path = claude_skills_dir.join(&skill.skill_name);
    create_skill_symlink(&source_path, &skill_link_path)?;

    // Add skill to config
    lock.update(|config| {
        config.add_skill(skill.skill_name.clone(), repo_id.clone(), skill.path.clone());
        Ok(())
    })?;

    println!(
        "Enabled skill {}",
        style(&skill.skill_name).cyan()
    );

    Ok(())
}

pub fn disable(skill_name_or_ref: &str) -> Result<()> {
    let lock = ConfigLock::acquire()?;
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
        return Ok(());
    }

    // Remove symlink
    let skill_link_path = paths::claude_skills_dir()?.join(&skill_name);
    if skill_link_path.exists() {
        std::fs::remove_file(&skill_link_path)
            .context("Failed to remove skill symlink")?;
    }

    // Update config
    lock.update(|config| config.disable_skill(&skill_name))?;

    println!(
        "Disabled skill {}",
        style(&skill_name).cyan()
    );

    Ok(())
}

pub fn list() -> Result<()> {
    let lock = ConfigLock::acquire()?;
    let config = lock.read_config()?;

    if config.skills.is_empty() {
        println!("No skills registered.");
        return Ok(());
    }

    // Collect and sort skills by name
    let mut skills: Vec<_> = config.skills.iter().collect();
    skills.sort_by_key(|(name, _)| *name);

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
    for (name, skill) in skills {
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

    Ok(())
}

// Helper functions

fn get_skill_source_path(skill: &SkillRef) -> Result<PathBuf> {
    let git_cache = paths::git_cache_dir()?;
    let repo_cache_path = git_cache.join(&skill.owner).join(&skill.repo);
    Ok(repo_cache_path.join(&skill.path))
}

fn create_skill_symlink(source: &PathBuf, link: &PathBuf) -> Result<()> {
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
