mod credentials;
#[macro_use]
mod macros;

use crate::{FilterValue, Filterable};

pub use credentials::Credentials;
use std::collections::HashMap;
use unicase::UniCase;

pub trait BackupEntity: std::fmt::Display + Filterable {
    fn name(&self) -> &str;
    fn target_path(&self) -> std::path::PathBuf {
        self.name().into()
    }
}

#[derive(Default, Clone, Debug)]
pub struct Metadata(HashMap<UniCase<&'static str>, FilterValue>);

impl Metadata {
    pub fn insert<V: Into<FilterValue>>(&mut self, key: &'static str, value: V) {
        self.0.insert(UniCase::new(key), value.into());
    }

    pub fn get(&self, key: &str) -> FilterValue {
        self.0
            .get(&UniCase::new(key))
            .cloned()
            .unwrap_or(FilterValue::Null)
    }
}

pub trait MetadataSource {
    fn inject_metadata(&self, metadata: &mut Metadata);
}

entity!(HttpFile(url: U => String) {
    with_credentials => credentials: Credentials,
    with_last_modified => last_modified: Option<chrono::DateTime<chrono::Utc>>,
    with_content_type => content_type: Option<String>,
});

entity!(GitRepo(clone_url: U => String) {
    with_credentials => credentials: Credentials,
});
