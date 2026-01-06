use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(name = "sm")]
#[command(version, about = "Agent Skill Manager - Manage your agent skills", long_about = None)]
pub struct Args {
    #[command(subcommand)]
    pub command: Command,
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

    /// Enable a skill (shortcut for 'skills enable')
    Enable {
        /// Skill name or reference (e.g., "git-commit" or "github.com/owner/repo/git-commit")
        skill_name_or_ref: String,
    },

    /// Disable a skill (shortcut for 'skills disable')
    Disable {
        /// Skill name or reference (e.g., "git-commit" or "github.com/owner/repo/git-commit")
        skill_name_or_ref: String,
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
}

#[derive(Debug, Subcommand)]
pub enum SkillAction {
    /// Enable a skill
    Enable {
        /// Skill name or reference (e.g., "git-commit" or "github.com/owner/repo/git-commit")
        skill_name_or_ref: String,
    },

    /// Disable a skill
    Disable {
        /// Skill name or reference (e.g., "git-commit" or "github.com/owner/repo/git-commit")
        skill_name_or_ref: String,
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
