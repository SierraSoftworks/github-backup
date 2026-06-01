mod forgejo;
mod git;
mod http_file;
mod release;

pub use forgejo::{ForgejoReleaseEngine, ForgejoRepoEngine};
pub use git::GitEngine;
pub use http_file::HttpFileEngine;
pub use release::ReleaseEngine;

use crate::BackupEntity;
use crate::entities::GitRepo;
use crate::target::{BackupTarget, RemoteTargetKind};
use std::fmt::Display;
use std::sync::atomic::AtomicBool;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BackupState {
    Skipped,
    New(Option<String>),
    Updated(Option<String>),
    Unchanged(Option<String>),
}

#[async_trait::async_trait]
pub trait BackupEngine<E: BackupEntity> {
    async fn backup(
        &self,
        entity: &E,
        target: &BackupTarget,
        cancel: &AtomicBool,
    ) -> Result<BackupState, human_errors::Error>;
}

/// A composite engine which backs up Git repositories either to the local
/// filesystem or to a Forgejo instance, depending on the configured target.
#[derive(Clone, Default)]
pub struct RepoEngine {
    git: GitEngine,
    forgejo: ForgejoRepoEngine,
}

impl RepoEngine {
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait::async_trait]
impl BackupEngine<GitRepo> for RepoEngine {
    async fn backup(
        &self,
        entity: &GitRepo,
        target: &BackupTarget,
        cancel: &AtomicBool,
    ) -> Result<BackupState, human_errors::Error> {
        match target {
            BackupTarget::FileSystem(path) => self.git.backup(entity, path, cancel).await,
            BackupTarget::Remote(remote) => match remote.kind {
                RemoteTargetKind::ForgejoRepo => self.forgejo.backup(entity, remote, cancel).await,
                RemoteTargetKind::ForgejoRelease => Err(human_errors::user(
                    "You have configured a 'forgejo/release' target for a repository backup, which is not supported.",
                    &[
                        "Use a 'forgejo/repo' target to back up repositories, or change the policy 'kind' to 'github/release' to back up release artifacts.",
                    ],
                )),
            },
        }
    }
}

impl Display for BackupState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BackupState::Skipped => write!(f, "skipped"),
            BackupState::New(Some(s)) => write!(f, "new {}", s),
            BackupState::Updated(Some(s)) => write!(f, "updated {}", s),
            BackupState::Unchanged(Some(s)) => write!(f, "unchanged {}", s),
            BackupState::New(None) => write!(f, "new"),
            BackupState::Updated(None) => write!(f, "updated"),
            BackupState::Unchanged(None) => write!(f, "unchanged"),
        }
    }
}

/// Combines the backup states of the individual components of a composite
/// entity (such as a release's assets and notes) into a single state.
///
/// The combined state reflects the "strongest" change applied: a `New`
/// component takes precedence over an `Updated` one, which takes precedence
/// over `Unchanged`. The description summarises how many components fell into
/// each category. An empty set of components is treated as `Skipped`.
pub(crate) fn summarize_states(states: &[BackupState]) -> BackupState {
    let mut new = 0;
    let mut updated = 0;
    let mut unchanged = 0;
    let mut skipped = 0;

    for state in states {
        match state {
            BackupState::New(_) => new += 1,
            BackupState::Updated(_) => updated += 1,
            BackupState::Unchanged(_) => unchanged += 1,
            BackupState::Skipped => skipped += 1,
        }
    }

    let summary = [
        (new, "new"),
        (updated, "updated"),
        (unchanged, "unchanged"),
        (skipped, "skipped"),
    ]
    .iter()
    .filter(|(count, _)| *count > 0)
    .map(|(count, label)| format!("{count} {label}"))
    .collect::<Vec<_>>()
    .join(", ");

    let summary = Some(summary).filter(|s| !s.is_empty());

    if new > 0 {
        BackupState::New(summary)
    } else if updated > 0 {
        BackupState::Updated(summary)
    } else if unchanged > 0 {
        BackupState::Unchanged(summary)
    } else {
        BackupState::Skipped
    }
}
