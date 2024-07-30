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
    Is(String),
    IsNot(String),
}

impl Display for BackupFilter {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            BackupFilter::Include(names) => write!(f, "name in [{}]", names.join(", ")),
            BackupFilter::Exclude(names) => write!(f, "name !in [{}]", names.join(", ")),
            BackupFilter::Is(name) => write!(f, "#{}", name),
            BackupFilter::IsNot(name) => write!(f, "!#{}", name),
        }
    }
}

fn default_backup_path() -> PathBuf {
    PathBuf::from("./backups")
}
