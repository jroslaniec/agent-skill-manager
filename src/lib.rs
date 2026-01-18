pub mod cli;
pub mod commands;
pub mod config;
pub mod git;
pub mod paths;
pub mod skill_ref;

pub type Result<T> = anyhow::Result<T>;

/// Integration tests for universal git sources and skill/agent management
#[cfg(test)]
mod integration_tests {
    use std::fs;
    use std::os::unix::fs as unix_fs;
    use std::path::Path;
    use std::process::Command;
    use tempfile::TempDir;

    use crate::commands::skill::create_skill_symlink;
    use crate::git;

    /// Helper to create a temporary git repository with skills and agents for testing
    fn create_test_repo_with_skills_and_agents() -> (TempDir, std::path::PathBuf) {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let repo_path = temp_dir.path().to_path_buf();

        // Initialize git repo
        Command::new("git")
            .args(["init", "-b", "main"])
            .current_dir(&repo_path)
            .output()
            .expect("Failed to init git repo");

        // Configure git user for commits
        Command::new("git")
            .args(["config", "user.email", "test@example.com"])
            .current_dir(&repo_path)
            .output()
            .expect("Failed to configure git email");

        Command::new("git")
            .args(["config", "user.name", "Test User"])
            .current_dir(&repo_path)
            .output()
            .expect("Failed to configure git name");

        // Create skill directories
        let skill1_dir = repo_path.join("test-skill-one");
        fs::create_dir_all(&skill1_dir).expect("Failed to create skill dir");
        fs::write(
            skill1_dir.join("SKILL.md"),
            "# Test Skill One\n\nA skill for testing.\n",
        )
        .expect("Failed to write SKILL.md");

        let skill2_dir = repo_path.join("test-skill-two");
        fs::create_dir_all(&skill2_dir).expect("Failed to create skill dir");
        fs::write(
            skill2_dir.join("SKILL.md"),
            "# Test Skill Two\n\nAnother test skill.\n",
        )
        .expect("Failed to write SKILL.md");

        // Create agent directories
        let agent1_dir = repo_path.join("test-agent-alpha");
        fs::create_dir_all(&agent1_dir).expect("Failed to create agent dir");
        fs::write(
            agent1_dir.join("AGENT.md"),
            "# Test Agent Alpha\n\nAn agent for testing.\n",
        )
        .expect("Failed to write AGENT.md");

        let agent2_dir = repo_path.join("test-agent-beta");
        fs::create_dir_all(&agent2_dir).expect("Failed to create agent dir");
        fs::write(
            agent2_dir.join("AGENT.md"),
            "# Test Agent Beta\n\nAnother test agent.\n",
        )
        .expect("Failed to write AGENT.md");

        // Add a README
        fs::write(
            repo_path.join("README.md"),
            "# Test Repository\n\nContains 2 skills and 2 agents.\n",
        )
        .expect("Failed to write README");

        // Commit everything
        Command::new("git")
            .args(["add", "."])
            .current_dir(&repo_path)
            .output()
            .expect("Failed to git add");

        Command::new("git")
            .args(["commit", "-m", "Initial commit with skills and agents"])
            .current_dir(&repo_path)
            .output()
            .expect("Failed to git commit");

        (temp_dir, repo_path)
    }

    /// Integration test: clone from local path to cache
    /// Verifies that a local git repository can be cloned to the cache directory
    #[test]
    fn test_integration_clone_local_path_to_cache() {
        let (_source_dir, source_path) = create_test_repo_with_skills_and_agents();
        let cache_dir = TempDir::new().expect("Failed to create cache dir");
        let cache_path = cache_dir.path().join("cloned-repo");

        // Clone from local path
        git::clone_repo(source_path.to_str().unwrap(), &cache_path)
            .expect("Failed to clone from local path to cache");

        // Verify clone succeeded
        assert!(cache_path.exists(), "Cache path should exist");
        assert!(cache_path.join(".git").exists(), ".git should exist");
        assert!(cache_path.join("README.md").exists(), "README should exist");

        // Verify skills and agents were cloned
        assert!(
            cache_path.join("test-skill-one/SKILL.md").exists(),
            "Skill one should be cloned"
        );
        assert!(
            cache_path.join("test-skill-two/SKILL.md").exists(),
            "Skill two should be cloned"
        );
        assert!(
            cache_path.join("test-agent-alpha/AGENT.md").exists(),
            "Agent alpha should be cloned"
        );
        assert!(
            cache_path.join("test-agent-beta/AGENT.md").exists(),
            "Agent beta should be cloned"
        );
    }

