use serde::Deserialize;
use std::fmt::{Display, Formatter};

#[derive(Debug, Deserialize)]
pub struct BackupPolicy {
    pub user: Option<String>,
    pub org: Option<String>,
    #[serde(default)]
    pub filters: Vec<RepoFilter>,
}

impl Display for BackupPolicy {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match (self.user.as_ref(), self.org.as_ref()) {
            (Some(user), None) => write!(f, "@{} ({})", user, self.filters.iter().map(|f| f.to_string()).collect::<Vec<String>>().join(", ")),
            (None, Some(org)) => write!(f, "@{} ({})", org, self.filters.iter().map(|f| f.to_string()).collect::<Vec<String>>().join(", ")),
            _ => write!(f, "<INVALID POLICY>"),
        }
    }
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

impl Display for RepoFilter {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            RepoFilter::Include(names) => write!(f, "Include: [{}]", names.join(", ")),
            RepoFilter::Exclude(names) => write!(f, "Exclude: [{}]", names.join(", ")),
            RepoFilter::Public => write!(f, "Public"),
            RepoFilter::Private => write!(f, "Private"),
            RepoFilter::NonEmpty => write!(f, "NonEmpty"),
            RepoFilter::Fork => write!(f, "Fork"),
            RepoFilter::NonFork => write!(f, "NonFork"),
            RepoFilter::Archived => write!(f, "Archived"),
            RepoFilter::NonArchived => write!(f, "NonArchived"),
        }
    }
}