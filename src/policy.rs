use serde::Deserialize;
use std::collections::HashMap;
use std::fmt::{Debug, Display, Formatter};
use std::path::PathBuf;

use crate::entities::Credentials;

#[derive(Deserialize)]
pub struct BackupPolicy {
    pub kind: String,
    pub from: String,
    #[serde(default = "default_backup_path")]
    pub to: PathBuf,
    #[serde(default)]
    pub credentials: Credentials,
    #[serde(default)]
    pub filters: Vec<BackupFilter>,
    #[serde(default)]
    pub properties: HashMap<String, String>,
}

impl Display for BackupPolicy {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}", self.kind, self.from)
    }
}

impl Debug for BackupPolicy {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}", self.kind, self.from)
    }
}

#[derive(Debug, Deserialize)]
pub enum BackupFilter {
    Include(Vec<String>),
    Exclude(Vec<String>),
    Public,
    Private,
    NonEmpty,
    Fork,
    NonFork,
    Archived,
    NonArchived,
}

impl Display for BackupFilter {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            BackupFilter::Include(names) => write!(f, "Include: [{}]", names.join(", ")),
            BackupFilter::Exclude(names) => write!(f, "Exclude: [{}]", names.join(", ")),
            BackupFilter::Public => write!(f, "Public"),
            BackupFilter::Private => write!(f, "Private"),
            BackupFilter::NonEmpty => write!(f, "NonEmpty"),
            BackupFilter::Fork => write!(f, "Fork"),
            BackupFilter::NonFork => write!(f, "NonFork"),
            BackupFilter::Archived => write!(f, "Archived"),
            BackupFilter::NonArchived => write!(f, "NonArchived"),
        }
    }
}

fn default_backup_path() -> PathBuf {
    PathBuf::from("./backups")
}
