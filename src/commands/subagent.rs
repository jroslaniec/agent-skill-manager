use anyhow::Result;

/// Enable one or more subagents
pub fn enable(_agent_names_or_refs: &[String]) -> Result<()> {
    eprintln!("Subagent enable not yet implemented");
    Ok(())
}

/// Disable one or more subagents
pub fn disable(_agent_names_or_refs: &[String]) -> Result<()> {
    eprintln!("Subagent disable not yet implemented");
    Ok(())
}

/// List subagents
pub fn list(_all: bool, _status: Option<&str>, _name_only: bool) -> Result<()> {
    eprintln!("Subagent list not yet implemented");
    Ok(())
}
