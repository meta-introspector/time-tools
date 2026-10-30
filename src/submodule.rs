use std::path::{Path, PathBuf};
use anyhow::{Context, Result};
use std::process::Command;

pub struct SubmoduleManager;

impl SubmoduleManager {
    pub fn new(_config: &crate::config::Config) -> Result<Self> {
        Ok(Self)
    }

    /// Register a worktree as a submodule in its time root's .gitmodules
    pub fn register_submodule(&self, worktree_path: &Path) -> Result<()> {
        let time_root = find_time_root(worktree_path)?;

        if !time_root.exists() {
            return Ok(());
        }

        let rel_path = worktree_path
            .strip_prefix(&time_root)
            .with_context(|| format!("Path not under time root: {}", worktree_path.display()))?
            .to_str()
            .context("Invalid UTF-8 in path")?;

        let gitmodules_path = time_root.join(".gitmodules");

        // Check if already registered
        if is_submodule_registered(&gitmodules_path, rel_path)? {
            println!("🧩 submodule: already tracked");
            return Ok(());
        }

        // Get URL from checkout
        let url = get_checkout_url(worktree_path)?;

        if url.is_empty() {
            anyhow::bail!("No URL found for submodule");
        }

        println!("🧩 submodule: + {}", rel_path);

        // Add submodule using git command
        let status = Command::new("git")
            .arg("-C")
            .arg(&time_root)
            .arg("submodule")
            .arg("add")
            .arg("--quiet")
            .arg("--name")
            .arg(rel_path)
            .arg(&url)
            .arg(rel_path)
            .status()?;

        if !status.success() {
            anyhow::bail!("git submodule add failed for {}", rel_path);
        }

        // Add git attributes
        add_submodule_attributes(&gitmodules_path, rel_path, &url)?;

        Ok(())
    }
}

/// Find the time root for a worktree path
fn find_time_root(worktree_path: &Path) -> Result<PathBuf> {
    let path = worktree_path.to_str().context("Invalid UTF-8 in path")?;
    let parts: Vec<&str> = path.split('/').collect();

    for i in 0..parts.len() {
        if parts[i].starts_with("time-") && parts[i].len() > 5 {
            let time_root = PathBuf::from(parts[..=i].join("/"));
            if time_root.exists() && time_root.is_dir() {
                return Ok(time_root);
            }
        }
    }

    anyhow::bail!("No time root found for path: {}", worktree_path.display())
}

/// Check if a submodule is already registered
fn is_submodule_registered(gitmodules_path: &Path, rel_path: &str) -> Result<bool> {
    if !gitmodules_path.exists() {
        return Ok(false);
    }

    let content = std::fs::read_to_string(gitmodules_path)?;

    for line in content.lines() {
        if line.trim_start().starts_with("path =") {
            let path_value = line.splitn(2, '=').nth(1).map(|v| v.trim());
            if path_value == Some(rel_path) {
                return Ok(true);
            }
        }
    }

    Ok(false)
}

/// Get the checkout URL from remotes
fn get_checkout_url(worktree_path: &Path) -> Result<String> {
    // Try git remote get-url upstream first, then origin, then local
    for remote in &["upstream", "origin", "local"] {
        let output = Command::new("git")
            .arg("-C")
            .arg(worktree_path)
            .arg("remote")
            .arg("get-url")
            .arg(remote)
            .output()?;

        if output.status.success() {
            let url = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !url.is_empty() {
                return Ok(url);
            }
        }
    }

    Ok(String::new())
}

/// Add git attributes to a submodule entry in .gitmodules
fn add_submodule_attributes(gitmodules_path: &Path, rel_path: &str, url: &str) -> Result<()> {
    use chrono::Local;

    let branch = get_default_branch(gitmodules_path).unwrap_or_else(|| "main".to_string());
    let now = Local::now().to_rfc3339();

    // Read existing content
    let content = std::fs::read_to_string(gitmodules_path)?;

    // Find the last submodule section and add attributes
    // The attributes need to be added under the correct [submodule] section
    let attrs = format!(
        "\tbranch = {}\n\tactive = true\n\tfetched-from = {}\n\tonboarded-at = {}\n",
        branch, url, now
    );

    // Insert attributes after the url line of the last submodule section
    let updated = insert_submodule_attrs(&content, rel_path, &attrs);

    std::fs::write(gitmodules_path, updated)?;

    Ok(())
}

fn get_default_branch(_path: &Path) -> Option<String> {
    Some("main".to_string())
}

fn insert_submodule_attrs(content: &str, rel_path: &str, attrs: &str) -> String {
    let lines: Vec<&str> = content.lines().collect();
    let mut result: Vec<String> = Vec::new();
    let mut in_target_section = false;
    let mut added = false;

    for (i, line) in lines.iter().enumerate() {
        result.push(line.to_string());

        if line.starts_with("[submodule \"") {
            let section_name = line.split('"').nth(1).unwrap_or("");
            in_target_section = section_name == rel_path;
        }

        if in_target_section && !added {
            if line.trim_start().starts_with("url =") {
                result.push(attrs.trim_end().to_string());
                added = true;
                in_target_section = false;
            }
        }
    }

    if !added {
        result.push(format!("[submodule \"{}\"]", rel_path));
        result.push(format!("\tpath = {}", rel_path));
        result.push(format!("\turl = {}", if attrs.contains("url =") { "" } else { "" }));
        result.push(attrs.trim_end().to_string());
    }

    result.join("\n")
}

/// Sync submodules from disk to .gitmodules
pub fn sync_submodules(time_root: &Path) -> Result<Vec<String>> {
    let mut synced = Vec::new();

    if !time_root.exists() {
        return Ok(synced);
    }

    // Walk the time root looking for .git directories
    for entry in walkdir::WalkDir::new(time_root)
        .max_depth(2)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let path = entry.path();
        if path.file_name() == Some(std::ffi::OsStr::new(".git")) {
            let worktree = path.parent().context("Invalid path")?;
            synced.push(worktree.to_string_lossy().to_string());
        }
    }

    Ok(synced)
}
