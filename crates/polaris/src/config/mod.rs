use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct Config {
    #[serde(default)]
    pub notion: NotionConfig,

    /// "light" | "dark" — written by the in-app Cmd+T toggle. Absent =
    /// follow the OS (delete the key to go back to following the OS).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub theme: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct NotionConfig {
    pub token: Option<String>,
    pub default_page: Option<String>,
}

impl Config {
    pub fn load() -> Result<Self> {
        let config_path = Self::config_path()?;

        if !config_path.exists() {
            return Ok(Self::default());
        }

        let contents = fs::read_to_string(&config_path)
            .with_context(|| format!("Failed to read config file: {:?}", config_path))?;

        let config: Config =
            toml::from_str(&contents).with_context(|| "Failed to parse config file")?;

        Ok(config)
    }

    pub fn save(&self) -> Result<()> {
        let config_path = Self::config_path()?;

        if let Some(parent) = config_path.parent() {
            fs::create_dir_all(parent).with_context(|| "Failed to create config directory")?;
        }

        let contents =
            toml::to_string_pretty(self).with_context(|| "Failed to serialize config")?;

        fs::write(&config_path, contents)
            .with_context(|| format!("Failed to write config file: {:?}", config_path))?;

        Ok(())
    }

    fn config_path() -> Result<PathBuf> {
        let home = dirs::home_dir().with_context(|| "Failed to get home directory")?;
        Ok(home.join(".polaris.toml"))
    }
}
