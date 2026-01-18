use anyhow::{bail, Context, Result};
use std::path::PathBuf;

/// The type of git source
#[derive(Debug, Clone, PartialEq)]
pub enum GitSourceType {
    /// HTTPS URL (e.g., https://github.com/owner/repo.git)
    Https,
    /// SSH URL (e.g., git@github.com:owner/repo.git)
    Ssh,
    /// Local filesystem path (e.g., /Users/dev/my-skills, ~/projects/skills, ./local)
    Local,
}

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

    /// Get the repository identifier (github.com/owner/repo or github.com/owner/repo/path)
    /// The repo ID includes the path to the directory containing skills
    pub fn repo_id(&self) -> String {
        if self.path.is_empty() {
            format!("github.com/{}/{}", self.owner, self.repo)
        } else if self.path == self.skill_name {
            // Path is just the skill name (skill at repo root)
            format!("github.com/{}/{}", self.owner, self.repo)
        } else {
            // Path contains parent dirs, extract parent directory
            let parent_path = self.path.rsplitn(2, '/').nth(1).unwrap_or("");
            if parent_path.is_empty() {
                format!("github.com/{}/{}", self.owner, self.repo)
            } else {
                format!("github.com/{}/{}/{}", self.owner, self.repo, parent_path)
            }
        }
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

/// Represents a repository reference supporting universal git sources
///
/// Supported formats:
/// - HTTPS: `https://github.com/owner/repo.git`, `gitlab.com/owner/repo`, `https://git.company.com/repo`
/// - SSH: `git@github.com:owner/repo.git`, `git@gitlab.com:group/subgroup/repo.git`
/// - Local: `/Users/dev/my-skills`, `~/projects/skills`, `./local-skills`
/// - With path: `github.com/owner/repo/nested/skills`
/// - With SHA: `github.com/owner/repo@abc12345`, `/path/to/repo@v1.0.0`
#[derive(Debug, Clone)]
pub struct RepoRef {
    /// The source type (HTTPS, SSH, or Local)
    pub source_type: GitSourceType,
    /// The original URL/path for git operations (without path suffix or SHA)
    pub git_url: String,
    /// Subdirectory path within repo (empty string for root)
    pub path: String,
    /// Optional SHA/tag from @suffix
    pub sha: Option<String>,
    /// A normalized identifier for this repo (used for config keys and cache paths)
    pub repo_id: String,
}

impl RepoRef {
    /// Parse a repository reference from various formats
    ///
    /// Supports:
    /// - HTTPS URLs: `https://github.com/owner/repo.git`, `gitlab.com/owner/repo`
    /// - SSH URLs: `git@github.com:owner/repo.git`
    /// - Local paths: `/absolute/path`, `~/home-relative`, `./current-relative`
    /// - With subdirectory: `github.com/owner/repo/subdir`
    /// - With version: `github.com/owner/repo@sha`, `/path/to/repo@tag`
    pub fn parse(reference: &str) -> Result<Self> {
        let reference = reference.trim();

        // Check for SSH URL format first (git@host:path.git)
        if reference.starts_with("git@") {
            return Self::parse_ssh(reference);
        }

        // Check for local filesystem path
        if Self::looks_like_local_path(reference) {
            return Self::parse_local(reference);
        }

        // Otherwise, treat as HTTPS URL
        Self::parse_https(reference)
    }

    /// Check if a reference looks like a local filesystem path
    fn looks_like_local_path(reference: &str) -> bool {
        // Absolute path
        reference.starts_with('/')
        // Home-relative path
        || reference.starts_with("~/")
        || reference == "~"
        // Current directory relative path
        || reference.starts_with("./")
        || reference == "."
        // Parent directory relative path
        || reference.starts_with("../")
        || reference == ".."
        // File URL
        || reference.starts_with("file://")
    }

    /// Parse an SSH URL (git@host:owner/repo.git or git@host:owner/repo.git/path)
    fn parse_ssh(reference: &str) -> Result<Self> {
        // Split on @ to extract SHA if present (but not the git@ prefix)
        let (reference, sha) = Self::extract_sha_suffix(reference)?;

        // Format: git@host:owner/repo.git or git@host:owner/repo.git/path
        let without_prefix = reference
            .strip_prefix("git@")
            .context("SSH URL must start with git@")?;

        // Find the colon separating host from path
        let colon_pos = without_prefix
            .find(':')
            .context("SSH URL must contain ':' after host")?;

        let host = &without_prefix[..colon_pos];
        let rest = &without_prefix[colon_pos + 1..];

        // Split on .git to separate repo from path
        let (repo_part, path) = if let Some(git_pos) = rest.find(".git") {
            let repo = &rest[..git_pos + 4]; // Include .git
            let after_git = &rest[git_pos + 4..];
            let path = after_git.strip_prefix('/').unwrap_or(after_git);
            (repo.to_string(), path.to_string())
        } else {
            // No .git suffix - treat entire rest as repo path, possibly with subdirectory
            // Try to detect if there's a path suffix by counting slashes
            // Typical: owner/repo or group/subgroup/repo
            // With path: owner/repo/subdir
            (rest.to_string(), String::new())
        };

        // Construct the git URL for cloning
        let git_url = format!("git@{}:{}", host, repo_part);

        // Create repo_id from host and repo path (without .git suffix)
        let repo_path_clean = repo_part.strip_suffix(".git").unwrap_or(&repo_part);
        let repo_id = format!("{}/{}", host, repo_path_clean);

        Ok(Self {
            source_type: GitSourceType::Ssh,
            git_url,
            path,
            sha,
            repo_id,
        })
    }

    /// Parse a local filesystem path
    fn parse_local(reference: &str) -> Result<Self> {
        // Split on @ to extract SHA/tag if present
        let (reference, sha) = Self::extract_sha_suffix(reference)?;

        // Handle file:// URLs
        let path_str = reference
            .strip_prefix("file://")
            .unwrap_or(reference);

        // Expand ~ to home directory
        let expanded = if path_str.starts_with("~/") || path_str == "~" {
            let home = dirs::home_dir()
                .context("Could not find home directory")?;
            if path_str == "~" {
                home
            } else {
                home.join(&path_str[2..])
            }
        } else {
            PathBuf::from(path_str)
        };

        // Canonicalize relative paths to absolute (but don't require existence yet)
        let canonical = if expanded.is_relative() {
            std::env::current_dir()
                .context("Could not get current directory")?
                .join(&expanded)
        } else {
            expanded
        };

        // Use the canonical path as both git_url and for generating repo_id
        let path_string = canonical.to_string_lossy().to_string();

        // Create a repo_id that's safe for use in config and cache
        // For local paths, we use a hash-based approach to handle special characters
        let repo_id = Self::local_path_to_repo_id(&path_string);

        Ok(Self {
            source_type: GitSourceType::Local,
            git_url: path_string,
            path: String::new(), // Local paths don't have subdirectory paths (the path IS the repo)
            sha,
            repo_id,
        })
    }

    /// Convert a local path to a safe repo_id
    fn local_path_to_repo_id(path: &str) -> String {
        // Use "local:" prefix followed by path with slashes converted
        // e.g., /Users/dev/my-skills -> local:/Users/dev/my-skills
        format!("local:{}", path)
    }

    /// Parse an HTTPS URL or shorthand (github.com/owner/repo)
    fn parse_https(reference: &str) -> Result<Self> {
        // Split on @ to extract SHA if present
        let (reference, sha) = Self::extract_sha_suffix(reference)?;

        // Normalize: add https:// if missing, handle various prefixes
        let normalized = if reference.starts_with("https://") || reference.starts_with("http://") {
            reference.to_string()
        } else {
            format!("https://{}", reference)
        };

        // Parse the URL to extract components
        // Format: https://host/path/to/repo.git or https://host/owner/repo/subpath
        let url = normalized.strip_prefix("https://")
            .or_else(|| normalized.strip_prefix("http://"))
            .context("Invalid URL format")?;

        // Split into host and path
        let slash_pos = url.find('/').context("URL must contain a path")?;
        let host = &url[..slash_pos];
        let url_path = &url[slash_pos + 1..];

        // Determine where the repo ends and subdirectory begins
        // If .git is present, that marks the end of the repo
        let (repo_path, subdir_path) = if let Some(git_pos) = url_path.find(".git") {
            let repo = &url_path[..git_pos];
            let after_git = &url_path[git_pos + 4..];
            let subdir = after_git.strip_prefix('/').unwrap_or(after_git);
            (repo.to_string(), subdir.to_string())
        } else {
            // No .git suffix - for well-known hosts, assume owner/repo format
            // For other hosts, treat the entire path as repo
            if Self::is_well_known_git_host(host) {
                // Try to extract owner/repo from path
                let parts: Vec<&str> = url_path.split('/').collect();
                if parts.len() >= 2 {
                    let repo = format!("{}/{}", parts[0], parts[1]);
                    let subdir = if parts.len() > 2 {
                        parts[2..].join("/")
                    } else {
                        String::new()
                    };
                    (repo, subdir)
                } else {
                    (url_path.to_string(), String::new())
                }
            } else {
                (url_path.to_string(), String::new())
            }
        };

        // Construct the git URL (with .git suffix for cloning)
        let git_url = if repo_path.ends_with(".git") {
            format!("https://{}/{}", host, repo_path)
        } else {
            format!("https://{}/{}.git", host, repo_path)
        };

        // Create repo_id from host and repo path
        let repo_id = format!("{}/{}", host, repo_path);

        Ok(Self {
            source_type: GitSourceType::Https,
            git_url,
            path: subdir_path,
            sha,
            repo_id,
        })
    }

    /// Check if a host is a well-known git hosting service
    fn is_well_known_git_host(host: &str) -> bool {
        matches!(
            host,
            "github.com" | "gitlab.com" | "bitbucket.org" | "codeberg.org" | "gitea.com"
        )
    }

    /// Extract @SHA or @tag suffix from a reference
    fn extract_sha_suffix(reference: &str) -> Result<(&str, Option<String>)> {
        // Don't split on @ for SSH URLs (git@...)
        if reference.starts_with("git@") {
            // Find the last @ that isn't the git@ prefix
            let rest = &reference[4..]; // Skip "git@"
            if let Some(at_pos) = rest.rfind('@') {
                let (_ref_part, sha_part) = rest.split_at(at_pos);
                let sha_str = &sha_part[1..]; // Skip the @ character
                Self::validate_sha(sha_str)?;
                return Ok((&reference[..4 + at_pos], Some(sha_str.to_string())));
            }
            return Ok((reference, None));
        }

        // For other formats, find last @
        if let Some(at_pos) = reference.rfind('@') {
            let (ref_part, sha_part) = reference.split_at(at_pos);
            let sha_str = &sha_part[1..]; // Skip the @ character
            Self::validate_sha(sha_str)?;
            Ok((ref_part, Some(sha_str.to_string())))
        } else {
            Ok((reference, None))
        }
    }

    /// Validate a SHA or tag reference
    fn validate_sha(sha: &str) -> Result<()> {
        // Allow tags (like v1.0.0) and SHAs
        // SHAs are 7-40 hex chars, tags can be alphanumeric with dots/dashes
        if sha.is_empty() {
            bail!("Version reference cannot be empty");
        }

        // Check if it looks like a SHA (all hex)
        if sha.chars().all(|c| c.is_ascii_hexdigit()) {
            if sha.len() < 7 || sha.len() > 40 {
                bail!("Invalid SHA: must be between 7 and 40 characters");
            }
        }
        // Otherwise accept as a tag reference (git will validate)

        Ok(())
    }

    /// Get the repository identifier (used for config keys)
    pub fn id(&self) -> &str {
        &self.repo_id
    }

    /// Get the full reference including path (if any)
    pub fn full_ref(&self) -> String {
        if self.path.is_empty() {
            self.repo_id.clone()
        } else {
            format!("{}/{}", self.repo_id, self.path)
        }
    }

    /// Get the URL to use for git clone
    pub fn clone_url(&self) -> &str {
        &self.git_url
    }

    /// Get the cache directory path for this repository
    /// Returns a path relative to the git cache root
    pub fn cache_path(&self) -> std::path::PathBuf {
        use std::path::PathBuf;

        match self.source_type {
            GitSourceType::Https | GitSourceType::Ssh => {
                // For remote repos, use host/owner/repo structure
                // repo_id format: "host/path" or "host/owner/repo"
                let parts: Vec<&str> = self.repo_id.split('/').collect();
                if parts.len() >= 2 {
                    PathBuf::from(parts[0]).join(parts[1..].join("/"))
                } else {
                    PathBuf::from(&self.repo_id)
                }
            }
            GitSourceType::Local => {
                // For local repos, use a hash of the path to avoid deep nesting
                use std::collections::hash_map::DefaultHasher;
                use std::hash::{Hash, Hasher};

                let mut hasher = DefaultHasher::new();
                self.git_url.hash(&mut hasher);
                let hash = format!("{:016x}", hasher.finish());

                // Get the directory name from the path
                let dir_name = std::path::Path::new(&self.git_url)
                    .file_name()
                    .map(|s| s.to_string_lossy().to_string())
                    .unwrap_or_else(|| "local".to_string());

                PathBuf::from("local").join(format!("{}-{}", dir_name, &hash[..8]))
            }
        }
    }

    /// Legacy helper: extract owner from repo_id for backward compatibility with GitHub URLs
    /// Returns None for non-GitHub or local repos
    pub fn github_owner(&self) -> Option<&str> {
        if self.repo_id.starts_with("github.com/") {
            let path = &self.repo_id["github.com/".len()..];
            path.split('/').next()
        } else {
            None
        }
    }

    /// Legacy helper: extract repo name from repo_id for backward compatibility with GitHub URLs
    /// Returns None for non-GitHub or local repos
    pub fn github_repo(&self) -> Option<&str> {
        if self.repo_id.starts_with("github.com/") {
            let path = &self.repo_id["github.com/".len()..];
            let parts: Vec<&str> = path.split('/').collect();
            if parts.len() >= 2 {
                Some(parts[1])
            } else {
                None
            }
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ===== SkillRef tests (unchanged) =====

    #[test]
    fn test_parse_skill_ref() {
        let skill = SkillRef::parse("github.com/testowner/test-skills/git-commit").unwrap();
        assert_eq!(skill.owner, "testowner");
        assert_eq!(skill.repo, "test-skills");
        assert_eq!(skill.path, "git-commit");
        assert_eq!(skill.skill_name, "git-commit");
    }

    #[test]
    fn test_parse_skill_ref_with_skill_md() {
        let skill = SkillRef::parse("github.com/testowner/test-skills/git-commit/SKILL.md").unwrap();
        assert_eq!(skill.path, "git-commit");
        assert_eq!(skill.skill_name, "git-commit");
    }

    #[test]
    fn test_parse_nested_skill() {
        let skill = SkillRef::parse("github.com/testowner/nested/directory/my-skill").unwrap();
        assert_eq!(skill.owner, "testowner");
        assert_eq!(skill.repo, "nested");
        assert_eq!(skill.path, "directory/my-skill");
        assert_eq!(skill.skill_name, "my-skill");
    }

    // ===== RepoRef HTTPS tests =====

    #[test]
    fn test_parse_repo_ref_github() {
        let repo = RepoRef::parse("github.com/testowner/test-skills").unwrap();
        assert_eq!(repo.source_type, GitSourceType::Https);
        assert_eq!(repo.repo_id, "github.com/testowner/test-skills");
        assert_eq!(repo.git_url, "https://github.com/testowner/test-skills.git");
        assert_eq!(repo.path, "");
        assert_eq!(repo.sha, None);
    }

    #[test]
    fn test_parse_repo_ref_github_with_https() {
        let repo = RepoRef::parse("https://github.com/testowner/test-skills").unwrap();
        assert_eq!(repo.source_type, GitSourceType::Https);
        assert_eq!(repo.repo_id, "github.com/testowner/test-skills");
    }

    #[test]
    fn test_parse_repo_ref_github_with_git_suffix() {
        let repo = RepoRef::parse("https://github.com/testowner/test-skills.git").unwrap();
        assert_eq!(repo.source_type, GitSourceType::Https);
        assert_eq!(repo.repo_id, "github.com/testowner/test-skills");
        assert_eq!(repo.git_url, "https://github.com/testowner/test-skills.git");
    }

    #[test]
    fn test_parse_repo_ref_gitlab() {
        let repo = RepoRef::parse("gitlab.com/testteam/test-skills").unwrap();
        assert_eq!(repo.source_type, GitSourceType::Https);
        assert_eq!(repo.repo_id, "gitlab.com/testteam/test-skills");
        assert_eq!(repo.git_url, "https://gitlab.com/testteam/test-skills.git");
    }

    #[test]
    fn test_parse_repo_ref_gitlab_with_https() {
        let repo = RepoRef::parse("https://gitlab.com/test.user/testfiles.git").unwrap();
        assert_eq!(repo.source_type, GitSourceType::Https);
        assert_eq!(repo.repo_id, "gitlab.com/test.user/testfiles");
        assert_eq!(repo.git_url, "https://gitlab.com/test.user/testfiles.git");
    }

    #[test]
    fn test_parse_repo_ref_bitbucket() {
        let repo = RepoRef::parse("bitbucket.org/testteam/testproject").unwrap();
        assert_eq!(repo.source_type, GitSourceType::Https);
        assert_eq!(repo.repo_id, "bitbucket.org/testteam/testproject");
    }

    #[test]
    fn test_parse_repo_ref_self_hosted() {
        let repo = RepoRef::parse("https://git.example.com/test-repo.git").unwrap();
        assert_eq!(repo.source_type, GitSourceType::Https);
        assert_eq!(repo.repo_id, "git.example.com/test-repo");
    }

    #[test]
    fn test_parse_repo_ref_with_path() {
        let repo = RepoRef::parse("github.com/testowner/test-skills/nested/skills").unwrap();
        assert_eq!(repo.repo_id, "github.com/testowner/test-skills");
        assert_eq!(repo.path, "nested/skills");
    }

    #[test]
    fn test_parse_repo_ref_with_git_and_path() {
        let repo = RepoRef::parse("https://github.com/testowner/testrepo.git/subdir").unwrap();
        assert_eq!(repo.repo_id, "github.com/testowner/testrepo");
        assert_eq!(repo.path, "subdir");
    }

    #[test]
    fn test_parse_repo_ref_with_sha() {
        let repo = RepoRef::parse("github.com/testowner/test-skills@489c9d85").unwrap();
        assert_eq!(repo.repo_id, "github.com/testowner/test-skills");
        assert_eq!(repo.sha, Some("489c9d85".to_string()));
    }

    #[test]
    fn test_parse_repo_ref_with_full_sha() {
        let repo = RepoRef::parse("github.com/testowner/test-skills@489c9d85c422a184feb9f56dc7e60e4af721a131").unwrap();
        assert_eq!(repo.sha, Some("489c9d85c422a184feb9f56dc7e60e4af721a131".to_string()));
    }

    #[test]
    fn test_parse_repo_ref_with_path_and_sha() {
        let repo = RepoRef::parse("github.com/testorg/skills/skills@abc12345").unwrap();
        assert_eq!(repo.repo_id, "github.com/testorg/skills");
        assert_eq!(repo.path, "skills");
        assert_eq!(repo.sha, Some("abc12345".to_string()));
    }

    #[test]
    fn test_parse_repo_ref_invalid_sha_too_short() {
        let result = RepoRef::parse("github.com/testowner/testrepo@abc");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_repo_ref_invalid_sha_non_hex() {
        // This should actually pass now since we support tags like v1.0.0
        let result = RepoRef::parse("github.com/testowner/testrepo@v1.0.0");
        assert!(result.is_ok());
    }

    // ===== RepoRef SSH tests =====

    #[test]
    fn test_parse_ssh_github() {
        let repo = RepoRef::parse("git@github.com:testowner/testrepo.git").unwrap();
        assert_eq!(repo.source_type, GitSourceType::Ssh);
        assert_eq!(repo.repo_id, "github.com/testowner/testrepo");
        assert_eq!(repo.git_url, "git@github.com:testowner/testrepo.git");
        assert_eq!(repo.path, "");
    }

    #[test]
    fn test_parse_ssh_gitlab() {
        let repo = RepoRef::parse("git@gitlab.com:test.user/testfiles.git").unwrap();
        assert_eq!(repo.source_type, GitSourceType::Ssh);
        assert_eq!(repo.repo_id, "gitlab.com/test.user/testfiles");
        assert_eq!(repo.git_url, "git@gitlab.com:test.user/testfiles.git");
    }

    #[test]
    fn test_parse_ssh_gitlab_group() {
        let repo = RepoRef::parse("git@gitlab.com:testgroup/subgroup/testrepo.git").unwrap();
        assert_eq!(repo.source_type, GitSourceType::Ssh);
        assert_eq!(repo.repo_id, "gitlab.com/testgroup/subgroup/testrepo");
        assert_eq!(repo.git_url, "git@gitlab.com:testgroup/subgroup/testrepo.git");
    }

    #[test]
    fn test_parse_ssh_with_path() {
        let repo = RepoRef::parse("git@github.com:testowner/testrepo.git/skills").unwrap();
        assert_eq!(repo.source_type, GitSourceType::Ssh);
        assert_eq!(repo.repo_id, "github.com/testowner/testrepo");
        assert_eq!(repo.path, "skills");
    }

    #[test]
    fn test_parse_ssh_with_sha() {
        let repo = RepoRef::parse("git@github.com:testowner/testrepo.git@abc12345").unwrap();
        assert_eq!(repo.source_type, GitSourceType::Ssh);
        assert_eq!(repo.repo_id, "github.com/testowner/testrepo");
        assert_eq!(repo.sha, Some("abc12345".to_string()));
    }

    // ===== RepoRef Local path tests =====

    #[test]
    fn test_parse_local_absolute() {
        let repo = RepoRef::parse("/Users/dev/my-skills").unwrap();
        assert_eq!(repo.source_type, GitSourceType::Local);
        assert_eq!(repo.git_url, "/Users/dev/my-skills");
        assert_eq!(repo.repo_id, "local:/Users/dev/my-skills");
        assert_eq!(repo.path, "");
    }

    #[test]
    fn test_parse_local_home_relative() {
        let repo = RepoRef::parse("~/projects/skills").unwrap();
        assert_eq!(repo.source_type, GitSourceType::Local);
        // The path should be expanded
        assert!(repo.git_url.contains("projects/skills"));
        assert!(repo.repo_id.starts_with("local:"));
    }

    #[test]
    fn test_parse_local_current_relative() {
        let repo = RepoRef::parse("./local-skills").unwrap();
        assert_eq!(repo.source_type, GitSourceType::Local);
        // The path should be expanded to absolute
        assert!(repo.git_url.ends_with("local-skills"));
        assert!(repo.repo_id.starts_with("local:"));
    }

    #[test]
    fn test_parse_local_with_sha() {
        let repo = RepoRef::parse("/Users/dev/my-skills@abc12345").unwrap();
        assert_eq!(repo.source_type, GitSourceType::Local);
        assert_eq!(repo.git_url, "/Users/dev/my-skills");
        assert_eq!(repo.sha, Some("abc12345".to_string()));
    }

    #[test]
    fn test_parse_local_with_tag() {
        let repo = RepoRef::parse("/Users/dev/my-skills@v1.0.0").unwrap();
        assert_eq!(repo.source_type, GitSourceType::Local);
        assert_eq!(repo.sha, Some("v1.0.0".to_string()));
    }

    #[test]
    fn test_parse_local_with_spaces() {
        let repo = RepoRef::parse("/Users/dev/my skills/repo").unwrap();
        assert_eq!(repo.source_type, GitSourceType::Local);
        assert_eq!(repo.git_url, "/Users/dev/my skills/repo");
    }

    #[test]
    fn test_parse_file_url() {
        let repo = RepoRef::parse("file:///Users/dev/my-skills").unwrap();
        assert_eq!(repo.source_type, GitSourceType::Local);
        assert_eq!(repo.git_url, "/Users/dev/my-skills");
    }

    // ===== Helper method tests =====

    #[test]
    fn test_looks_like_local_path() {
        assert!(RepoRef::looks_like_local_path("/absolute/path"));
        assert!(RepoRef::looks_like_local_path("~/home/path"));
        assert!(RepoRef::looks_like_local_path("./relative"));
        assert!(RepoRef::looks_like_local_path("../parent"));
        assert!(RepoRef::looks_like_local_path("file:///path"));

        assert!(!RepoRef::looks_like_local_path("github.com/testowner/testrepo"));
        assert!(!RepoRef::looks_like_local_path("https://github.com/testowner/testrepo"));
        assert!(!RepoRef::looks_like_local_path("git@github.com:testowner/testrepo.git"));
    }

    #[test]
    fn test_clone_url() {
        let https = RepoRef::parse("github.com/testowner/testrepo").unwrap();
        assert_eq!(https.clone_url(), "https://github.com/testowner/testrepo.git");

        let ssh = RepoRef::parse("git@github.com:testowner/testrepo.git").unwrap();
        assert_eq!(ssh.clone_url(), "git@github.com:testowner/testrepo.git");

        let local = RepoRef::parse("/Users/dev/testrepo").unwrap();
        assert_eq!(local.clone_url(), "/Users/dev/testrepo");
    }

    #[test]
    fn test_full_ref() {
        let repo = RepoRef::parse("github.com/testowner/testrepo/subdir").unwrap();
        assert_eq!(repo.full_ref(), "github.com/testowner/testrepo/subdir");

        let repo_no_path = RepoRef::parse("github.com/testowner/testrepo").unwrap();
        assert_eq!(repo_no_path.full_ref(), "github.com/testowner/testrepo");
    }
}
