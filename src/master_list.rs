use std::path::{Path, PathBuf};
use anyhow::{Context, Result};
use std::io::Write;

pub struct MasterListEntry {
    pub entry_type: String,
    pub owner: String,
    pub repo: String,
    pub path: String,
    pub url: String,
    pub date: String,
}

pub struct MasterList {
    path: PathBuf,
}

impl MasterList {
    pub fn new(config: &crate::config::Config) -> Result<Self> {
        let path = config.time_root.join("master-list.tsv");
        std::fs::create_dir_all(path.parent().unwrap())?;
        Ok(Self { path })
    }

    pub fn add_entry(
        &self,
        owner: &str,
        repo: &str,
        worktree_path: &Path,
        _time_root: &Path,
    ) -> Result<()> {
        let entry = MasterListEntry {
            entry_type: "worktree".to_string(),
            owner: owner.to_string(),
            repo: repo.to_string(),
            path: worktree_path.to_string_lossy().to_string(),
            url: format!("https://github.com/{}/{}", owner, repo),
            date: chrono::Local::now().format("%Y-%m-%dT%H:%M:%S%:z").to_string(),
        };

        self.append_entry(&entry)
    }

    fn append_entry(&self, entry: &MasterListEntry) -> Result<()> {
        // Check if entry already exists
        if self.entry_exists(entry)? {
            return Ok(());
        }

        let line = format!(
            "{}\t{}\t{}\t{}\t{}\n",
            entry.entry_type,
            entry.owner,
            entry.repo,
            entry.path,
            entry.url
        );

        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)?;

        file.write_all(line.as_bytes())?;

        Ok(())
    }

    fn entry_exists(&self, entry: &MasterListEntry) -> Result<bool> {
        if !self.path.exists() {
            return Ok(false);
        }

        let content = std::fs::read_to_string(&self.path)?;
        let search_key = format!(
            "\t{}\t{}\t{}\t",
            entry.owner, entry.repo, entry.path
        );

        Ok(content.contains(&search_key))
    }

    pub fn sync(&self, _config: &crate::config::Config) -> Result<()> {
        Ok(())
    }

    pub fn list(&self) -> Result<Vec<MasterListEntry>> {
        if !self.path.exists() {
            return Ok(Vec::new());
        }

        let content = std::fs::read_to_string(&self.path)?;
        let mut entries = Vec::new();

        for line in content.lines() {
            let parts: Vec<&str> = line.split('\t').collect();
            if parts.len() >= 5 {
                entries.push(MasterListEntry {
                    entry_type: parts[0].to_string(),
                    owner: parts[1].to_string(),
                    repo: parts[2].to_string(),
                    path: parts[3].to_string(),
                    url: parts[4].to_string(),
                    date: chrono::Local::now().format("%Y-%m-%dT%H:%M:%S%:z").to_string(),
                });
            }
        }

        Ok(entries)
    }
}