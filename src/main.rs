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
        },
        Command::Skills { action } => match action {
            SkillAction::Enable { skill_name_or_ref } => commands::skill::enable(&skill_name_or_ref),
            SkillAction::Disable { skill_name_or_ref } => commands::skill::disable(&skill_name_or_ref),
            SkillAction::List => commands::skill::list(),
        },
        Command::Cache { action } => match action {
            CacheAction::Dir => commands::cache::dir(),
        },
    };

    if let Err(e) = result {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
}
