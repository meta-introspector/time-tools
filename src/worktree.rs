use std::path::{Path, PathBuf};
use anyhow::Result;

pub struct WorktreeManager {
    mirror_dir: PathBuf,
}

impl WorktreeManager {
    pub fn new(config: &crate::config::Config) -> Result<Self> {
        Ok(Self {
            mirror_dir: config.mirror_dir.clone(),
        })
    }

    /// Get the worktree path for a repository in a time root
    pub fn get_worktree_path(&self, time_root: &Path, repo: &str) -> PathBuf {
        time_root.join(repo)
    }

    /// Ensure a worktree exists for a repository
    pub fn ensure_worktree(
        &self,
        time_root: &Path,
        mirror_path: &Path,
        repo: &str,
        force: bool,
    ) -> Result<PathBuf> {
        let worktree_path = self.get_worktree_path(time_root, repo);

        if worktree_path.exists() {
            if force {
                std::fs::remove_dir_all(&worktree_path)?;
                return self.create_worktree(time_root, mirror_path, repo);
            }
            return Ok(worktree_path);
        }

        self.create_worktree(time_root, mirror_path, repo)
    }

    fn create_worktree(
        &self,
        time_root: &Path,
        mirror_path: &Path,
        repo: &str,
    ) -> Result<PathBuf> {
        let worktree_path = self.get_worktree_path(time_root, repo);
        std::fs::create_dir_all(time_root)?;

        println!("🌳 Creating worktree: {} -> {}", mirror_path.display(), worktree_path.display());

        // Use git command for worktree creation
        let status = std::process::Command::new("git")
            .arg("--git-dir")
            .arg(mirror_path)
            .arg("worktree")
            .arg("add")
            .arg("--quiet")
            .arg(&worktree_path)
            .arg("HEAD")
            .status()?;

        if !status.success() {
            // Fallback: clone from mirror
            let status = std::process::Command::new("git")
                .arg("clone")
                .arg("--quiet")
                .arg(mirror_path)
                .arg(&worktree_path)
                .status()?;

            if !status.success() {
                anyhow::bail!("Failed to create worktree at {}", worktree_path.display());
            }
        }

        Ok(worktree_path)
    }

    /// Check if a worktree is newly created (not just reused)
    pub fn is_new_worktree(&self, path: &Path) -> Result<bool> {
        Ok(!path.exists())
    }

    /// List all worktrees
    pub fn list(&self) -> Result<Vec<PathBuf>> {
        Ok(Vec::new())
    }
}