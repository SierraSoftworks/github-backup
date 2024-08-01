use std::{collections::HashSet, fmt::Display, path::PathBuf};

use super::{BackupEntity, Credentials};

#[derive(Clone, Debug)]
pub struct HttpFile {
    pub url: String,
    pub name: String,
    pub filename: String,
    pub credentials: Credentials,
    pub tags: HashSet<&'static str>,
    pub last_modified: Option<chrono::DateTime<chrono::Utc>>,
    pub content_type: Option<String>,
}

impl HttpFile {
    pub fn new<N: Into<String> + Into<PathBuf>, U: Into<String>>(name: N, url: U) -> Self {
        let name: String = name.into();
        Self {
            name: name.clone(),
            filename: name,
            url: url.into(),
            credentials: Credentials::None,
            // TODO: Switch to a case-insensitive hasher here
            tags: HashSet::new(),
            last_modified: None,
            content_type: None,
        }
    }

    pub fn with_filename<P: Into<String>>(mut self, filename: P) -> Self {
        self.filename = filename.into();
        self
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

    pub fn with_content_type(mut self, content_type: Option<String>) -> Self {
        self.content_type = content_type;
        self
    }

    pub fn with_last_modified(
        mut self,
        last_modified: Option<chrono::DateTime<chrono::Utc>>,
    ) -> Self {
        self.last_modified = last_modified;
        self
    }
}

impl BackupEntity for HttpFile {
    fn name(&self) -> &str {
        &self.name
    }

    fn has_tag(&self, tag: &str) -> bool {
        self.tags.contains(&tag)
    }

    fn target_path(&self) -> std::path::PathBuf {
        self.filename.as_str().into()
    }
}

impl Display for HttpFile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.filename)
    }
}
