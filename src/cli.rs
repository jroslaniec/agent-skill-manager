use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(name = "sm")]
#[command(version, about = "Agent Skill Manager - Manage your agent skills", long_about = None)]
#[command(subcommand_required = false)]
pub struct Args {
    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Manage skill repositories
    #[command(visible_aliases = ["repository", "repos", "repo"])]
    Repositories {
        #[command(subcommand)]
        action: RepoAction,
    },

    /// Manage skills (default command group)
    #[command(visible_aliases = ["skill", "sk"])]
    Skills {
        #[command(subcommand)]
        action: SkillAction,
    },

    /// Cache utilities
    Cache {
        #[command(subcommand)]
        action: CacheAction,
    },

    /// Add one or more skills (clones repo if needed and enables skills)
    Add {
        /// Interactive mode - select skills from a repository
        #[arg(short, long)]
        interactive: bool,

        /// Skill references or repository URL (with -i flag)
        skill_refs: Vec<String>,
    },

    /// Enable one or more skills (shortcut for 'skills enable')
    Enable {
        /// Skill names or references (e.g., "git-commit" or "github.com/owner/repo/git-commit")
        skill_names_or_refs: Vec<String>,
    },

    /// Disable one or more skills (shortcut for 'skills disable')
    Disable {
        /// Skill names or references (e.g., "git-commit" or "github.com/owner/repo/git-commit")
        skill_names_or_refs: Vec<String>,
    },

    /// List skills (shortcut for 'skills list')
    #[command(visible_aliases = ["ls"])]
    List {
        /// Show all skills (enabled and disabled)
        #[arg(short, long)]
        all: bool,

        /// Filter by status (enabled or disabled)
        #[arg(long)]
        status: Option<String>,

        /// Show only skill names (one per line)
        #[arg(long)]
        name_only: bool,
    },

    /// Purge all repositories and skills (full reset)
    Purge {
        /// Skip confirmation prompt
        #[arg(short, long)]
        force: bool,
    },

    /// Upgrade all unpinned repositories
    Upgrade {
        /// Skip confirmation prompt
        #[arg(short, long)]
        force: bool,
    },
}

#[derive(Debug, Subcommand)]
pub enum RepoAction {
    /// Add a repository
    Add {
        /// Repository URL (e.g., github.com/owner/repo or github.com/owner/repo/path)
        url: String,
    },

    /// Delete a repository
    #[command(visible_aliases = ["remove", "rm", "del"])]
    Delete {
        /// Repository URL to delete
        url: String,

        /// Force deletion without confirmation
        #[arg(short, long)]
        force: bool,
    },

    /// List all registered repositories
    #[command(visible_aliases = ["ls"])]
    List,

    /// Pin a repository to its current commit
    Pin {
        /// Repository URL to pin
        url: String,
    },

    /// Unpin a repository to allow upgrades
    Unpin {
        /// Repository URL to unpin
        url: String,
    },

    /// Upgrade a repository to latest or specific commit
    Upgrade {
        /// Repository URL (with optional @SHA)
        url: String,
    },
}

#[derive(Debug, Subcommand)]
pub enum SkillAction {
    /// Enable one or more skills
    Enable {
        /// Skill names or references (e.g., "git-commit" or "github.com/owner/repo/git-commit")
        skill_names_or_refs: Vec<String>,
    },

    /// Disable one or more skills
    Disable {
        /// Skill names or references (e.g., "git-commit" or "github.com/owner/repo/git-commit")
        skill_names_or_refs: Vec<String>,
    },

    /// List all skills
    #[command(visible_aliases = ["ls"])]
    List {
        /// Show all skills (enabled and disabled)
        #[arg(short, long)]
        all: bool,

        /// Filter by status (enabled or disabled)
        #[arg(long)]
        status: Option<String>,

        /// Show only skill names (one per line)
        #[arg(long)]
        name_only: bool,
    },
}

#[derive(Debug, Subcommand)]
pub enum CacheAction {
    /// Show cache directory path
    Dir,
}
