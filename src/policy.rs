use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct BackupPolicy {
    pub org: String,
    #[serde(default)]
    pub filters: Vec<RepoFilter>,
}

#[derive(Debug, Deserialize)]
pub enum RepoFilter {
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
