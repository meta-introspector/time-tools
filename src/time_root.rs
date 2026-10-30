use std::path::{Path, PathBuf};
use anyhow::{Context, Result};
use chrono::{Datelike, Local};

pub struct TimeRoot {
    pub path: PathBuf,
    pub year: i32,
    pub month: u32,
}

impl TimeRoot {
    /// Create time root in the new format: time-YYYY-MM
    pub fn new(base: &Path, year: i32, month: u32) -> Self {
        let name = format!("time-{:04}-{:02}", year, month);
        Self {
            path: base.join(name),
            year,
            month,
        }
    }

    /// Get time root for a specific date (YYYY-MM-DD)
    pub fn for_date(config: &crate::config::Config, date: &str) -> Result<Self> {
        let parts: Vec<&str> = date.split('-').collect();
        if parts.len() != 3 {
            anyhow::bail!("Invalid date format: {}", date);
        }
        let year = parts[0].parse::<i32>()?;
        let month = parts[1].parse::<u32>()?;
        let day = parts[2].parse::<u32>()?;

        if !(1..=12).contains(&month) {
            anyhow::bail!("Invalid month: {}", month);
        }
        if !(1..=31).contains(&day) {
            anyhow::bail!("Invalid day: {}", day);
        }

        Ok(Self::new(&config.time_root, year, month))
    }

    /// Create the time root directory
    pub fn create(&self) -> Result<()> {
        std::fs::create_dir_all(&self.path)
            .with_context(|| format!("Failed to create time root: {}", self.path.display()))
    }

    /// Create a time root for a year/month
    pub fn create_at(config: &crate::config::Config, year: i32, month: u32) -> Result<Self> {
        let time_root = Self::new(&config.time_root, year, month);
        time_root.create()?;
        Ok(time_root)
    }

    /// List all time roots under the base directory
    pub fn list(config: &crate::config::Config) -> Result<Vec<PathBuf>> {
        let mut roots = Vec::new();
        if config.time_root.exists() {
            for entry in std::fs::read_dir(&config.time_root)? {
                let entry = entry?;
                let path = entry.path();
                if path.is_dir() {
                    if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                        if name.starts_with("time-") {
                            roots.push(path);
                        }
                    }
                }
            }
        }
        roots.sort();
        Ok(roots)
    }

    /// Get the worktree path for a repository in this time root
    pub fn worktree_path(&self, repo: &str) -> PathBuf {
        self.path.join(repo)
    }
}