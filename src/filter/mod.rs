mod expr;
mod interpreter;
mod lexer;
mod location;
mod parser;
mod token;
mod value;

use std::{fmt::Display, pin::Pin, ptr::NonNull};

use expr::{Expr, ExprVisitor};
use interpreter::FilterContext;
pub use value::*;

pub struct Filter {
    #[allow(clippy::box_collection)]
    filter: Pin<Box<String>>,
    ast: Expr<'static>,
}

impl Filter {
    pub fn new<S: Into<String>>(filter: S) -> Result<Self, crate::Error> {
        let filter = Box::new(filter.into());
        let filter_ptr = NonNull::from(&filter);
        let pinned = Box::into_pin(filter);

        let tokens = crate::filter::lexer::Scanner::new(unsafe { filter_ptr.as_ref() });
        let ast = crate::filter::parser::Parser::parse(tokens.into_iter())?;
        Ok(Self {
            filter: pinned,
            ast,
        })
    }

    pub fn matches<T: Filterable>(&self, target: &T) -> Result<bool, crate::Error> {
        Ok(FilterContext::new(target).visit_expr(&self.ast).is_truthy())
    }

    /// Gets the raw filter expression which was used to construct this filter.
    pub fn raw(&self) -> &str {
        &self.filter
    }
}

impl Default for Filter {
    fn default() -> Self {
        Self {
            filter: Box::pin("true".to_string()),
            ast: Expr::Literal(FilterValue::Bool(true)),
        }
    }
}

impl Display for Filter {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{}", self.raw())
    }
}

impl<'de> serde::Deserialize<'de> for Filter {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct FilterVisitor;

        impl<'de> serde::de::Visitor<'de> for FilterVisitor {
            type Value = Filter;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("a valid filter expression")
            }

            fn visit_none<E>(self) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                Filter::new("true").map_err(serde::de::Error::custom)
            }

            fn visit_some<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
            where
                D: serde::Deserializer<'de>,
            {
                deserializer.deserialize_str(self)
            }

            fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                Filter::new(v).map_err(serde::de::Error::custom)
            }
        }

        deserializer.deserialize_option(FilterVisitor)
    }
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::*;

    struct TestObject {
        name: String,
        age: i32,
        alive: bool,
        tags: Vec<&'static str>,
    }

    impl Default for TestObject {
        fn default() -> Self {
            Self {
                name: "John Doe".to_string(),
                age: 30,
                alive: true,
                tags: vec!["red"],
            }
        }
    }

    impl Filterable for TestObject {
        fn get(&self, property: &str) -> FilterValue {
            match property {
                "name" => self.name.clone().into(),
                "age" => self.age.into(),
                "alive" => self.alive.into(),
                "tags" => self
                    .tags
                    .iter()
                    .cloned()
                    .map(|v| v.into())
                    .collect::<Vec<FilterValue>>()
                    .into(),
                _ => FilterValue::Null,
            }
        }
    }

    #[rstest]
    #[case("name == \"John Doe\"", true)]
    #[case("name != \"John Doe\"", false)]
    #[case("name == \"Jane Doe\"", false)]
    #[case("name != \"Jane Doe\"", true)]
    #[case("name startswith \"John\"", true)]
    #[case("name startswith \"Jane\"", false)]
    #[case("name endswith \"Doe\"", true)]
    #[case("name endswith \"Smith\"", false)]
    #[case("age == 30", true)]
    #[case("age != 30", false)]
    #[case("age == 31", false)]
    #[case("age != 31", true)]
    #[case("age > 31", false)]
    #[case("age < 31", true)]
    #[case("age >= 30", true)]
    #[case("age <= 30", true)]
    #[case("tags == [\"red\"]", true)]
    #[case("tags != [\"red\"]", false)]
    #[case("tags == [\"blue\"]", false)]
    #[case("tags contains \"red\"", true)]
    #[case("tags contains \"blue\"", false)]
    #[case("\"red\" in tags", true)]
    #[case("\"blue\" in tags", false)]
    fn case_sensitive_filtering(#[case] filter: &str, #[case] matches: bool) {
        let obj = TestObject::default();

        assert_eq!(
            Filter::new(filter)
                .expect("parse filter")
                .matches(&obj)
                .expect("run filter"),
            matches
        );
    }

    #[rstest]
    #[case("name == \"john doe\"", true)]
    #[case("name != \"john doe\"", false)]
    #[case("name == \"jane doe\"", false)]
    #[case("name != \"jane doe\"", true)]
    #[case("name startswith \"john\"", true)]
    #[case("name startswith \"jane\"", false)]
    #[case("name endswith \"doe\"", true)]
    #[case("name endswith \"smith\"", false)]
    #[case("\"RED\" in tags", true)]
    #[case("\"BLUE\" in tags", false)]
    fn case_insensitive_filtering(#[case] filter: &str, #[case] matches: bool) {
        let obj = TestObject::default();

        assert_eq!(
            Filter::new(filter)
                .expect("parse filter")
                .matches(&obj)
                .expect("run filter"),
            matches
        );
    }

    #[rstest]
    #[case("name == \"John Doe\" && age == 30", true)]
    #[case("name == \"John Doe\" && age == 31", false)]
    #[case("name == \"Jane Doe\" && age == 30", false)]
    #[case("name == \"John Doe\" || age == 30", true)]
    #[case("name == \"John Doe\" || age == 31", true)]
    #[case("name == \"Jane Doe\" || age == 30", true)]
    #[case("name == \"Jane Doe\" || age == 31", false)]
    fn binary_operator_filtering(#[case] filter: &str, #[case] matches: bool) {
        let obj = TestObject::default();

        assert_eq!(
            Filter::new(filter)
                .expect("parse filter")
                .matches(&obj)
                .expect("run filter"),
            matches
        );
    }

    #[rstest]
    #[case("alive", true)]
    #[case("!alive", false)]
    #[case("name && age", true)]
    #[case("name && !age", false)]
    fn logical_operator_filtering(#[case] filter: &str, #[case] matches: bool) {
        let obj = TestObject::default();

        assert_eq!(
            Filter::new(filter)
                .expect("parse filter")
                .matches(&obj)
                .expect("run filter"),
            matches
        );
    }
}