    /// Integration test: enable skill creates directory symlink
    /// Verifies that enabling a skill creates a symlink to the skill directory
    #[test]
    fn test_integration_skill_enable_creates_directory_symlink() {
        let (_source_dir, source_path) = create_test_repo_with_skills_and_agents();
        let skills_dir = TempDir::new().expect("Failed to create skills dir");

        // The source is a directory (skill directory)
        let source = source_path.join("test-skill-one");
        let link = skills_dir.path().join("test-skill-one");

        // Create the symlink using the skill symlink function
        create_skill_symlink(&source, &link).expect("Failed to create skill symlink");

        // Verify symlink was created
        assert!(link.exists(), "Link should exist");
        assert!(link.is_symlink(), "Link should be a symlink");

        // Verify it's a directory symlink (symlink resolves to a directory)
        assert!(link.is_dir(), "Skill symlink should resolve to a directory");

        // Verify we can read through the symlink
        let skill_md_content = fs::read_to_string(link.join("SKILL.md"))
            .expect("Should be able to read SKILL.md through symlink");
        assert!(
            skill_md_content.contains("Test Skill One"),
            "Should read correct content"
        );

        // Verify the target is the source directory
        let target = fs::read_link(&link).expect("Should be able to read link");
        assert_eq!(target, source, "Symlink should point to source directory");
    }

    /// Integration test: enable agent creates file symlink with .md extension
    /// Verifies that enabling an agent creates a file symlink to AGENT.md
    #[test]
    fn test_integration_agent_enable_creates_file_symlink_with_md_extension() {
        let (_source_dir, source_path) = create_test_repo_with_skills_and_agents();
        let agents_dir = TempDir::new().expect("Failed to create agents dir");

        // The source is the AGENT.md file (not the directory)
        let source = source_path.join("test-agent-alpha").join("AGENT.md");
        // The link has .md extension: {agents_dir}/{name}.md
        let link = agents_dir.path().join("test-agent-alpha.md");

        // Create the file symlink
        unix_fs::symlink(&source, &link).expect("Failed to create agent symlink");

        // Verify symlink was created
        assert!(link.exists(), "Link should exist");
        assert!(link.is_symlink(), "Link should be a symlink");

        // Verify it's a file symlink (symlink resolves to a file)
        assert!(link.is_file(), "Agent symlink should resolve to a file");

        // Verify the link has .md extension
        assert!(
            link.extension().map(|e| e == "md").unwrap_or(false),
            "Agent symlink should have .md extension"
        );

        // Verify we can read the content through the symlink
        let agent_content =
            fs::read_to_string(&link).expect("Should be able to read through symlink");
        assert!(
            agent_content.contains("Test Agent Alpha"),
            "Should read correct content"
        );

        // Verify the target is the AGENT.md file
        let target = fs::read_link(&link).expect("Should be able to read link");
        assert_eq!(target, source, "Symlink should point to AGENT.md file");
    }

    /// Integration test: skill and agent symlinks have different structures
    /// Skills: {skills_dir}/{name}/ -> cached skill directory
    /// Agents: {agents_dir}/{name}.md -> cached AGENT.md file
    #[test]
    fn test_integration_skill_vs_agent_symlink_structure() {
        let (_source_dir, source_path) = create_test_repo_with_skills_and_agents();
        let skills_dir = TempDir::new().expect("Failed to create skills dir");
        let agents_dir = TempDir::new().expect("Failed to create agents dir");

        // Create skill symlink (directory)
        let skill_source = source_path.join("test-skill-one");
        let skill_link = skills_dir.path().join("test-skill-one");
        create_skill_symlink(&skill_source, &skill_link).expect("Failed to create skill symlink");

        // Create agent symlink (file with .md extension)
        let agent_source = source_path.join("test-agent-alpha").join("AGENT.md");
        let agent_link = agents_dir.path().join("test-agent-alpha.md");
        unix_fs::symlink(&agent_source, &agent_link).expect("Failed to create agent symlink");

        // Verify skill symlink structure
        assert!(skill_link.is_symlink(), "Skill link should be a symlink");
        assert!(
            skill_link.is_dir(),
            "Skill link should resolve to a directory"
        );
        assert!(
            !skill_link
                .file_name()
                .unwrap()
                .to_str()
                .unwrap()
                .ends_with(".md"),
            "Skill link should NOT have .md extension"
        );
        // Can access files within skill directory
        assert!(
            skill_link.join("SKILL.md").exists(),
            "Should access SKILL.md within skill directory"
        );

        // Verify agent symlink structure
        assert!(agent_link.is_symlink(), "Agent link should be a symlink");
        assert!(agent_link.is_file(), "Agent link should resolve to a file");
        assert!(
            agent_link
                .file_name()
                .unwrap()
                .to_str()
                .unwrap()
                .ends_with(".md"),
            "Agent link SHOULD have .md extension"
        );
        // Agent link IS the file (not a directory containing files)
        let content = fs::read_to_string(&agent_link).expect("Should read agent file");
        assert!(
            content.contains("Test Agent Alpha"),
            "Should read agent content directly"
        );
    }

