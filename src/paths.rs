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

/// Normalize integration name to canonical form
/// e.g., "claude", "Claude", "claudecode" -> "claude-code"
/// e.g., "opencode", "open-code", "OpenCode" -> "opencode"
pub fn normalize_integration_name(input: &str) -> String {
    let lower = input.to_lowercase().replace('-', "").replace('_', "");
    match lower.as_str() {
        "claude" | "claudecode" => "claude-code".to_string(),
        "opencode" => "opencode".to_string(),
        "codex" | "openaicodex" | "codexcli" => "codex".to_string(),
        "gemini" | "geminicli" => "gemini-cli".to_string(),
        _ => input.to_lowercase(),
    }
}

/// Get the default skills directory for a built-in integration
/// Returns None for unknown integrations (require --path flag)
pub fn get_builtin_skills_dir(name: &str) -> Option<PathBuf> {
    let home = dirs::home_dir()?;
    let normalized = normalize_integration_name(name);
    match normalized.as_str() {
        "claude-code" => Some(home.join(".claude").join("skills")),
        "opencode" => Some(home.join(".config").join("opencode").join("skill")),
        "codex" => Some(home.join(".codex").join("skills")),
        "gemini-cli" => Some(home.join(".gemini").join("skills")),
        _ => None,
    }
}

/// List of known built-in integrations with their default paths
pub fn builtin_integrations() -> Vec<(&'static str, &'static str)> {
    vec![
        ("claude-code", "~/.claude/skills"),
        ("codex", "~/.codex/skills"),
        ("gemini-cli", "~/.gemini/skills"),
        ("opencode", "~/.config/opencode/skill"),
    ]
}

/// Expand ~ in path to home directory
pub fn expand_tilde(path: &str) -> Result<PathBuf> {
    if path.starts_with("~/") {
        let home = dirs::home_dir()
            .context("Could not find home directory")?;
        Ok(home.join(&path[2..]))
    } else if path == "~" {
        dirs::home_dir().context("Could not find home directory")
    } else {
        Ok(PathBuf::from(path))
    }
}
