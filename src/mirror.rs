use std::path::{Path, PathBuf};
use anyhow::{Context, Result};

pub struct MirrorManager {
    mirror_dir: PathBuf,
}

impl MirrorManager {
    pub fn new(config: &crate::config::Config) -> Result<Self> {
        std::fs::create_dir_all(&config.mirror_dir)?;
        Ok(Self {
            mirror_dir: config.mirror_dir.clone(),
        })
    }

    /// Get the mirror path for a repository
    pub fn get_mirror_path(&self, owner: &str, repo: &str) -> PathBuf {
        self.mirror_dir.join(owner).join(format!("{}.git", repo))
    }

    /// Ensure the mirror exists, cloning if necessary
    pub fn ensure_mirror(&self, owner: &str, repo: &str, force: bool) -> Result<PathBuf> {
        let mirror_path = self.get_mirror_path(owner, repo);

        if mirror_path.exists() {
            if force {
                std::fs::remove_dir_all(&mirror_path)?;
                return self.clone_mirror(owner, repo);
            }
            return Ok(mirror_path);
        }

        self.clone_mirror(owner, repo)
    }

    fn clone_mirror(&self, owner: &str, repo: &str) -> Result<PathBuf> {
        let mirror_path = self.get_mirror_path(owner, repo);
        let parent = mirror_path.parent().context("Could not get parent dir")?;
        std::fs::create_dir_all(parent)?;

        let url = format!("https://github.com/{}/{}.git", owner, repo);
        println!("📥 Cloning mirror: {} -> {}", url, mirror_path.display());

        // Use git command for cloning
        let status = std::process::Command::new("git")
            .arg("clone")
            .arg("--quiet")
            .arg("--mirror")
            .arg(&url)
            .arg(&mirror_path)
            .status()?;

        if !status.success() {
            anyhow::bail!("Failed to clone mirror from {}", url);
        }

        Ok(mirror_path)
    }

    /// List all mirrors
    pub fn list(&self) -> Result<Vec<PathBuf>> {
        let mut mirrors = Vec::new();
        if self.mirror_dir.exists() {
            for entry in std::fs::read_dir(&self.mirror_dir)? {
                let entry = entry?;
                let path = entry.path();
                if path.is_dir() {
                    if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                        if name.ends_with(".git") {
                            mirrors.push(path);
                        }
                    }
                }
            }
        }
        mirrors.sort();
        Ok(mirrors)
    }

    /// Check if a mirror exists
    pub fn mirror_exists(&self, owner: &str, repo: &str) -> bool {
        self.get_mirror_path(owner, repo).exists()
    }
}