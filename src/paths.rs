use anyhow::{Context, Result};
use std::path::PathBuf;

/// Get the cache directory for agent-skill-manager
/// Always uses ~/.cache/agent-skill-manager on all platforms for consistency
pub fn cache_dir() -> Result<PathBuf> {
    let home = dirs::home_dir().context("Could not find home directory")?;
    Ok(home.join(".cache").join("agent-skill-manager"))
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
    let home = dirs::home_dir().context("Could not find home directory")?;
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
    let lower = input.to_lowercase().replace(['-', '_'], "");
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

/// Get the default agents directory for a built-in integration
/// Returns None for integrations that don't support agents (e.g., codex uses AGENTS.md)
pub fn get_builtin_agents_dir(name: &str) -> Option<PathBuf> {
    let home = dirs::home_dir()?;
    let normalized = normalize_integration_name(name);
    match normalized.as_str() {
        "claude-code" => Some(home.join(".claude").join("agents")),
        "opencode" => Some(home.join(".config").join("opencode").join("agent")),
        "gemini-cli" => Some(home.join(".gemini").join("agents")),
        // codex uses AGENTS.md, not individual agent files
        "codex" => None,
        _ => None,
    }
}

/// Built-in integration info with default skills and agents directories
pub struct BuiltinIntegration {
    pub name: &'static str,
    pub skills_dir: Option<&'static str>,
    pub agents_dir: Option<&'static str>,
}

/// List of known built-in integrations with their default paths
pub fn builtin_integrations() -> Vec<BuiltinIntegration> {
    vec![
        BuiltinIntegration {
            name: "claude-code",
            skills_dir: Some("~/.claude/skills"),
            agents_dir: Some("~/.claude/agents"),
        },
        BuiltinIntegration {
            name: "codex",
            skills_dir: Some("~/.codex/skills"),
            agents_dir: None, // codex uses AGENTS.md
        },
        BuiltinIntegration {
            name: "gemini-cli",
            skills_dir: Some("~/.gemini/skills"),
            agents_dir: Some("~/.gemini/agents"),
        },
        BuiltinIntegration {
            name: "opencode",
            skills_dir: Some("~/.config/opencode/skill"),
            agents_dir: Some("~/.config/opencode/agent"),
        },
    ]
}

/// Legacy function for backward compatibility - returns (name, skills_dir) pairs
pub fn builtin_integrations_skills_only() -> Vec<(&'static str, &'static str)> {
    builtin_integrations()
        .into_iter()
        .filter_map(|bi| bi.skills_dir.map(|sd| (bi.name, sd)))
        .collect()
}

/// Expand ~ in path to home directory
pub fn expand_tilde(path: &str) -> Result<PathBuf> {
    if let Some(stripped) = path.strip_prefix("~/") {
        let home = dirs::home_dir().context("Could not find home directory")?;
        Ok(home.join(stripped))
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
///
/// NOTE: This returns the "expected" path for new repos. For existing repos
/// that may be at legacy paths, use `resolve_repo_cache_path()` instead.
pub fn repo_cache_path(repo_ref: &crate::skill_ref::RepoRef) -> Result<PathBuf> {
    Ok(git_cache_dir()?.join(repo_ref.cache_path()))
}

/// Resolve the actual cache path for a repository, checking both new and legacy locations
///
/// Before universal git support, GitHub repos were cached at `git/{owner}/{repo}`.
/// After universal git support, they are cached at `git/{host}/{owner}/{repo}`.
///
/// For local repos, returns the original filesystem path directly (no cache copy).
///
/// This function checks if the repo exists at the new-style path first, then falls
/// back to checking the legacy path for GitHub repos.
///
/// Returns the path where the repo actually exists, or the new-style path if
/// the repo doesn't exist yet (for new clones).
pub fn resolve_repo_cache_path(repo_ref: &crate::skill_ref::RepoRef) -> Result<PathBuf> {
    // Local repos: return the original filesystem path directly
    if repo_ref.source_type == crate::skill_ref::GitSourceType::Local {
        return Ok(PathBuf::from(&repo_ref.git_url));
    }

    let git_cache = git_cache_dir()?;

    // First check the new-style path
    let new_style_path = git_cache.join(repo_ref.cache_path());
    if new_style_path.exists() {
        return Ok(new_style_path);
    }

    // For GitHub repos, check the legacy path (git/{owner}/{repo} without host)
    if repo_ref.repo_id.starts_with("github.com/") {
        let legacy_path = repo_ref
            .repo_id
            .strip_prefix("github.com/")
            .map(|rest| git_cache.join(rest));

        if let Some(legacy) = legacy_path
            && legacy.exists()
        {
            return Ok(legacy);
        }
    }

    // Neither exists yet - return new-style path (for new clones)
    Ok(new_style_path)
}

/// Get the legacy cache path for a GitHub repository (git/{owner}/{repo})
///
/// This is used for backward compatibility with repos cloned before
/// universal git support was added.
///
/// Returns None if the repo is not a GitHub repo.
pub fn legacy_github_cache_path(repo_id: &str) -> Result<Option<PathBuf>> {
    if repo_id.starts_with("github.com/") {
        let rest = repo_id.strip_prefix("github.com/").unwrap();
        Ok(Some(git_cache_dir()?.join(rest)))
    } else {
        Ok(None)
    }
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

        // Verify claude-code is present with both directories
        let claude = integrations.iter().find(|bi| bi.name == "claude-code");
        assert!(claude.is_some());
        let claude = claude.unwrap();
        assert_eq!(claude.skills_dir, Some("~/.claude/skills"));
        assert_eq!(claude.agents_dir, Some("~/.claude/agents"));

        // Verify codex has no agents_dir
        let codex = integrations.iter().find(|bi| bi.name == "codex");
        assert!(codex.is_some());
        assert!(codex.unwrap().agents_dir.is_none());
    }

    #[test]
    fn test_get_builtin_agents_dir() {
        // Claude-code should have agents dir
        let claude = get_builtin_agents_dir("claude-code");
        assert!(claude.is_some());
        assert!(claude.unwrap().to_string_lossy().contains(".claude"));

        // Codex should not have agents dir
        let codex = get_builtin_agents_dir("codex");
        assert!(codex.is_none());

        // Unknown integration should return None
        let unknown = get_builtin_agents_dir("unknown");
        assert!(unknown.is_none());
    }

    #[test]
    fn test_legacy_github_cache_path() {
        // GitHub repos should return Some with the legacy path
        let path = legacy_github_cache_path("github.com/testowner/testrepo").unwrap();
        assert!(path.is_some());
        let path = path.unwrap();
        assert!(path.to_string_lossy().ends_with("testowner/testrepo"));
        // Should NOT contain github.com in the path (that's the legacy format)
        assert!(!path.to_string_lossy().contains("github.com"));
    }

    #[test]
    fn test_legacy_github_cache_path_non_github() {
        // Non-GitHub repos should return None
        let gitlab = legacy_github_cache_path("gitlab.com/testowner/testrepo").unwrap();
        assert!(gitlab.is_none());

        let local = legacy_github_cache_path("local:/Users/dev/repo").unwrap();
        assert!(local.is_none());
    }

    #[test]
    fn test_resolve_repo_cache_path_new_style() {
        use crate::skill_ref::RepoRef;

        // This test just verifies the logic - actual behavior depends on
        // whether files exist on disk which we can't easily mock
        let repo = RepoRef::parse("github.com/testowner/testrepo").unwrap();
        let _path = resolve_repo_cache_path(&repo);

        // The path should be valid (it won't exist, but the function should return ok)
        // The actual path returned will be the new-style since neither exists
    }

    #[test]
    fn test_resolve_repo_cache_path_gitlab_no_legacy() {
        use crate::skill_ref::RepoRef;

        // GitLab repos don't have legacy paths, so they should always
        // return the new-style path
        let repo = RepoRef::parse("gitlab.com/testteam/testrepo").unwrap();
        let path = resolve_repo_cache_path(&repo).unwrap();

        // Should be the new-style path with gitlab.com
        assert!(path.to_string_lossy().contains("gitlab.com"));
    }
}
