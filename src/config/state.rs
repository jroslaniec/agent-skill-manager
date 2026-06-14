use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Config {
    #[serde(default)]
    pub repositories: HashMap<String, Repository>,
    #[serde(default)]
    pub skills: HashMap<String, Skill>,
    #[serde(default)]
    pub agents: HashMap<String, Agent>,
    #[serde(default)]
    pub integrations: HashMap<String, Integration>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Repository {
    pub url: String,
    pub path: String, // Subdirectory path within repo (empty for root)
    #[serde(default = "chrono_now")]
    pub cloned_at: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub current_sha: Option<String>, // Current checked-out commit (12 chars)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pinned_sha: Option<String>, // If set, repo is pinned to this SHA
    #[serde(default, skip_serializing_if = "is_false")]
    pub auto_upgrade: bool, // If true, upgraded automatically by the daily maintenance pass
}

fn is_false(value: &bool) -> bool {
    !*value
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Skill {
    pub repository: String, // Repository ID (e.g., "github.com/owner/repo")
    pub skill_path: String, // Path within repo (e.g., "git-commit")
    pub enabled: bool,
    #[serde(default = "chrono_now")]
    pub enabled_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub disabled_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Agent {
    pub repository: String, // Repository ID (e.g., "github.com/owner/repo")
    pub agent_path: String, // Path within repo (e.g., "code-reviewer")
    pub enabled: bool,
    #[serde(default = "chrono_now")]
    pub enabled_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub disabled_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Integration {
    /// Path to skills directory (e.g., "~/.claude/skills")
    /// Optional: Some integrations may only support agents
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub skills_dir: Option<String>,
    /// Path to agents directory (e.g., "~/.claude/agents")
    /// Optional: Some integrations may only support skills
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agents_dir: Option<String>,
    #[serde(default = "chrono_now")]
    pub enabled_at: String,
}

fn chrono_now() -> String {
    chrono::Utc::now().to_rfc3339()
}

impl Config {
    /// Load config from TOML string
    pub fn from_toml(content: &str) -> Result<Self> {
        Ok(toml::from_str(content)?)
    }

    /// Serialize config to TOML string
    pub fn to_toml(&self) -> Result<String> {
        Ok(toml::to_string_pretty(self)?)
    }

    /// Create a new empty config
    pub fn new() -> Self {
        Self::default()
    }

    /// Check if a repository exists
    pub fn has_repository(&self, repo_id: &str) -> bool {
        self.repositories.contains_key(repo_id)
    }

    /// Add a repository
    pub fn add_repository(
        &mut self,
        repo_id: String,
        url: String,
        path: String,
        current_sha: Option<String>,
        pinned_sha: Option<String>,
    ) {
        self.repositories.insert(
            repo_id,
            Repository {
                url,
                path,
                cloned_at: chrono_now(),
                current_sha,
                pinned_sha,
                auto_upgrade: false,
            },
        );
    }

    /// Enable or disable automatic upgrades for a repository.
    pub fn set_auto_upgrade(&mut self, repo_id: &str, enabled: bool) -> Result<()> {
        let repo = self
            .repositories
            .get_mut(repo_id)
            .ok_or_else(|| anyhow::anyhow!("Repository '{}' not found", repo_id))?;
        repo.auto_upgrade = enabled;
        Ok(())
    }

    /// Remove a repository
    pub fn remove_repository(&mut self, repo_id: &str) -> Option<Repository> {
        self.repositories.remove(repo_id)
    }

    /// Get skills belonging to a repository
    pub fn skills_for_repo(&self, repo_id: &str) -> Vec<&Skill> {
        self.skills
            .values()
            .filter(|skill| skill.repository == repo_id)
            .collect()
    }

    /// Check if a skill exists
    pub fn has_skill(&self, skill_name: &str) -> bool {
        self.skills.contains_key(skill_name)
    }

    /// Add or update a skill
    pub fn add_skill(&mut self, skill_name: String, repository: String, skill_path: String) {
        self.skills.insert(
            skill_name,
            Skill {
                repository,
                skill_path,
                enabled: true,
                enabled_at: chrono_now(),
                disabled_at: None,
            },
        );
    }

    /// Enable a skill
    pub fn enable_skill(&mut self, skill_name: &str) -> Result<()> {
        let skill = self
            .skills
            .get_mut(skill_name)
            .ok_or_else(|| anyhow::anyhow!("Skill '{}' not found", skill_name))?;

        skill.enabled = true;
        skill.enabled_at = chrono_now();
        skill.disabled_at = None;

        Ok(())
    }

    /// Disable a skill
    pub fn disable_skill(&mut self, skill_name: &str) -> Result<()> {
        let skill = self
            .skills
            .get_mut(skill_name)
            .ok_or_else(|| anyhow::anyhow!("Skill '{}' not found", skill_name))?;

        skill.enabled = false;
        skill.disabled_at = Some(chrono_now());

        Ok(())
    }

    /// Remove a skill
    pub fn remove_skill(&mut self, skill_name: &str) -> Option<Skill> {
        self.skills.remove(skill_name)
    }

    /// Get agents belonging to a repository
    pub fn agents_for_repo(&self, repo_id: &str) -> Vec<&Agent> {
        self.agents
            .values()
            .filter(|agent| agent.repository == repo_id)
            .collect()
    }

    /// Check if an agent exists
    pub fn has_agent(&self, agent_name: &str) -> bool {
        self.agents.contains_key(agent_name)
    }

    /// Add or update an agent
    pub fn add_agent(&mut self, agent_name: String, repository: String, agent_path: String) {
        self.agents.insert(
            agent_name,
            Agent {
                repository,
                agent_path,
                enabled: true,
                enabled_at: chrono_now(),
                disabled_at: None,
            },
        );
    }

    /// Enable an agent
    pub fn enable_agent(&mut self, agent_name: &str) -> Result<()> {
        let agent = self
            .agents
            .get_mut(agent_name)
            .ok_or_else(|| anyhow::anyhow!("Agent '{}' not found", agent_name))?;

        agent.enabled = true;
        agent.enabled_at = chrono_now();
        agent.disabled_at = None;

        Ok(())
    }

    /// Disable an agent
    pub fn disable_agent(&mut self, agent_name: &str) -> Result<()> {
        let agent = self
            .agents
            .get_mut(agent_name)
            .ok_or_else(|| anyhow::anyhow!("Agent '{}' not found", agent_name))?;

        agent.enabled = false;
        agent.disabled_at = Some(chrono_now());

        Ok(())
    }

    /// Remove an agent
    pub fn remove_agent(&mut self, agent_name: &str) -> Option<Agent> {
        self.agents.remove(agent_name)
    }

    /// Get all enabled agents
    pub fn enabled_agents(&self) -> Vec<(&String, &Agent)> {
        self.agents
            .iter()
            .filter(|(_, agent)| agent.enabled)
            .collect()
    }

    /// Check if an integration exists
    pub fn has_integration(&self, name: &str) -> bool {
        self.integrations.contains_key(name)
    }

    /// Add an integration
    pub fn add_integration(
        &mut self,
        name: String,
        skills_dir: Option<String>,
        agents_dir: Option<String>,
    ) {
        self.integrations.insert(
            name,
            Integration {
                skills_dir,
                agents_dir,
                enabled_at: chrono_now(),
            },
        );
    }

    /// Remove an integration
    pub fn remove_integration(&mut self, name: &str) -> Option<Integration> {
        self.integrations.remove(name)
    }

    /// Replace legacy per-tool integrations (`codex`, `gemini-cli`, `opencode`)
    /// with the unified `agents` integration that targets the shared skills
    /// location all of those tools now read.
    ///
    /// Returns the legacy integration names that were present (empty if there
    /// was nothing to migrate). When at least one was present and `agents` isn't
    /// already registered, it is added (skills-only) with `agents_skills_dir`.
    pub fn unify_legacy_integrations(&mut self, agents_skills_dir: Option<String>) -> Vec<String> {
        const LEGACY: [&str; 3] = ["codex", "gemini-cli", "opencode"];

        let present: Vec<String> = LEGACY
            .iter()
            .copied()
            .filter(|name| self.integrations.contains_key(*name))
            .map(|name| name.to_string())
            .collect();

        if present.is_empty() {
            return present;
        }

        for name in &present {
            self.integrations.remove(name);
        }
        if !self.integrations.contains_key("agents") {
            self.add_integration("agents".to_string(), agents_skills_dir, None);
        }

        present
    }

    /// Get all enabled skills
    pub fn enabled_skills(&self) -> Vec<(&String, &Skill)> {
        self.skills
            .iter()
            .filter(|(_, skill)| skill.enabled)
            .collect()
    }

    /// Migrate existing integrations to add missing agents_dir for built-in integrations
    ///
    /// This handles the case where users added integrations before agent support was added.
    /// Built-in integrations that have known agents directories will have them added
    /// if they're missing from the config.
    ///
    /// Returns true if any changes were made.
    pub fn migrate_integration_agents_dirs(&mut self) -> bool {
        use crate::paths;

        let mut changed = false;

        // Get the built-in integrations with their default agents directories
        let builtins = paths::builtin_integrations();

        for bi in builtins {
            // Skip if no agents_dir defined for this integration
            let default_agents_dir = match bi.agents_dir {
                Some(dir) => dir,
                None => continue,
            };

            // Check if this integration exists in config but is missing agents_dir
            if let Some(integration) = self.integrations.get_mut(bi.name)
                && integration.agents_dir.is_none()
            {
                integration.agents_dir = Some(default_agents_dir.to_string());
                changed = true;
            }
        }

        changed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_agent_toml_roundtrip() {
        let mut config = Config::new();
        config.add_agent(
            "test-agent".to_string(),
            "example.com/testowner/testrepo".to_string(),
            "my-agent".to_string(),
        );

        let toml_str = config.to_toml().unwrap();
        let parsed: Config = Config::from_toml(&toml_str).unwrap();

        assert!(parsed.has_agent("test-agent"));
        let agent = &parsed.agents["test-agent"];
        assert_eq!(agent.repository, "example.com/testowner/testrepo");
        assert_eq!(agent.agent_path, "my-agent");
        assert!(agent.enabled);
    }

    #[test]
    fn test_agent_enable_disable() {
        let mut config = Config::new();
        config.add_agent(
            "test-agent".to_string(),
            "example.com/testowner/testrepo".to_string(),
            "my-agent".to_string(),
        );

        assert!(config.agents["test-agent"].enabled);

        config.disable_agent("test-agent").unwrap();
        assert!(!config.agents["test-agent"].enabled);
        assert!(config.agents["test-agent"].disabled_at.is_some());

        config.enable_agent("test-agent").unwrap();
        assert!(config.agents["test-agent"].enabled);
        assert!(config.agents["test-agent"].disabled_at.is_none());
    }

    #[test]
    fn test_agents_for_repo() {
        let mut config = Config::new();
        config.add_agent(
            "agent1".to_string(),
            "example.com/testowner/repo1".to_string(),
            "path1".to_string(),
        );
        config.add_agent(
            "agent2".to_string(),
            "example.com/testowner/repo1".to_string(),
            "path2".to_string(),
        );
        config.add_agent(
            "agent3".to_string(),
            "example.com/testowner/repo2".to_string(),
            "path3".to_string(),
        );

        let agents = config.agents_for_repo("example.com/testowner/repo1");
        assert_eq!(agents.len(), 2);
    }

    #[test]
    fn test_enabled_agents() {
        let mut config = Config::new();
        config.add_agent(
            "agent1".to_string(),
            "example.com/testowner/testrepo".to_string(),
            "path1".to_string(),
        );
        config.add_agent(
            "agent2".to_string(),
            "example.com/testowner/testrepo".to_string(),
            "path2".to_string(),
        );

        config.disable_agent("agent1").unwrap();

        let enabled = config.enabled_agents();
        assert_eq!(enabled.len(), 1);
        assert_eq!(enabled[0].0, "agent2");
    }

    #[test]
    fn test_remove_agent() {
        let mut config = Config::new();
        config.add_agent(
            "test-agent".to_string(),
            "example.com/testowner/testrepo".to_string(),
            "my-agent".to_string(),
        );

        assert!(config.has_agent("test-agent"));
        let removed = config.remove_agent("test-agent");
        assert!(removed.is_some());
        assert!(!config.has_agent("test-agent"));
    }

    #[test]
    fn test_config_with_agents_and_skills() {
        let mut config = Config::new();
        config.add_skill(
            "my-skill".to_string(),
            "example.com/testowner/testrepo".to_string(),
            "skill-path".to_string(),
        );
        config.add_agent(
            "my-agent".to_string(),
            "example.com/testowner/testrepo".to_string(),
            "agent-path".to_string(),
        );

        let toml_str = config.to_toml().unwrap();
        let parsed: Config = Config::from_toml(&toml_str).unwrap();

        assert!(parsed.has_skill("my-skill"));
        assert!(parsed.has_agent("my-agent"));
    }

    #[test]
    fn test_agent_not_found_errors() {
        let mut config = Config::new();

        let enable_result = config.enable_agent("nonexistent");
        assert!(enable_result.is_err());

        let disable_result = config.disable_agent("nonexistent");
        assert!(disable_result.is_err());
    }

    #[test]
    fn test_deserialize_config_without_agents() {
        // Backward compatibility: config without agents section should still parse
        let toml_str = r#"
[repositories]

[skills]

[integrations]
"#;
        let config = Config::from_toml(toml_str).unwrap();
        assert!(config.agents.is_empty());
    }

    #[test]
    fn test_integration_with_both_dirs() {
        let mut config = Config::new();
        config.add_integration(
            "claude-code".to_string(),
            Some("/home/user/.claude/skills".to_string()),
            Some("/home/user/.claude/agents".to_string()),
        );

        let toml_str = config.to_toml().unwrap();
        let parsed: Config = Config::from_toml(&toml_str).unwrap();

        let integration = parsed.integrations.get("claude-code").unwrap();
        assert_eq!(
            integration.skills_dir,
            Some("/home/user/.claude/skills".to_string())
        );
        assert_eq!(
            integration.agents_dir,
            Some("/home/user/.claude/agents".to_string())
        );
    }

    #[test]
    fn test_integration_skills_only() {
        let mut config = Config::new();
        config.add_integration(
            "codex".to_string(),
            Some("/home/user/.codex/skills".to_string()),
            None, // codex doesn't support agents
        );

        let toml_str = config.to_toml().unwrap();
        let parsed: Config = Config::from_toml(&toml_str).unwrap();

        let integration = parsed.integrations.get("codex").unwrap();
        assert_eq!(
            integration.skills_dir,
            Some("/home/user/.codex/skills".to_string())
        );
        assert!(integration.agents_dir.is_none());
    }

    #[test]
    fn test_backward_compatibility_old_integration_format() {
        // Old config format only had skills_dir as a plain string
        let toml_str = r#"
[integrations.claude-code]
skills_dir = "/home/user/.claude/skills"
enabled_at = "2024-01-01T00:00:00Z"
"#;
        let config = Config::from_toml(toml_str).unwrap();

        let integration = config.integrations.get("claude-code").unwrap();
        assert_eq!(
            integration.skills_dir,
            Some("/home/user/.claude/skills".to_string())
        );
        // agents_dir should be None since it wasn't in the old format
        assert!(integration.agents_dir.is_none());
    }

    #[test]
    fn test_integration_agents_dir_only() {
        // Some integrations might only have agents_dir (hypothetical case)
        let mut config = Config::new();
        config.add_integration(
            "agents-only".to_string(),
            None,
            Some("/home/user/.agents".to_string()),
        );

        let toml_str = config.to_toml().unwrap();
        let parsed: Config = Config::from_toml(&toml_str).unwrap();

        let integration = parsed.integrations.get("agents-only").unwrap();
        assert!(integration.skills_dir.is_none());
        assert_eq!(
            integration.agents_dir,
            Some("/home/user/.agents".to_string())
        );
    }

    #[test]
    fn test_migrate_integration_agents_dirs_builtin() {
        // Simulate a config from before agent support was added
        let toml_str = r#"
[integrations.claude-code]
skills_dir = "/home/user/.claude/skills"
enabled_at = "2024-01-01T00:00:00Z"
"#;
        let mut config = Config::from_toml(toml_str).unwrap();

        // Before migration: agents_dir should be None
        let integration = config.integrations.get("claude-code").unwrap();
        assert!(integration.agents_dir.is_none());

        // Run migration
        let changed = config.migrate_integration_agents_dirs();
        assert!(changed);

        // After migration: agents_dir should be set to the default
        let integration = config.integrations.get("claude-code").unwrap();
        assert_eq!(integration.agents_dir, Some("~/.claude/agents".to_string()));
    }

    #[test]
    fn test_migrate_integration_agents_dirs_preserves_existing() {
        // Config that already has agents_dir set should not be changed
        let mut config = Config::new();
        config.add_integration(
            "claude-code".to_string(),
            Some("/custom/skills".to_string()),
            Some("/custom/agents".to_string()), // Custom agents path
        );

        // Run migration
        let changed = config.migrate_integration_agents_dirs();
        assert!(!changed);

        // Should preserve the custom path
        let integration = config.integrations.get("claude-code").unwrap();
        assert_eq!(integration.agents_dir, Some("/custom/agents".to_string()));
    }

    #[test]
    fn test_migrate_integration_agents_dirs_skips_codex() {
        // Codex doesn't support individual agent files (uses AGENTS.md)
        let toml_str = r#"
[integrations.codex]
skills_dir = "/home/user/.codex/skills"
enabled_at = "2024-01-01T00:00:00Z"
"#;
        let mut config = Config::from_toml(toml_str).unwrap();

        // Run migration
        let changed = config.migrate_integration_agents_dirs();
        assert!(!changed);

        // agents_dir should remain None for codex
        let integration = config.integrations.get("codex").unwrap();
        assert!(integration.agents_dir.is_none());
    }

    #[test]
    fn test_migrate_integration_agents_dirs_custom_integration() {
        // Custom integrations (not built-in) should not be affected
        let mut config = Config::new();
        config.add_integration(
            "my-custom".to_string(),
            Some("/custom/skills".to_string()),
            None, // No agents_dir
        );

        // Run migration
        let changed = config.migrate_integration_agents_dirs();
        assert!(!changed);

        // Custom integration should not get agents_dir added
        let integration = config.integrations.get("my-custom").unwrap();
        assert!(integration.agents_dir.is_none());
    }

    #[test]
    fn test_unify_legacy_integrations() {
        let mut config = Config::new();
        config.add_integration("codex".to_string(), Some("/old/codex".to_string()), None);
        config.add_integration(
            "gemini-cli".to_string(),
            Some("/old/gemini".to_string()),
            Some("/old/gemini-agents".to_string()),
        );
        config.add_integration(
            "claude-code".to_string(),
            Some("/c/skills".to_string()),
            Some("/c/agents".to_string()),
        );

        let migrated = config.unify_legacy_integrations(Some("/home/.agents/skills".to_string()));

        // Both legacy integrations migrated; claude-code untouched.
        assert_eq!(migrated.len(), 2);
        assert!(migrated.contains(&"codex".to_string()));
        assert!(migrated.contains(&"gemini-cli".to_string()));
        assert!(!config.has_integration("codex"));
        assert!(!config.has_integration("gemini-cli"));
        assert!(config.has_integration("claude-code"));

        // Unified agents integration added (skills-only) with the shared dir.
        let agents = config.integrations.get("agents").unwrap();
        assert_eq!(agents.skills_dir, Some("/home/.agents/skills".to_string()));
        assert!(agents.agents_dir.is_none());
    }

    #[test]
    fn test_unify_legacy_integrations_noop_when_none() {
        let mut config = Config::new();
        config.add_integration(
            "claude-code".to_string(),
            Some("/c/skills".to_string()),
            None,
        );

        let migrated = config.unify_legacy_integrations(Some("/home/.agents/skills".to_string()));

        assert!(migrated.is_empty());
        assert!(!config.has_integration("agents"));
    }

    #[test]
    fn test_unify_legacy_integrations_keeps_existing_agents() {
        // If `agents` is already present, it isn't overwritten.
        let mut config = Config::new();
        config.add_integration("opencode".to_string(), Some("/old/oc".to_string()), None);
        config.add_integration(
            "agents".to_string(),
            Some("/existing/agents".to_string()),
            None,
        );

        let migrated = config.unify_legacy_integrations(Some("/home/.agents/skills".to_string()));

        assert_eq!(migrated, vec!["opencode".to_string()]);
        let agents = config.integrations.get("agents").unwrap();
        assert_eq!(agents.skills_dir, Some("/existing/agents".to_string()));
    }
}
