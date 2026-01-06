use anyhow::{bail, Context, Result};

/// Represents a parsed skill reference (e.g., github.com/owner/repo/path/to/skill)
#[derive(Debug, Clone)]
pub struct SkillRef {
    pub owner: String,
    pub repo: String,
    pub path: String,        // Path within repo (e.g., "nested/skills" or "git-commit")
    pub skill_name: String,  // The skill directory name (e.g., "git-commit")
}

impl SkillRef {
    /// Parse a skill reference like:
    /// - github.com/owner/repo/skill-dir
    /// - github.com/owner/repo/skill-dir/SKILL.md
    /// - github.com/owner/repo/nested/path/skill-dir
    pub fn parse(reference: &str) -> Result<Self> {
        let reference = reference.trim();

        // Remove github.com prefix if present
        let reference = reference
            .strip_prefix("https://github.com/")
            .or_else(|| reference.strip_prefix("http://github.com/"))
            .or_else(|| reference.strip_prefix("github.com/"))
            .context("Skill reference must start with github.com/")?;

        // Split into parts
        let parts: Vec<&str> = reference.split('/').collect();

        if parts.len() < 3 {
            bail!("Invalid skill reference: must be github.com/OWNER/REPO/PATH");
        }

        let owner = parts[0].to_string();
        let repo = parts[1].to_string();

        // Get the path (everything after owner/repo)
        let mut path_parts = parts[2..].to_vec();

        // Remove SKILL.md if present at the end
        if path_parts.last() == Some(&"SKILL.md") {
            path_parts.pop();
        }

        if path_parts.is_empty() {
            bail!("Invalid skill reference: must include a skill directory");
        }

        // The skill name is the last directory in the path
        let skill_name = path_parts.last().unwrap().to_string();
        let path = path_parts.join("/");

        Ok(Self {
            owner,
            repo,
            path,
            skill_name,
        })
    }

    /// Get the repository identifier (github.com/owner/repo)
    pub fn repo_id(&self) -> String {
        format!("github.com/{}/{}", self.owner, self.repo)
    }

    /// Get the git clone URL
    pub fn git_url(&self) -> String {
        format!("https://github.com/{}/{}.git", self.owner, self.repo)
    }

    /// Get the full reference (github.com/owner/repo/path)
    pub fn full_ref(&self) -> String {
        format!("{}/{}", self.repo_id(), self.path)
    }
}

/// Represents a repository reference (e.g., github.com/owner/repo or github.com/owner/repo/subpath)
#[derive(Debug, Clone)]
pub struct RepoRef {
    pub owner: String,
    pub repo: String,
    pub path: String,  // Subdirectory path within repo (empty string for root)
}

impl RepoRef {
    /// Parse a repository reference like:
    /// - github.com/owner/repo
    /// - github.com/owner/repo/nested/skills
    pub fn parse(reference: &str) -> Result<Self> {
        let reference = reference.trim();

        // Remove github.com prefix if present
        let reference = reference
            .strip_prefix("https://github.com/")
            .or_else(|| reference.strip_prefix("http://github.com/"))
            .or_else(|| reference.strip_prefix("github.com/"))
            .context("Repository reference must start with github.com/")?;

        // Split into parts
        let parts: Vec<&str> = reference.split('/').collect();

        if parts.len() < 2 {
            bail!("Invalid repository reference: must be github.com/OWNER/REPO");
        }

        let owner = parts[0].to_string();
        let repo = parts[1].to_string();
        let path = if parts.len() > 2 {
            parts[2..].join("/")
        } else {
            String::new()
        };

        Ok(Self { owner, repo, path })
    }

    /// Get the repository identifier (github.com/owner/repo)
    pub fn repo_id(&self) -> String {
        format!("github.com/{}/{}", self.owner, self.repo)
    }

    /// Get the full reference including path
    pub fn full_ref(&self) -> String {
        if self.path.is_empty() {
            self.repo_id()
        } else {
            format!("{}/{}", self.repo_id(), self.path)
        }
    }

    /// Get the git clone URL
    pub fn git_url(&self) -> String {
        format!("https://github.com/{}/{}.git", self.owner, self.repo)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_skill_ref() {
        let skill = SkillRef::parse("github.com/jroslaniec/agent-skills/git-commit").unwrap();
        assert_eq!(skill.owner, "jroslaniec");
        assert_eq!(skill.repo, "agent-skills");
        assert_eq!(skill.path, "git-commit");
        assert_eq!(skill.skill_name, "git-commit");
    }

    #[test]
    fn test_parse_skill_ref_with_skill_md() {
        let skill = SkillRef::parse("github.com/jroslaniec/agent-skills/git-commit/SKILL.md").unwrap();
        assert_eq!(skill.path, "git-commit");
        assert_eq!(skill.skill_name, "git-commit");
    }

    #[test]
    fn test_parse_nested_skill() {
        let skill = SkillRef::parse("github.com/jroslaniec/nested/directory/my-skill").unwrap();
        assert_eq!(skill.owner, "jroslaniec");
        assert_eq!(skill.repo, "nested");
        assert_eq!(skill.path, "directory/my-skill");
        assert_eq!(skill.skill_name, "my-skill");
    }

    #[test]
    fn test_parse_repo_ref() {
        let repo = RepoRef::parse("github.com/jroslaniec/agent-skills").unwrap();
        assert_eq!(repo.owner, "jroslaniec");
        assert_eq!(repo.repo, "agent-skills");
        assert_eq!(repo.path, "");
    }

    #[test]
    fn test_parse_repo_ref_with_path() {
        let repo = RepoRef::parse("github.com/jroslaniec/agent-skills/nested/skills").unwrap();
        assert_eq!(repo.path, "nested/skills");
    }
}
