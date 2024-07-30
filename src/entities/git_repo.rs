use std::collections::HashSet;
use std::fmt::{Debug, Display};

use super::{BackupEntity, Credentials};

pub const TAG_EMPTY: &str = "empty";
pub const TAG_ARCHIVED: &str = "archived";
pub const TAG_FORK: &str = "fork";
pub const TAG_PRIVATE: &str = "private";

#[derive(Clone)]
pub struct GitRepo {
    pub name: String,
    pub clone_url: String,
    pub credentials: Credentials,
    pub tags: HashSet<&'static str>,
}

impl GitRepo {
    pub fn new<N: Into<String>, C: Into<String>>(name: N, clone_url: C) -> Self {
        Self {
            name: name.into(),
            clone_url: clone_url.into(),
            credentials: Credentials::None,
            tags: HashSet::new(),
        }
    }

    pub fn with_credentials(mut self, credentials: Credentials) -> Self {
        self.credentials = credentials;
        self
    }

    pub fn with_optional_tag(mut self, tag: Option<&'static str>) -> Self {
        if let Some(tag) = tag {
            self.tags.insert(tag);
        }
        self
    }
}

impl BackupEntity for GitRepo {
    fn name(&self) -> &str {
        &self.name
    }

    fn matches(&self, filter: crate::BackupFilter) -> bool {
        match filter {
            crate::BackupFilter::Include(names) => {
                names.iter().any(|n| self.name.eq_ignore_ascii_case(n))
            }
            crate::BackupFilter::Exclude(names) => {
                !names.iter().any(|n| self.name.eq_ignore_ascii_case(n))
            }
            crate::BackupFilter::Archived => self.tags.contains(TAG_ARCHIVED),
            crate::BackupFilter::NonArchived => !self.tags.contains(TAG_ARCHIVED),
            crate::BackupFilter::Fork => self.tags.contains(TAG_FORK),
            crate::BackupFilter::NonFork => !self.tags.contains(TAG_FORK),
            crate::BackupFilter::Private => self.tags.contains(TAG_PRIVATE),
            crate::BackupFilter::Public => !self.tags.contains(TAG_PRIVATE),
            crate::BackupFilter::NonEmpty => !self.tags.contains(TAG_EMPTY),
        }
    }
}

impl Display for GitRepo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} ({})", self.name, self.credentials)
    }
}

impl Debug for GitRepo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} ({})", self.name, self.credentials)
    }
}
