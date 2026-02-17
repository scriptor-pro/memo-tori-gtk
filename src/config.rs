use std::fs;
use std::path::Path;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

pub fn default_capture_hints() -> Vec<String> {
    vec![
        "L'idee que je viens d'avoir :".to_string(),
        "Note rapide :".to_string(),
        "Je ne dois pas oublier :".to_string(),
        "Pense-bete du moment :".to_string(),
        "A creuser plus tard :".to_string(),
    ]
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AppConfig {
    pub quit_on_close: bool,
    pub text_scale: f32,
    #[serde(default = "default_capture_hints")]
    pub capture_hints: Vec<String>,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            quit_on_close: false,
            text_scale: 1.0,
            capture_hints: default_capture_hints(),
        }
    }
}

impl AppConfig {
    pub fn load_or_create(path: &Path) -> Result<Self> {
        if !path.exists() {
            let default = Self::default();
            let raw =
                toml::to_string_pretty(&default).context("failed to encode default config")?;
            fs::write(path, raw).context("failed to create config file")?;
            return Ok(default);
        }

        let raw = fs::read_to_string(path).context("failed to read config file")?;
        let config: Self = toml::from_str(&raw).context("failed to parse config file")?;
        Ok(config)
    }
}
