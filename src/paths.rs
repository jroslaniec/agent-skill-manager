use anyhow::{Context, Result};
use std::path::PathBuf;

/// Get the cache directory for agent-skill-manager
pub fn cache_dir() -> Result<PathBuf> {
    let cache = dirs::cache_dir()
        .context("Could not find cache directory")?;
    Ok(cache.join("agent-skill-manager"))
}

/// Get the config file path (in cache directory as requested)
pub fn config_path() -> Result<PathBuf> {
    Ok(cache_dir()?.join("config.toml"))
}

/// Get the lock file path (in tmp directory)
pub fn lock_path() -> PathBuf {
    let pid = std::process::id();
    std::env::temp_dir().join(format!("agent-skill-manager-{}.lock", pid))
}

/// Get the git cache directory where repos are cloned
pub fn git_cache_dir() -> Result<PathBuf> {
    Ok(cache_dir()?.join("git"))
}

/// Get the Claude config directory (~/.claude)
pub fn claude_dir() -> Result<PathBuf> {
    let home = dirs::home_dir()
        .context("Could not find home directory")?;
    Ok(home.join(".claude"))
}

/// Get the Claude skills directory (~/.claude/skills)
pub fn claude_skills_dir() -> Result<PathBuf> {
    Ok(claude_dir()?.join("skills"))
}

/// Ensure all necessary directories exist
pub fn ensure_dirs() -> Result<()> {
    std::fs::create_dir_all(cache_dir()?)?;
    std::fs::create_dir_all(git_cache_dir()?)?;
    Ok(())
}
