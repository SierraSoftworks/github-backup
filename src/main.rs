use clap::Parser;
use errors::Error;
use policy::BackupPolicy;
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::time::Duration;
use tokio::task::JoinSet;
use tracing::Instrument;

#[macro_use]
mod macros;

mod config;
mod errors;
mod policy;
mod sources;
mod targets;
mod telemetry;

static CANCEL: AtomicBool = AtomicBool::new(false);

/// Backup your GitHub repositories automatically.
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
pub struct Args {
    /// Path to the configuration file.
    #[arg(short, long, default_value = "config.yaml")]
    pub config: String,

    /// Run in dry-run mode.
    #[arg(short, long)]
    pub dry_run: bool,
}

#[async_trait::async_trait]
pub trait RepositorySource<T: BackupEntity> {
    async fn get_repos(&self, policy: &BackupPolicy, cancel: &AtomicBool) -> Result<Vec<T>, Error>;
}

pub trait BackupTarget<T: BackupEntity> {
    fn backup(&self, repo: &T, cancel: &AtomicBool) -> Result<String, Error>;
}

pub trait BackupEntity {
    fn backup_path(&self) -> PathBuf;
    fn full_name(&self) -> &str;
    fn clone_url(&self) -> &str;

    fn matches(&self, filter: &policy::RepoFilter) -> bool {
        match filter {
            policy::RepoFilter::Include(names) => names
                .iter()
                .any(|n| self.full_name().eq_ignore_ascii_case(n)),
            policy::RepoFilter::Exclude(names) => !names
                .iter()
                .any(|n| self.full_name().eq_ignore_ascii_case(n)),
            _ => true,
        }
    }
}

async fn run(args: Args) -> Result<(), Error> {
    let config = config::Config::try_from(&args)?;

    let github = sources::GitHubSource::from(&config);
    let git_backup = targets::FileSystemBackupTarget::from(&config);

    while !CANCEL.load(std::sync::atomic::Ordering::Relaxed) {
        let next_run = config.schedule.as_ref()
            .and_then(|s| s.find_next_occurrence(&chrono::Utc::now(), false).ok());

        {
            let _span = tracing::info_span!("backup.all").entered();

            for policy in config.backups.iter() {
                let policy_span = tracing::info_span!("backup.policy", policy = %policy).entered();

                match github.get_repos(policy, &CANCEL).instrument(tracing::info_span!(parent: &policy_span, "backup.get_repos")).await {
                    Ok(repos) => {
                        println!("Backing up repositories for: {}", &policy);
                        let mut join_set: JoinSet<Result<(_, String), (_, errors::Error)>> = JoinSet::new();
                        for repo in repos {
                            if policy.filters.iter().all(|p| repo.matches(p)) {
                                if args.dry_run {
                                    println!(" - {} (dry-run)", repo.full_name());
                                    continue;
                                }

                                let git_backup = git_backup.clone();

                                let span = tracing::info_span!(parent: &policy_span, "backup.repo", repo = %repo.full_name());
                                join_set.spawn(async move {
                                    match git_backup.backup(&repo, &CANCEL) {
                                        Ok(id) => Ok((repo, id)),
                                        Err(e) => Err((repo, e)),
                                    }
                                }.instrument(span));
                            }
                        }

                        while let Some(fut) = join_set.join_next().await {
                            match fut.map_err(|e| errors::system_with_internal(
                                "Failed to complete a background backup task due to an internal runtime error.",
                                "Please report this issue to us on GitHub with details of what you were doing when it occurred.",
                                e))? {
                                Ok((repo, id)) => println!(" - {} (backup at {})", repo.full_name(), id),
                                Err((repo, e)) => {
                                    println!(" - {} (backup failed)", repo.full_name());
                                    eprintln!("{}", e)
                                },
                            }
                        }
                    },
                    Err(e) => {
                        eprintln!("Failed to get repositories for policy '{}'", policy);
                        eprintln!("{}", e);
                        continue;
                    }
                }
                
                println!();
            }
        }

        if let Some(next_run) = next_run {
            println!("Next backup scheduled for: {}", next_run);

            while chrono::Utc::now() < next_run && !CANCEL.load(std::sync::atomic::Ordering::Relaxed) {
                tokio::time::sleep(Duration::from_millis(500)).await;
            }
        } else {
            break;
        }
    }

    Ok(())
}

#[tokio::main]
async fn main() {
    ctrlc::set_handler(|| {
        CANCEL.store(true, std::sync::atomic::Ordering::Relaxed);
    }).unwrap_or_default();

    let args = Args::parse();

    telemetry::setup();

    let result = run(args).await;

    telemetry::shutdown();

    if let Err(e) = result {
        eprintln!("{}", e);
        std::process::exit(1);
    }
}
