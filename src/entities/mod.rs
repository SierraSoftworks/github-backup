mod credentials;
#[macro_use]
mod macros;
mod recovery;
mod release;

use crate::{FilterValue, Filterable};

pub use credentials::Credentials;
pub use recovery::RecoveryMode;
pub use release::Release;
use std::borrow::Cow;
use std::collections::HashMap;
use unicase::UniCase;

pub trait BackupEntity: std::fmt::Display + Filterable {
    fn name(&self) -> &str;
    fn target_path(&self) -> std::path::PathBuf {
        self.name().into()
    }
}

#[derive(Default, Clone, Debug)]
pub struct Metadata(HashMap<UniCase<&'static str>, FilterValue<'static>>);

impl Metadata {
    pub fn insert<'a, V: Into<FilterValue<'a>>>(&mut self, key: &'static str, value: V) {
        self.0.insert(UniCase::new(key), into_owned(value.into()));
    }

    pub fn get(&self, key: &str) -> FilterValue<'_> {
        self.0
            .get(&UniCase::new(key))
            .cloned()
            .unwrap_or(FilterValue::Null)
    }
}

/// Converts a [`FilterValue`] into one which owns all of its data so that it
/// can be cached within a [`Metadata`] collection (whose entries must outlive
/// the entity they were derived from).
fn into_owned(value: FilterValue<'_>) -> FilterValue<'static> {
    match value {
        FilterValue::Null => FilterValue::Null,
        FilterValue::Bool(b) => FilterValue::Bool(b),
        FilterValue::Number(n) => FilterValue::Number(n),
        FilterValue::String(s) => FilterValue::String(Cow::Owned(s.into_owned())),
        FilterValue::Tuple(v) => FilterValue::Tuple(v.into_iter().map(into_owned).collect()),
        FilterValue::DateTime(dt) => FilterValue::DateTime(dt),
        FilterValue::Duration(d) => FilterValue::Duration(d),
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

entity!(GitRepo(clone_url: U => String, refspecs: R => Option<Vec<String>>) {
    with_credentials => credentials: Credentials,
    with_recovery_mode => recovery_mode: RecoveryMode,
});
