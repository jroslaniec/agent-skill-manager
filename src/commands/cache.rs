use crate::paths;
use anyhow::Result;

pub fn dir() -> Result<()> {
    let cache_dir = paths::cache_dir()?;
    println!("{}", cache_dir.display());
    Ok(())
}
