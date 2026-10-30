use std::path::{Path, PathBuf};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub time_root: PathBuf,
    pub mirror_dir: PathBuf,
}

impl Config {
    pub fn load() -> Result<Self> {
        let config_path = Self::config_path()?;
        if config_path.exists() {
            let content = std::fs::read_to_string(&config_path)?;
            let config: Config = toml::from_str(&content)?;
            Ok(config)
        } else {
            Ok(Self::default())
        }
    }

    fn config_path() -> Result<PathBuf> {
        let config_dir = dirs::config_dir()
            .context("Could not find config directory")?
            .join("time-tools");
        Ok(config_dir.join("config.toml"))
    }

    pub fn save(&self) -> Result<()> {
        let config_path = Self::config_path()?;
        std::fs::create_dir_all(config_path.parent().unwrap())?;
        let content = toml::to_string_pretty(self)?;
        std::fs::write(&config_path, content)?;
        Ok(())
    }

    pub fn init(time_root: PathBuf, mirror_dir: Option<PathBuf>) -> Result<()> {
        let config = Self {
            time_root,
            mirror_dir: mirror_dir.unwrap_or_else(|| dirs::home_dir().unwrap().join("git/github.com")),
        };
        config.save()
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            time_root: PathBuf::from("/mnt/data1"),
            mirror_dir: dirs::home_dir().unwrap().join("git/github.com"),
        }
    }
}