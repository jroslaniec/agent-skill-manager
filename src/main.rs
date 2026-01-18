use agent_skill_manager::cli::{
    Args, CacheAction, Command, IntegrationAction, RepoAction, SkillAction, SubagentAction,
};
use agent_skill_manager::commands;
use clap::Parser;

fn main() {
    let args = Args::parse();

    let result = match args.command {
        None => commands::skill::manage(),
        Some(Command::Repositories { action }) => match action {
            RepoAction::Add { urls } => commands::repo::add(&urls),
            RepoAction::Delete { urls, force } => commands::repo::delete(&urls, force),
            RepoAction::List => commands::repo::list(),
            RepoAction::Pin { url } => commands::repo::pin(&url),
            RepoAction::Unpin { url } => commands::repo::unpin(&url),
            RepoAction::Upgrade { url } => commands::repo::upgrade(&url),
        },
        Some(Command::Skills { action }) => match action {
            SkillAction::Enable {
                skill_names_or_refs,
            } => commands::skill::enable(&skill_names_or_refs),
            SkillAction::Disable {
                skill_names_or_refs,
            } => commands::skill::disable(&skill_names_or_refs),
            SkillAction::List {
                all,
                status,
                name_only,
            } => commands::skill::list(all, status.as_deref(), name_only),
        },
        Some(Command::Subagents { action }) => match action {
            SubagentAction::Enable {
                agent_names_or_refs,
            } => commands::subagent::enable(&agent_names_or_refs),
            SubagentAction::Disable {
                agent_names_or_refs,
            } => commands::subagent::disable(&agent_names_or_refs),
            SubagentAction::List {
                all,
                status,
                name_only,
            } => commands::subagent::list(all, status.as_deref(), name_only),
        },
        Some(Command::Cache { action }) => match action {
            CacheAction::Dir => commands::cache::dir(),
        },
        Some(Command::Add {
            interactive,
            skill_refs,
        }) => commands::skill::add(&skill_refs, interactive),
        Some(Command::List {
            all,
            status,
            name_only,
            skills,
            agents,
        }) => commands::skill::list_combined(all, status.as_deref(), name_only, skills, agents),
        Some(Command::Purge { force }) => commands::purge::purge(force),
        Some(Command::Upgrade { force }) => commands::repo::upgrade_all(force),
        Some(Command::Integrations { action }) => match action {
            IntegrationAction::Add {
                name,
                path,
                agents_path,
            } => commands::integration::add(&name, path.as_deref(), agents_path.as_deref()),
            IntegrationAction::Remove { name } => commands::integration::remove(&name),
            IntegrationAction::List => commands::integration::list(),
        },
        Some(Command::Configure) => commands::integration::configure(),
    };

    if let Err(e) = result {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
}
