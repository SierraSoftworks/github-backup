mod expr;
mod lexer;
mod parser;
mod token;
mod value;

use std::{pin::Pin, ptr::NonNull};

use expr::{Expr, ExprVisitor};
use token::Token;
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
        let mut context = FilterContext { target };
        Ok(context.visit_expr(&self.ast).is_truthy())
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

struct FilterContext<'a, T: Filterable> {
    target: &'a T,
}

impl<'a, T: Filterable> ExprVisitor<FilterValue> for FilterContext<'a, T> {
    fn visit_literal(&mut self, value: &FilterValue) -> FilterValue {
        value.clone()
    }

    fn visit_property(&mut self, name: &str) -> FilterValue {
        self.target.get(name).clone()
    }

    fn visit_binary(&mut self, left: &Expr, operator: &Token, right: &Expr) -> FilterValue {
        let left = self.visit_expr(left);
        let right = self.visit_expr(right);
        match operator {
            Token::Equals => (left == right).into(),
            Token::NotEquals => (left != right).into(),
            Token::Contains => left.contains(&right).into(),
            Token::In => right.contains(&left).into(),
            Token::StartsWith => left.startswith(&right).into(),
            Token::EndsWith => left.endswith(&right).into(),
            token => unreachable!("Encountered an unexpected binary operator '{token}'"),
        }
    }

    fn visit_logical(&mut self, left: &Expr, operator: &Token, right: &Expr) -> FilterValue {
        let left = self.visit_expr(left);

        match operator {
            Token::And if left.is_truthy() => self.visit_expr(right),
            Token::And => false.into(),
            Token::Or if !left.is_truthy() => self.visit_expr(right),
            Token::Or => true.into(),
            token => unreachable!("Encountered an unexpected logical operator '{token}'"),
        }
    }

    fn visit_unary(&mut self, operator: &Token, right: &Expr) -> FilterValue {
        let right = self.visit_expr(right);

        match operator {
            Token::Not => {
                if right.is_truthy() {
                    false.into()
                } else {
                    right
                }
            }
            token => unreachable!("Encountered an unexpected unary operator '{token}'"),
        }
    }
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::*;

    struct TestObject {
        name: String,
        age: i32,
        tags: Vec<&'static str>,
    }

    impl Filterable for TestObject {
        fn get(&self, property: &str) -> FilterValue {
            match property {
                "name" => self.name.clone().into(),
                "age" => self.age.into(),
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
    #[case("tags == [\"red\"]", true)]
    #[case("tags != [\"red\"]", false)]
    #[case("tags == [\"blue\"]", false)]
    #[case("tags contains \"red\"", true)]
    #[case("tags contains \"blue\"", false)]
    #[case("\"red\" in tags", true)]
    #[case("\"blue\" in tags", false)]
    fn case_sensitive_filtering(#[case] filter: &str, #[case] matches: bool) {
        let obj = TestObject {
            name: "John Doe".to_string(),
            age: 30,
            tags: vec!["red"],
        };

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
        let obj = TestObject {
            name: "John Doe".to_string(),
            age: 30,
            tags: vec!["red"],
        };

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
        let obj = TestObject {
            name: "John Doe".to_string(),
            age: 30,
            tags: vec!["red"],
        };

        assert_eq!(
            Filter::new(filter)
                .expect("parse filter")
                .matches(&obj)
                .expect("run filter"),
            matches
        );
    }
}
