mod forgejo;
mod git;
mod http_file;

pub use forgejo::{ForgejoReleaseEngine, ForgejoRepoEngine};
pub use git::GitEngine;
pub use http_file::HttpFileEngine;

use crate::BackupEntity;
use crate::entities::{GitRepo, HttpFile};
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

/// A composite engine which backs up release artifacts either to the local
/// filesystem or to a Forgejo instance, depending on the configured target.
#[derive(Clone, Default)]
pub struct ReleaseEngine {
    http: HttpFileEngine,
    forgejo: ForgejoReleaseEngine,
}

impl ReleaseEngine {
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait::async_trait]
impl BackupEngine<HttpFile> for ReleaseEngine {
    async fn backup(
        &self,
        entity: &HttpFile,
        target: &BackupTarget,
        cancel: &AtomicBool,
    ) -> Result<BackupState, human_errors::Error> {
        match target {
            BackupTarget::FileSystem(path) => self.http.backup(entity, path, cancel).await,
            BackupTarget::Remote(remote) => match remote.kind {
                RemoteTargetKind::ForgejoRelease => {
                    self.forgejo.backup(entity, remote, cancel).await
                }
                RemoteTargetKind::ForgejoRepo => Err(human_errors::user(
                    "You have configured a 'forgejo/repo' target for a release backup, which is not supported.",
                    &[
                        "Use a 'forgejo/release' target to back up release artifacts, or change the policy 'kind' to 'github/repo' to mirror repositories.",
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
