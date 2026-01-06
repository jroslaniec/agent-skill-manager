use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Config {
    #[serde(default)]
    pub repositories: HashMap<String, Repository>,
    #[serde(default)]
    pub skills: HashMap<String, Skill>,
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
}
