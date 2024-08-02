use std::collections::HashMap;
use std::fmt::{Debug, Display};

use unicase::UniCase;

use crate::{FilterValue, Filterable};

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
    pub metadata: HashMap<UniCase<&'static str>, FilterValue>,
}

impl GitRepo {
    pub fn new<N: Into<String>, C: Into<String>>(name: N, clone_url: C) -> Self {
        Self {
            name: name.into(),
            clone_url: clone_url.into(),
            credentials: Credentials::None,
            metadata: HashMap::new(),
        }
    }

    pub fn with_credentials(mut self, credentials: Credentials) -> Self {
        self.credentials = credentials;
        self
    }

    pub fn with_metadata<V: Into<FilterValue>>(mut self, key: &'static str, value: V) -> Self {
        self.metadata.insert(UniCase::new(key), value.into());
        self
    }
}

impl BackupEntity for GitRepo {
    fn name(&self) -> &str {
        &self.name
    }
}

impl Filterable for GitRepo {
    fn get(&self, key: &str) -> FilterValue {
        self.metadata
            .get(&UniCase::new(key))
            .cloned()
            .unwrap_or(FilterValue::Null)
    }
}

impl Display for GitRepo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} ({})", self.name, self.clone_url)
    }
}

impl Debug for GitRepo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} ({})", self.name, self.clone_url)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_entity_implementation() {
        let repo = GitRepo::new("org/repo", "https://github.com/org/repo.git")
            .with_credentials(Credentials::Token("token".to_string()))
            .with_metadata("repo.name", "repo")
            .with_metadata("repo.archived", true)
            .with_metadata("repo.public", true)
            .with_metadata("repo.fork", false);

        assert_eq!(repo.name(), "org/repo");
        assert_eq!(repo.get("repo.archived"), true.into());
        assert_eq!(repo.get("repo.public"), true.into());
        assert_eq!(repo.get("repo.fork"), false.into());
    }
}
