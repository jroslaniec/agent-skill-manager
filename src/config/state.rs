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
    pub path: String,  // Subdirectory path within repo (empty for root)
    #[serde(default = "chrono_now")]
    pub cloned_at: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub current_sha: Option<String>,  // Current checked-out commit (12 chars)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pinned_sha: Option<String>,   // If set, repo is pinned to this SHA
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Skill {
    pub repository: String,  // Repository ID (e.g., "github.com/owner/repo")
    pub skill_path: String,  // Path within repo (e.g., "git-commit")
    pub enabled: bool,
    #[serde(default = "chrono_now")]
    pub enabled_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub disabled_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Agent {
    pub repository: String,  // Repository ID (e.g., "github.com/owner/repo")
    pub agent_path: String,  // Path within repo (e.g., "code-reviewer")
    pub enabled: bool,
    #[serde(default = "chrono_now")]
    pub enabled_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub disabled_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Integration {
    pub skills_dir: String,  // Path to skills directory (e.g., "~/.claude/skills")
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
    pub fn add_repository(&mut self, repo_id: String, url: String, path: String, current_sha: Option<String>, pinned_sha: Option<String>) {
        self.repositories.insert(
            repo_id,
            Repository {
                url,
                path,
                cloned_at: chrono_now(),
                current_sha,
                pinned_sha,
            },
        );
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
    pub fn add_integration(&mut self, name: String, skills_dir: String) {
        self.integrations.insert(
            name,
            Integration {
                skills_dir,
                enabled_at: chrono_now(),
            },
        );
    }

    /// Remove an integration
    pub fn remove_integration(&mut self, name: &str) -> Option<Integration> {
        self.integrations.remove(name)
    }

    /// Get all enabled skills
    pub fn enabled_skills(&self) -> Vec<(&String, &Skill)> {
        self.skills
            .iter()
            .filter(|(_, skill)| skill.enabled)
            .collect()
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
}
