use anyhow::{Context, Result};
use fs2::FileExt;
use std::fs::{File, OpenOptions};
use std::io::{Read, Write};

use super::state::Config;
use crate::paths;

pub struct ConfigLock {
    _lock_file: File,
    config_path: std::path::PathBuf,
}

impl ConfigLock {
    /// Acquire exclusive lock and read config
    pub fn acquire() -> Result<Self> {
        // Ensure directories exist
        paths::ensure_dirs()?;

        let lock_path = paths::lock_path();
        let config_path = paths::config_path()?;

        // Open/create lock file
        let lock_file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(true)
            .open(&lock_path)
            .context("Failed to open lock file")?;

        // Acquire exclusive lock (blocks until available)
        lock_file
            .lock_exclusive()
            .context("Failed to acquire lock")?;

        Ok(Self {
            _lock_file: lock_file,
            config_path,
        })
    }

    /// Read the current config
    pub fn read_config(&self) -> Result<Config> {
        if !self.config_path.exists() {
            return Ok(Config::new());
        }

        let mut file = File::open(&self.config_path).context("Failed to open config file")?;
        let mut contents = String::new();
        file.read_to_string(&mut contents)
            .context("Failed to read config file")?;

        Config::from_toml(&contents)
    }

    /// Write config atomically
    pub fn write_config(&self, config: &Config) -> Result<()> {
        let toml_content = config.to_toml()?;

        // Write to temporary file first
        let temp_file = tempfile::NamedTempFile::new_in(
            self.config_path
                .parent()
                .expect("Config path should have parent"),
        )
        .context("Failed to create temp file")?;

        temp_file
            .as_file()
            .write_all(toml_content.as_bytes())
            .context("Failed to write temp file")?;

        // Atomically rename to actual config file
        temp_file
            .persist(&self.config_path)
            .context("Failed to persist config file")?;

        Ok(())
    }

    /// Update config using a closure
    pub fn update<F>(&self, f: F) -> Result<()>
    where
        F: FnOnce(&mut Config) -> Result<()>,
    {
        let mut config = self.read_config()?;
        f(&mut config)?;
        self.write_config(&config)?;
        Ok(())
    }
}

impl Drop for ConfigLock {
    fn drop(&mut self) {
        // Lock is automatically released when file is closed
        // Clean up lock file
        let _ = std::fs::remove_file(paths::lock_path());
    }
}
