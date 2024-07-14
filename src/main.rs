use clap::Parser;
use errors::Error;
use policy::BackupPolicy;
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use tokio::task::JoinSet;

#[macro_use]
mod macros;

mod config;
mod errors;
mod policy;
mod sources;
mod targets;
mod telemetry;

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
    telemetry::setup();

    let config = config::Config::try_from(&args)?;

    let github = sources::GitHubSource::from(&config);
    let git_backup = targets::FileSystemBackupTarget::from(&config);

    let cancel = AtomicBool::new(false);

    loop {
        let _span = tracing::info_span!("backup.all").entered();

        for policy in config.backups.iter() {
            let _span = tracing::info_span!("backup.policy", policy = %policy).entered();

            println!("Backing up repositories for: {}", &policy);
            let repos = github.get_repos(policy, &cancel).await?;

            let mut join_set: JoinSet<Result<(_, String), (_, errors::Error)>> = JoinSet::new();
            for repo in repos {
                if policy.filters.iter().all(|p| repo.matches(p)) {
                    if args.dry_run {
                        println!(" - {} (dry-run)", repo.full_name());
                        continue;
                    }

                    let git_backup = git_backup.clone();
                    let cancel = AtomicBool::new(false);

                    join_set.spawn(async move {
                        match git_backup.backup(&repo, &cancel) {
                            Ok(id) => Ok((repo, id)),
                            Err(e) => Err((repo, e)),
                        }
                    });
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

            println!();
        }

        if let Some(schedule) = &config.schedule {
            let now = chrono::Utc::now();
            match schedule.find_next_occurrence(&now, false) {
                Ok(next) => {
                    let wait = next - now;
                    println!("Next backup scheduled for: {}", next);
                    tokio::time::sleep(wait.to_std().unwrap()).await;
                }
                Err(err) => {
                    eprintln!("Failed to calculate the next backup time: {}", err);
                    break;
                }
            }
        } else {
            break;
        }
    }

    Ok(())
}

#[tokio::main]
async fn main() {
    let args = Args::parse();
    if let Err(e) = run(args).await {
        eprintln!("{}", e);
        std::process::exit(1);
    }
}
