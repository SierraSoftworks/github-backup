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
