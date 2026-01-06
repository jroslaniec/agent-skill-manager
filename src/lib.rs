pub mod cli;
pub mod commands;
pub mod config;
pub mod git;
pub mod paths;
pub mod skill_ref;

pub type Result<T> = anyhow::Result<T>;
