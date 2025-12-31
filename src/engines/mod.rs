mod git;
mod http_file;

pub use git::GitEngine;
pub use http_file::HttpFileEngine;

use crate::BackupEntity;
use std::fmt::Display;
use std::path::Path;
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
    async fn backup<P: AsRef<Path> + Send>(
        &self,
        entity: &E,
        target: P,
        cancel: &AtomicBool,
    ) -> Result<BackupState, human_errors::Error>;
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
