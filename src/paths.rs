use std::fs;
use std::path::PathBuf;

use anyhow::{Context, Result};

#[derive(Debug, Clone)]
pub struct AppPaths {
    pub db_path: PathBuf,
    pub config_path: PathBuf,
}

impl AppPaths {
    pub fn resolve() -> Result<Self> {
        let data_home = dirs::data_dir().context("could not resolve XDG data directory")?;
        let config_home = dirs::config_dir().context("could not resolve XDG config directory")?;

        let data_dir = data_home.join("memo-tori");
        let config_dir = config_home.join("memo-tori");

        fs::create_dir_all(&data_dir).context("failed to create data directory")?;
        fs::create_dir_all(&config_dir).context("failed to create config directory")?;

        Ok(Self {
            db_path: data_dir.join("memo-tori.db"),
            config_path: config_dir.join("config.toml"),
        })
    }
}
