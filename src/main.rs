use clap::Parser;
use errors::Error;
use std::sync::atomic::AtomicBool;
use std::time::Duration;
use tokio_stream::StreamExt;

#[macro_use]
mod macros;

mod config;
mod engines;
mod entities;
mod errors;
mod filter;
pub(crate) mod helpers;
mod pairing;
mod policy;
mod sources;
mod telemetry;

pub use entities::BackupEntity;
pub use filter::{Filter, FilterValue, Filterable};
pub use policy::BackupPolicy;
pub use sources::BackupSource;

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

    /// The maximum number of concurrent backup tasks which are permitted to run at a given time.
    #[arg(long, default_value = "10")]
    pub concurrency: usize,
}

async fn run(args: Args) -> Result<(), Error> {
    let config = config::Config::try_from(&args)?;

    let github_repo =
        pairing::Pairing::new(sources::GitHubRepoSource::default(), engines::GitEngine)
            .with_dry_run(args.dry_run)
            .with_concurrency_limit(args.concurrency);

    let github_release = pairing::Pairing::new(
        sources::GitHubReleasesSource::default(),
        engines::HttpFileEngine::new(),
    )
    .with_dry_run(args.dry_run)
    .with_concurrency_limit(args.concurrency);

    while !CANCEL.load(std::sync::atomic::Ordering::Relaxed) {
        let next_run = config
            .schedule
            .as_ref()
            .and_then(|s| s.find_next_occurrence(&chrono::Utc::now(), false).ok());

        {
            let _span = tracing::info_span!("backup.all").entered();

            for policy in config.backups.iter() {
                let _policy_span = tracing::info_span!("backup.policy", policy = %policy).entered();

                match policy.kind.as_str() {
                    "github/repo" => {
                        println!("Backing up repositories for {}", &policy);

                        let stream = github_repo.run(policy, &CANCEL);
                        tokio::pin!(stream);
                        while let Some(result) = stream.next().await {
                            match result {
                                Ok((entity, state)) => {
                                    println!(" - {} ({})", entity, state);
                                }
                                Err(e) => {
                                    eprintln!("Error: {}", e);
                                }
                            }
                        }
                    }
                    "github/release" => {
                        println!("Backing up release artifacts for {}", &policy);

                        let stream = github_release.run(policy, &CANCEL);
                        tokio::pin!(stream);
                        while let Some(result) = stream.next().await {
                            match result {
                                Ok((entity, state)) => {
                                    println!(" - {} ({})", entity, state);
                                }
                                Err(e) => {
                                    eprintln!("Error: {}", e);
                                }
                            }
                        }
                    }
                    _ => {
                        eprintln!("Unknown policy kind: {}", policy.kind);
                    }
                }

                println!();
            }
        }

        if let Some(next_run) = next_run {
            println!("Next backup scheduled for: {}", next_run);

            while chrono::Utc::now() < next_run
                && !CANCEL.load(std::sync::atomic::Ordering::Relaxed)
            {
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
    })
    .unwrap_or_default();

    let args = Args::parse();

    telemetry::setup();

    let result = run(args).await;

    telemetry::shutdown();

    if let Err(e) = result {
        eprintln!("{}", e);
        std::process::exit(1);
    }
}