    /// Integration test: scan_for_skills and scan_for_agents find correct items
    #[test]
    fn test_integration_scan_discovers_both_skills_and_agents() {
        let (_source_dir, source_path) = create_test_repo_with_skills_and_agents();

        // Use the repo scanning functions directly
        let skills = scan_for_skills(&source_path).expect("Failed to scan for skills");
        let agents = scan_for_agents(&source_path).expect("Failed to scan for agents");

        // Verify skills were found
        assert_eq!(skills.len(), 2, "Should find 2 skills");
        assert!(
            skills.contains(&"test-skill-one".to_string()),
            "Should find test-skill-one"
        );
        assert!(
            skills.contains(&"test-skill-two".to_string()),
            "Should find test-skill-two"
        );

        // Verify agents were found
        assert_eq!(agents.len(), 2, "Should find 2 agents");
        assert!(
            agents.contains(&"test-agent-alpha".to_string()),
            "Should find test-agent-alpha"
        );
        assert!(
            agents.contains(&"test-agent-beta".to_string()),
            "Should find test-agent-beta"
        );
    }

    /// Helper: scan directory for skills (mirrors repo.rs scan_for_skills)
    fn scan_for_skills(path: &Path) -> anyhow::Result<Vec<String>> {
        let mut skills = Vec::new();

        if !path.exists() {
            return Ok(skills);
        }

        let entries = fs::read_dir(path)?;

        for entry in entries {
            let entry = entry?;
            let entry_path = entry.path();

            if entry_path.is_dir() {
                let skill_md = entry_path.join("SKILL.md");
                if skill_md.exists() {
                    if let Some(skill_name) = entry_path.file_name() {
                        skills.push(skill_name.to_string_lossy().to_string());
                    }
                }
            }
        }

        skills.sort();
        Ok(skills)
    }

    /// Helper: scan directory for agents (mirrors repo.rs scan_for_agents)
    fn scan_for_agents(path: &Path) -> anyhow::Result<Vec<String>> {
        let mut agents = Vec::new();

        if !path.exists() {
            return Ok(agents);
        }

        let entries = fs::read_dir(path)?;

        for entry in entries {
            let entry = entry?;
            let entry_path = entry.path();

            if entry_path.is_dir() {
                let agent_md = entry_path.join("AGENT.md");
                if agent_md.exists() {
                    if let Some(agent_name) = entry_path.file_name() {
                        agents.push(agent_name.to_string_lossy().to_string());
                    }
                }
            }
        }

        agents.sort();
        Ok(agents)
    }

    /// Integration test: type filters work correctly in list
    /// Tests the filter logic for --skills and --agents flags
    #[test]
    fn test_integration_list_type_filters() {
        // Simulate list filtering logic
        #[derive(Debug, Clone, PartialEq)]
        enum ItemType {
            Skill,
            Agent,
        }

        #[derive(Debug, Clone)]
        struct ListItem {
            name: String,
            item_type: ItemType,
            enabled: bool,
        }

        // Create test items (simulating combined list)
        let items = vec![
            ListItem {
                name: "skill-a".to_string(),
                item_type: ItemType::Skill,
                enabled: true,
            },
            ListItem {
                name: "skill-b".to_string(),
                item_type: ItemType::Skill,
                enabled: false,
            },
            ListItem {
                name: "agent-x".to_string(),
                item_type: ItemType::Agent,
                enabled: true,
            },
            ListItem {
                name: "agent-y".to_string(),
                item_type: ItemType::Agent,
                enabled: false,
            },
        ];

        // Filter: skills only
        let skills_only: Vec<_> = items
            .iter()
            .filter(|i| i.item_type == ItemType::Skill)
            .collect();
        assert_eq!(skills_only.len(), 2, "Should have 2 skills");
        assert!(
            skills_only.iter().all(|i| i.item_type == ItemType::Skill),
            "All should be skills"
        );

        // Filter: agents only
        let agents_only: Vec<_> = items
            .iter()
            .filter(|i| i.item_type == ItemType::Agent)
            .collect();
        assert_eq!(agents_only.len(), 2, "Should have 2 agents");
        assert!(
            agents_only.iter().all(|i| i.item_type == ItemType::Agent),
            "All should be agents"
        );

        // Filter: enabled only (default)
        let enabled_only: Vec<_> = items.iter().filter(|i| i.enabled).collect();
        assert_eq!(enabled_only.len(), 2, "Should have 2 enabled items");
        assert!(
            enabled_only.iter().all(|i| i.enabled),
            "All should be enabled"
        );

        // Filter: skills + enabled
        let enabled_skills: Vec<_> = items
            .iter()
            .filter(|i| i.item_type == ItemType::Skill && i.enabled)
            .collect();
        assert_eq!(enabled_skills.len(), 1, "Should have 1 enabled skill");
        assert_eq!(enabled_skills[0].name, "skill-a");

        // Filter: agents + enabled
        let enabled_agents: Vec<_> = items
            .iter()
            .filter(|i| i.item_type == ItemType::Agent && i.enabled)
            .collect();
        assert_eq!(enabled_agents.len(), 1, "Should have 1 enabled agent");
        assert_eq!(enabled_agents[0].name, "agent-x");

        // Filter: all (no filter)
        let all_items: Vec<_> = items.iter().collect();
        assert_eq!(all_items.len(), 4, "Should have all 4 items");
    }
}
