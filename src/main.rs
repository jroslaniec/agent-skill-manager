use agent_skill_manager::cli::{Args, CacheAction, Command, RepoAction, SkillAction};
use agent_skill_manager::commands;
use clap::Parser;

fn main() {
    let args = Args::parse();

    let result = match args.command {
        Command::Repositories { action } => match action {
            RepoAction::Add { url } => commands::repo::add(&url),
            RepoAction::Delete { url, force } => commands::repo::delete(&url, force),
            RepoAction::List => commands::repo::list(),
            RepoAction::Pin { url } => commands::repo::pin(&url),
            RepoAction::Unpin { url } => commands::repo::unpin(&url),
            RepoAction::Upgrade { url } => commands::repo::upgrade(&url),
        },
        Command::Skills { action } => match action {
            SkillAction::Enable { skill_name_or_ref } => commands::skill::enable(&skill_name_or_ref),
            SkillAction::Disable { skill_name_or_ref } => commands::skill::disable(&skill_name_or_ref),
            SkillAction::List { all, status, name_only } => {
                commands::skill::list(all, status.as_deref(), name_only)
            }
        },
        Command::Cache { action } => match action {
            CacheAction::Dir => commands::cache::dir(),
        },
        // Shortcuts for skill commands
        Command::Enable { skill_name_or_ref } => commands::skill::enable(&skill_name_or_ref),
        Command::Disable { skill_name_or_ref } => commands::skill::disable(&skill_name_or_ref),
        Command::List { all, status, name_only } => {
            commands::skill::list(all, status.as_deref(), name_only)
        }
        Command::Purge { force } => commands::purge::purge(force),
        Command::Upgrade { force } => commands::repo::upgrade_all(force),
    };

    if let Err(e) = result {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
}
