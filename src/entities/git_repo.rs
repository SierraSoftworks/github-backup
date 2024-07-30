use std::collections::HashSet;
use std::fmt::{Debug, Display};

use super::{BackupEntity, Credentials};

// NOTE: Tags should always be lowercase
pub const TAG_EMPTY: &str = "empty";
pub const TAG_ARCHIVED: &str = "archived";
pub const TAG_FORK: &str = "fork";
pub const TAG_PRIVATE: &str = "private";
pub const TAG_PUBLIC: &str = "public";

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

    fn has_tag(&self, tag: &str) -> bool {
        self.tags.contains(&tag)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_entity_implementation() {
        let repo = GitRepo::new("org/repo", "https://github.com/org/repo.git")
            .with_credentials(Credentials::Token("token".to_string()))
            .with_optional_tag(Some(TAG_ARCHIVED))
            .with_optional_tag(Some(TAG_PUBLIC));

        assert_eq!(repo.name(), "org/repo");
        assert!(repo.has_tag(TAG_ARCHIVED));
        assert!(repo.has_tag(TAG_PUBLIC));
        assert!(!repo.has_tag(TAG_FORK));

        assert!(repo.matches(&crate::BackupFilter::Is(TAG_PUBLIC.to_string())));
    }
}
