macro_rules! entity {
    ($name:ident ($($rfield:ident: $rgtype:ident => $rtype:ty),* $(,)?) {
        $($setter:ident => $field:ident: $type:ty),* $(,)?
    }) => {
        #[derive(Clone, Debug)]
        pub struct $name {
            pub name: String,
            $(pub $rfield: $rtype,)*
            $(pub $field: $type,)*
            pub metadata: $crate::entities::Metadata,
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

            pub fn with_metadata_source(mut self, source: &dyn $crate::entities::MetadataSource) -> Self {
                source.inject_metadata(&mut self.metadata);
                self
            }
        }

        impl $crate::entities::BackupEntity for $name {
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

#[cfg(test)]
mod tests {
    use crate::{entities::Credentials, FilterValue, Filterable};

    entity!(TestEntity(url: U  => String) {
      with_credentials => credentials: Credentials,
    });

    #[test]
    fn test_entity() {
        let entity = TestEntity::new("test", "http://example.com")
            .with_credentials(Credentials::Token("test".to_string()))
            .with_metadata("test", "test")
            .with_metadata("test2", 1);

        assert_eq!(entity.name, "test");
        assert_eq!(entity.url, "http://example.com");
        assert_eq!(entity.credentials, Credentials::Token("test".to_string()));

        assert_eq!(entity.get("test"), FilterValue::String("test".to_string()));
        assert_eq!(entity.get("test2"), FilterValue::Number(1 as f64));
    }
}
