use clap::Parser;
use engines::BackupState;
use human_errors::Error;
use pairing::PairingHandler;
use ping::Pinger;
use std::sync::atomic::{AtomicBool, AtomicUsize};
use std::time::Duration;
use tracing_batteries::prelude::*;
use tracing_batteries::{Analytics, OpenTelemetry, Session};

#[macro_use]
mod macros;

mod config;
mod engines;
mod entities;
mod errors;
pub(crate) mod helpers;
mod pairing;
mod ping;
mod policy;
mod sources;
mod target;
mod telemetry;

use crate::helpers::github::GitHubArtifactKind;
use crate::pairing::SummaryStatistics;
pub use entities::BackupEntity;
pub use filt_rs::{Filter, FilterValue, Filterable};
pub use policy::BackupPolicy;
pub use sources::BackupSource;
pub use target::BackupTarget;

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

async fn run(args: Args, session: &Session) -> Result<(), Error> {
    let config = config::Config::try_from(&args)?;

    let pinger = Pinger::new(config.ping.clone());

    let github_repo = pairing::Pairing::new(
        sources::GitHubRepoSource::default(),
        engines::RepoEngine::new(),
    )
    .with_dry_run(args.dry_run)
    .with_concurrency_limit(args.concurrency);

    let github_gist = pairing::Pairing::new(
        sources::GitHubGistSource::default(),
        engines::RepoEngine::new(),
    )
    .with_dry_run(args.dry_run)
    .with_concurrency_limit(args.concurrency);

    let github_release = pairing::Pairing::new(
        sources::GitHubReleasesSource::default(),
        engines::ReleaseEngine::new(),
    )
    .with_dry_run(args.dry_run)
    .with_concurrency_limit(args.concurrency);

    while !CANCEL.load(std::sync::atomic::Ordering::Relaxed) {
        let next_run = config
            .schedule
            .as_ref()
            .and_then(|s| s.find_next_occurrence(&chrono::Utc::now(), false).ok());

        let handler = LoggingPairingHandler::new(session);

        pinger.on_start().await;

        {
            let _span = info_span!("backup.all").entered();
            let _page = session.record_new_page("/backup");

            for policy in config.backups.iter() {
                let _policy_span = info_span!("backup.policy", policy = %policy).entered();

                match policy.kind.as_str() {
                    k if k == GitHubArtifactKind::Repo.as_str() => {
                        info!("Backing up repositories for {}", &policy);
                        github_repo.run(policy, &handler, &CANCEL).await;
                    }
                    k if k == GitHubArtifactKind::Release.as_str() => {
                        info!("Backing up release artifacts for {}", &policy);
                        github_release.run(policy, &handler, &CANCEL).await;
                    }
                    k if k == GitHubArtifactKind::Gist.as_str() => {
                        info!("Backing up gist artifacts for {}", &policy);
                        github_gist.run(policy, &handler, &CANCEL).await;
                    }
                    _ => {
                        error!("Unknown policy kind: {}", policy.kind);
                    }
                }
            }
        }

        if CANCEL.load(std::sync::atomic::Ordering::Relaxed) {
            // The run was interrupted (e.g. by SIGINT), so we deliberately avoid
            // reporting either success or failure to the cron monitor.
            break;
        }

        if handler.errors() > 0 {
            pinger.on_failure().await;
        } else {
            pinger.on_success().await;
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

pub struct LoggingPairingHandler<'a> {
    session: &'a Session,
    errors: AtomicUsize,
}

impl<'a> LoggingPairingHandler<'a> {
    fn new(session: &'a Session) -> Self {
        Self {
            session,
            errors: AtomicUsize::new(0),
        }
    }

    /// The total number of errors observed across every policy reported to this
    /// handler, used to decide whether a backup run should be reported as a
    /// success or a failure to the cron monitor.
    fn errors(&self) -> usize {
        self.errors.load(std::sync::atomic::Ordering::Relaxed)
    }
}

impl<E: BackupEntity> PairingHandler<E> for LoggingPairingHandler<'_> {
    fn on_complete(&self, _policy: &BackupPolicy, entity: E, state: BackupState) {
        match &state {
            state @ BackupState::Unchanged(_) | state @ BackupState::Skipped => {
                debug!(" - {} ({})", entity, state)
            }
            _ => info!(" - {} ({})", entity, state),
        }
    }

    fn on_error(&self, policy: &BackupPolicy, error: Error) {
        self.errors
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);

        if error.is(human_errors::Kind::System) {
            let info = tracing_batteries::ErrorInfo::new(&error)
                .with_metadata("policy.kind", policy.kind.to_string());

            self.session.record_custom_error(info);
        } else {
            error!("Error: {}", error);
        }

        warn!("Error: {}", error);
    }

    fn on_summary(&self, policy: &BackupPolicy, summary: SummaryStatistics) {
        info!(
            "Backup completed after {}s: {summary}",
            summary.duration().as_secs()
        );

        self.session.record_event(
            "policy::run",
            [
                ("policy.kind", policy.kind.to_string()),
                ("stats.new", summary.new.to_string()),
                ("stats.unchanged", summary.unchanged.to_string()),
                ("stats.updated", summary.updated.to_string()),
                ("stats.skipped", summary.skipped.to_string()),
                ("stats.error", summary.error.to_string()),
            ]
            .map(|(k, v)| (k.to_string(), v))
            .into(),
        );
    }
}

#[tokio::main]
async fn main() {
    ctrlc::set_handler(|| {
        CANCEL.store(true, std::sync::atomic::Ordering::Relaxed);
        warn!("Received SIGINT, shutting down...");
    })
    .unwrap_or_default();

    let args = Args::parse();

    let session = Session::new("github-backup", version!())
        .with_battery(OpenTelemetry::new(""))
        .with_battery(Analytics::new("https://analytics.sierrasoftworks.com"));

    let result = run(args, &session).await;

    if let Err(e) = result {
        session.record_error(&e);
        error!("{}", human_errors::pretty(&e));
        session.shutdown();
        std::process::exit(1);
    } else {
        session.shutdown();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entities::GitRepo;

    #[test]
    fn logging_handler_counts_errors() {
        let session =
            Session::new("github-backup", version!()).with_battery(tracing_batteries::Testing);

        let handler = LoggingPairingHandler::new(&session);
        assert_eq!(handler.errors(), 0);

        // Each reported error should be accumulated so that a run with any
        // failures can be reported to the cron monitor as a failure.
        let policy = BackupPolicy::default();
        PairingHandler::<GitRepo>::on_error(&handler, &policy, human_errors::user("boom", &[]));
        PairingHandler::<GitRepo>::on_error(&handler, &policy, human_errors::user("boom", &[]));
        assert_eq!(handler.errors(), 2);

        // Successful completions must not affect the error count.
        let repo = GitRepo::new("octocat/Hello-World", "https://example.com/repo.git", None);
        handler.on_complete(&policy, repo, BackupState::Skipped);
        assert_eq!(handler.errors(), 2);
    }
}
