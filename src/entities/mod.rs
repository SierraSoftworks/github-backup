mod credentials;

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

macro_rules! entity {
    ($name:ident ($($rfield:ident: $rgtype:ident => $rtype:ty),* $(,)?) {
        $($setter:ident => $field:ident: $type:ty),* $(,)?
    }) => {
        #[derive(Clone, Debug)]
        pub struct $name {
            pub name: String,
            $(pub $rfield: $rtype,)*
            $(pub $field: $type,)*
            pub metadata: Metadata,
        }

        #[allow(dead_code)]
        impl $name {
            #[allow(non_camel_case_types)]
            pub fn new<N: Into<String> $(,$rgtype: Into<$rtype>)*>(name: N, $($rfield: $rgtype,)*) -> Self {
                Self {
                    name: name.into(),
                    $($rfield: $rfield.into(),)*
                    $($field: Default::default(),)*
                    metadata: Default::default(),
                }
            }

            $(
            pub fn $setter(mut self, $field: $type) -> Self {
                self.$field = $field;
                self
            }
            )*

            pub fn with_metadata<V: Into<FilterValue>>(mut self, key: &'static str, value: V) -> Self {
                self.metadata.insert(key, value.into());
                self
            }

            pub fn with_metadata_source(mut self, source: &dyn MetadataSource) -> Self {
                source.inject_metadata(&mut self.metadata);
                self
            }
        }

        impl BackupEntity for $name {
            fn name(&self) -> &str {
                &self.name
            }
        }

        impl crate::Filterable for $name {
            fn get(&self, key: &str) -> crate::FilterValue {
                self.metadata.get(key)
            }
        }

        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "{}", self.name)
            }
        }
    }
}

entity!(HttpFile(url: U => String) {
    with_credentials => credentials: Credentials,
    with_last_modified => last_modified: Option<chrono::DateTime<chrono::Utc>>,
    with_content_type => content_type: Option<String>,
});

entity!(GitRepo(clone_url: U => String) {
    with_credentials => credentials: Credentials,
});