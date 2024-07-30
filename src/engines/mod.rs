mod git;

pub use git::GitEngine;

use crate::BackupEntity;
use std::fmt::Display;
use std::sync::atomic::AtomicBool;

#[derive(Debug, Eq, PartialEq)]
pub enum BackupState {
    Skipped,
    New(Option<String>),
    Updated(Option<String>),
    Unchanged(Option<String>),
}

#[async_trait::async_trait]
pub trait BackupEngine<E: BackupEntity>: Clone + Send + Sync {
    async fn backup(
        &self,
        entity: &E,
        target: &std::path::Path,
        cancel: &AtomicBool,
    ) -> Result<BackupState, crate::Error>;
}

impl Display for BackupState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BackupState::Skipped => write!(f, "skipped"),
            BackupState::New(Some(s)) => write!(f, "new at {}", s),
            BackupState::Updated(Some(s)) => write!(f, "updated at {}", s),
            BackupState::Unchanged(Some(s)) => write!(f, "unchanged at {}", s),
            BackupState::New(None) => write!(f, "new"),
            BackupState::Updated(None) => write!(f, "updated"),
            BackupState::Unchanged(None) => write!(f, "unchanged"),
        }
    }
}
