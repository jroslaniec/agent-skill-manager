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

/// Get the full cache path for a repository
///
/// This combines the git cache directory with the repository-specific cache path.
/// The cache structure depends on the source type:
/// - HTTPS/SSH (remote): `{cache}/git/{host}/{owner}/{repo}`
/// - Local paths: `{cache}/git/local/{dirname}-{hash}`
///
/// This ensures backward compatibility with existing GitHub-only paths
/// while supporting new universal URL formats.
pub fn repo_cache_path(repo_ref: &crate::skill_ref::RepoRef) -> Result<PathBuf> {
    Ok(git_cache_dir()?.join(repo_ref.cache_path()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_integration_name() {
        assert_eq!(normalize_integration_name("claude"), "claude-code");
        assert_eq!(normalize_integration_name("Claude"), "claude-code");
        assert_eq!(normalize_integration_name("claudecode"), "claude-code");
        assert_eq!(normalize_integration_name("claude-code"), "claude-code");
        assert_eq!(normalize_integration_name("opencode"), "opencode");
        assert_eq!(normalize_integration_name("codex"), "codex");
        assert_eq!(normalize_integration_name("custom"), "custom");
    }

    #[test]
    fn test_expand_tilde() {
        // Test non-tilde path
        let result = expand_tilde("/absolute/path").unwrap();
        assert_eq!(result, PathBuf::from("/absolute/path"));

        // Test tilde path (home directory expansion)
        let result = expand_tilde("~/test").unwrap();
        assert!(result.to_string_lossy().contains("test"));
        assert!(!result.to_string_lossy().starts_with("~"));
    }

    #[test]
    fn test_repo_cache_path_github() {
        use crate::skill_ref::RepoRef;

        let repo = RepoRef::parse("github.com/testowner/testrepo").unwrap();
        let cache_path = repo.cache_path();

        // Should use host/owner/repo structure for backward compatibility
        assert_eq!(
            cache_path,
            PathBuf::from("github.com").join("testowner/testrepo")
        );
    }

    #[test]
    fn test_repo_cache_path_gitlab() {
        use crate::skill_ref::RepoRef;

        let repo = RepoRef::parse("gitlab.com/testteam/testproject").unwrap();
        let cache_path = repo.cache_path();

        // Should use host/owner/repo structure
        assert_eq!(
            cache_path,
            PathBuf::from("gitlab.com").join("testteam/testproject")
        );
    }

    #[test]
    fn test_repo_cache_path_ssh() {
        use crate::skill_ref::RepoRef;

        let repo = RepoRef::parse("git@github.com:testowner/testrepo.git").unwrap();
        let cache_path = repo.cache_path();

        // SSH URLs should produce same cache path as HTTPS for same repo
        assert_eq!(
            cache_path,
            PathBuf::from("github.com").join("testowner/testrepo")
        );
    }

    #[test]
    fn test_repo_cache_path_local() {
        use crate::skill_ref::RepoRef;

        let repo = RepoRef::parse("/Users/dev/my-skills").unwrap();
        let cache_path = repo.cache_path();

        // Local paths should use local/{dirname}-{hash} format
        assert!(cache_path.starts_with("local/"));
        assert!(cache_path.to_string_lossy().contains("my-skills"));
    }

    #[test]
    fn test_repo_cache_path_local_with_special_chars() {
        use crate::skill_ref::RepoRef;

        // Test path with spaces
        let repo = RepoRef::parse("/Users/dev/my skills/repo").unwrap();
        let cache_path = repo.cache_path();

        // Should be safely encoded in local/ directory
        assert!(cache_path.starts_with("local/"));
        // The path should contain "repo" as the directory name
        assert!(cache_path.to_string_lossy().contains("repo"));
    }

    #[test]
    fn test_repo_cache_path_gitlab_groups() {
        use crate::skill_ref::RepoRef;

        // GitLab can have nested groups
        let repo = RepoRef::parse("git@gitlab.com:testgroup/subgroup/testrepo.git").unwrap();
        let cache_path = repo.cache_path();

        // Should handle nested group structure
        assert!(cache_path.starts_with("gitlab.com"));
        assert!(cache_path.to_string_lossy().contains("testgroup"));
        assert!(cache_path.to_string_lossy().contains("subgroup"));
    }

    #[test]
    fn test_repo_cache_path_self_hosted() {
        use crate::skill_ref::RepoRef;

        let repo = RepoRef::parse("https://git.example.com/internal/testrepo.git").unwrap();
        let cache_path = repo.cache_path();

        // Self-hosted repos should use their domain
        assert!(cache_path.starts_with("git.example.com"));
    }

    #[test]
    fn test_builtin_integrations() {
        let integrations = builtin_integrations();

        // Should have 4 built-in integrations
        assert_eq!(integrations.len(), 4);

        // Verify claude-code is present
        assert!(integrations.iter().any(|(name, _)| *name == "claude-code"));
    }
}
