use std::path::{Path, PathBuf};
use clap::{Parser, Subcommand};
use anyhow::Result;

mod config;
mod github;
mod mirror;
mod worktree;
mod submodule;
mod master_list;
mod time_root;

use config::Config;
use github::GitHubClient;
use mirror::MirrorManager;
use worktree::WorktreeManager;
use submodule::SubmoduleManager;
use master_list::MasterList;
use time_root::TimeRoot;

#[derive(Parser)]
#[command(name = "time-tools")]
#[command(about = "Time-based repository management tools")]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// Enable verbose output
    #[arg(short, long, global = true)]
    verbose: bool,

    /// Dry run - show what would be done without making changes
    #[arg(long, global = true)]
    dry_run: bool,
}

#[derive(Subcommand)]
enum Commands {
    /// Initialize time-tools configuration
    Init {
        /// Time root directory (default: /mnt/data1)
        #[arg(long, default_value = "/mnt/data1")]
        time_root: PathBuf,

        /// Mirror directory (default: ~/git/github.com)
        #[arg(long)]
        mirror_dir: Option<PathBuf>,
    },

    /// Onboard a repository (create mirror, worktree, submodule)
    Onboard {
        /// Repository in owner/repo format
        repo: String,

        /// Date for the worktree (YYYY-MM-DD, default: today)
        #[arg(long)]
        date: Option<String>,

        /// Force create even if exists
        #[arg(short, long)]
        force: bool,
    },

    /// Onboard recently starred/forked repos since a date
    OnboardRecent {
        /// Date to fetch from (YYYY-MM-DD, default: 30 days ago)
        #[arg(long)]
        since: Option<String>,

        /// Only onboard starred repos
        #[arg(long)]
        stars_only: bool,

        /// Only onboard forked repos
        #[arg(long)]
        forks_only: bool,

        /// Additional owner to check for forks
        #[arg(long)]
        owner: Option<String>,
    },

    /// Create a new time root for a year/month
    CreateTimeRoot {
        /// Year (e.g., 2026)
        year: i32,

        /// Month (1-12)
        month: u32,
    },

    /// List all time roots
    ListTimeRoots,

    /// List all mirrors
    ListMirrors,

    /// List all worktrees
    ListWorktrees,

    /// Register an existing worktree as a submodule
    RegisterSubmodule {
        /// Path to worktree
        path: PathBuf,
    },

    /// Sync master list with time roots
    SyncMasterList,

