use clap::Parser;
use engines::BackupState;
use errors::Error;
use pairing::PairingHandler;
use std::sync::atomic::AtomicBool;
use std::time::Duration;
use tracing_batteries::prelude::*;

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

use crate::helpers::github::GitHubArtifactKind;
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

    let github_repo = pairing::Pairing::new(sources::GitHubRepoSource::repo(), engines::GitEngine)
        .with_dry_run(args.dry_run)
        .with_concurrency_limit(args.concurrency);

    let github_star = pairing::Pairing::new(sources::GitHubRepoSource::star(), engines::GitEngine)
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
                    k if k == GitHubArtifactKind::Repo.as_str() => {
                        info!("Backing up repositories for {}", &policy);
                        github_repo
                            .run(policy, &LoggingPairingHandler, &CANCEL)
                            .await;
                    }
                    k if k == GitHubArtifactKind::Star.as_str() => {
                        info!("Backing up starred repositories for {}", &policy);
                        github_star
                            .run(policy, &LoggingPairingHandler, &CANCEL)
                            .await;
                    }
                    k if k == GitHubArtifactKind::Release.as_str() => {
                        info!("Backing up release artifacts for {}", &policy);
                        github_release
                            .run(policy, &LoggingPairingHandler, &CANCEL)
                            .await;
                    }
                    _ => {
                        error!("Unknown policy kind: {}", policy.kind);
                    }
                }

                println!();
            }
        }

        if CANCEL.load(std::sync::atomic::Ordering::Relaxed) {
            break;
        }

        if let Some(next_run) = next_run {
            info!("Next backup scheduled for: {}", next_run);

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

pub struct LoggingPairingHandler;

impl<E: BackupEntity> PairingHandler<E> for LoggingPairingHandler {
    fn on_complete(&self, entity: E, state: BackupState) {
        info!(" - {} ({})", entity, state);
    }

    fn on_error(&self, error: crate::Error) {
        warn!("Error: {}", error);
    }
}

#[tokio::main]
async fn main() {
    ctrlc::set_handler(|| {
        CANCEL.store(true, std::sync::atomic::Ordering::Relaxed);
        info!("Received SIGINT, shutting down...");
    })
    .unwrap_or_default();

    let args = Args::parse();

    let session = telemetry::setup();

    let result = run(args).await;

    if let Err(e) = result {
        session.record_error(&e);
        error!("{}", e);
        session.shutdown();
        std::process::exit(1);
    } else {
        session.shutdown();
    }
}