    /// Show status of a repository
    Status {
        repo: String,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    let config = Config::load()?;
    let github = GitHubClient::new()?;
    let mirror_mgr = MirrorManager::new(&config)?;
    let worktree_mgr = WorktreeManager::new(&config)?;
    let submodule_mgr = SubmoduleManager::new(&config)?;
    let master_list = MasterList::new(&config)?;

    match cli.command {
        Commands::Init { time_root, mirror_dir } => {
            Config::init(time_root, mirror_dir)?;
            println!("Configuration initialized");
        }
        Commands::Onboard { repo, date, force } => {
            let date = date.unwrap_or_else(|| {
                chrono::Local::now().format("%Y-%m-%d").to_string()
            });
            let (owner, name) = parse_repo(&repo)?;
            let time_root = TimeRoot::for_date(&config, &date)?;

            let mirror_path = mirror_mgr.ensure_mirror(&owner, &name, force)?;
            let worktree_path = worktree_mgr.ensure_worktree(&time_root.path, &mirror_path, &name, force)?;

            if !cli.dry_run {
                submodule_mgr.register_submodule(&worktree_path)?;
                master_list.add_entry(&owner, &name, &worktree_path, &time_root.path)?;
            }
            println!("✅ Onboarded {}/{} to {}", owner, name, worktree_path.display());
        }
        Commands::OnboardRecent { since, stars_only, forks_only, owner } => {
            let since = since.unwrap_or_else(|| {
                (chrono::Local::now() - chrono::Duration::days(30))
                    .format("%Y-%m-%d").to_string()
            });

            let mut owners = vec![github.get_authenticated_user().await?];
            if let Some(o) = owner {
                owners.push(o);
            }

            let mut count_onboarded = 0u32;
            let mut count_reused = 0u32;
            let mut count_failed = 0u32;

            if !forks_only {
                let stars = github.get_starred_repos(&since).await?;
                for (date, repo) in stars {
                    match onboard_one(
                        &config, &mirror_mgr, &worktree_mgr, &submodule_mgr, &master_list,
                        &date, &repo, "-",
                    ) {
                        Ok(true) => count_onboarded += 1,
                        Ok(false) => count_reused += 1,
                        Err(_) => count_failed += 1,
                    }
                }
            }

            if !stars_only {
                for owner in owners {
                    let forks = github.get_forks(&owner, &since).await?;
                    for (date, parent_repo, fork_owner) in forks {
                        match onboard_one(
                            &config, &mirror_mgr, &worktree_mgr, &submodule_mgr, &master_list,
                            &date, &parent_repo, &fork_owner,
                        ) {
                            Ok(true) => count_onboarded += 1,
                            Ok(false) => count_reused += 1,
                            Err(_) => count_failed += 1,
                        }
                    }
                }
            }

            println!("═══════════════════════════════════════");
            println!("✅ onboarded: {}   ♻️  reused: {}   ⚠️  failed: {}", count_onboarded, count_reused, count_failed);
            println!("═══════════════════════════════════════");
        }
        Commands::CreateTimeRoot { year, month } => {
            let time_root = TimeRoot::create_at(&config, year, month)?;
            println!("Created time root: {}", time_root.path.display());
        }
        Commands::ListTimeRoots => {
            for root in TimeRoot::list(&config)? {
                println!("{}", root.display());
            }
        }
        Commands::ListMirrors => {
            for mirror in mirror_mgr.list()? {
                println!("{}", mirror.display());
            }
        }
        Commands::ListWorktrees => {
            for worktree in worktree_mgr.list()? {
                println!("{}", worktree.display());
            }
        }
        Commands::RegisterSubmodule { path } => {
            submodule_mgr.register_submodule(&path)?;
            println!("Registered submodule for {}", path.display());
        }
        Commands::SyncMasterList => {
            master_list.sync(&config)?;
            println!("Master list synced");
        }
        Commands::Status { repo } => {
            let (owner, name) = parse_repo(&repo)?;
            let mirror_path = mirror_mgr.get_mirror_path(&owner, &name);
            println!("Mirror: {}", mirror_path.display());
            println!("Exists: {}", mirror_path.exists());
        }
    }

    Ok(())
}

fn parse_repo(repo: &str) -> Result<(String, String)> {
    let parts: Vec<&str> = repo.split('/').collect();
    if parts.len() != 2 {
        anyhow::bail!("Repository must be in owner/repo format");
    }
    Ok((parts[0].to_string(), parts[1].to_string()))
}

fn onboard_one(
    config: &Config,
    mirror_mgr: &MirrorManager,
    worktree_mgr: &WorktreeManager,
    submodule_mgr: &SubmoduleManager,
    master_list: &MasterList,
    date: &str,
    repo: &str,
    _fork_owner: &str,
) -> Result<bool> {
    let (owner, name) = parse_repo(repo)?;
    let time_root = TimeRoot::for_date(config, date)?;

    let mirror_path = mirror_mgr.ensure_mirror(&owner, &name, false)?;
    let worktree_path = worktree_mgr.ensure_worktree(&time_root.path, &mirror_path, &name, false)?;

    let is_new = !worktree_path.exists();

    submodule_mgr.register_submodule(&worktree_path)?;
    master_list.add_entry(&owner, &name, &worktree_path, &time_root.path)?;

    Ok(is_new)
}