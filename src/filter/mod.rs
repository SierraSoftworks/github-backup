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

    #[test]
    fn test_filter() {
        let obj = TestObject {
            name: "John Doe".to_string(),
            age: 30,
            tags: vec!["red"],
        };

        assert!(Filter::new("name == \"John Doe\"")
            .expect("parse filter")
            .matches(&obj)
            .expect("run filter"));

        assert!(!Filter::new("name == \"Jane Doe\"")
            .expect("parse filter")
            .matches(&obj)
            .expect("run filter"));

        assert!(Filter::new("age == 30")
            .expect("parse filter")
            .matches(&obj)
            .expect("run filter"));

        assert!(!Filter::new("age == 31")
            .expect("parse filter")
            .matches(&obj)
            .expect("run filter"));

        assert!(Filter::new("\"red\" in tags")
            .expect("parse filter")
            .matches(&obj)
            .expect("run filter"));
    }

    #[test]
    fn test_string_comparisons() {
        let obj = TestObject {
            name: "John Doe".to_string(),
            age: 30,
            tags: vec!["red", "blue"],
        };

        assert!(Filter::new("name == \"john doe\"")
            .expect("parse filter")
            .matches(&obj)
            .expect("run filter"));

        assert!(Filter::new("name == \"John Doe\"")
            .expect("parse filter")
            .matches(&obj)
            .expect("run filter"));

        assert!(Filter::new("name == \"JOHN DOE\"")
            .expect("parse filter")
            .matches(&obj)
            .expect("run filter"));

        assert!(Filter::new("name contains \"John\"")
            .expect("parse filter")
            .matches(&obj)
            .expect("run filter"));

        assert!(Filter::new("name contains \"DOE\"")
            .expect("parse filter")
            .matches(&obj)
            .expect("run filter"));

        assert!(!Filter::new("name contains \"jane\"")
            .expect("parse filter")
            .matches(&obj)
            .expect("run filter"));

        assert!(Filter::new("\"John\" in name")
            .expect("parse filter")
            .matches(&obj)
            .expect("run filter"));

        assert!(Filter::new("\"DOE\" in name")
            .expect("parse filter")
            .matches(&obj)
            .expect("run filter"));

        assert!(!Filter::new("\"jane\" in name")
            .expect("parse filter")
            .matches(&obj)
            .expect("run filter"));

        assert!(Filter::new("\"red\" in tags")
            .expect("parse filter")
            .matches(&obj)
            .expect("run filter"));

        assert!(Filter::new("\"BLUE\" in tags")
            .expect("parse filter")
            .matches(&obj)
            .expect("run filter"));

        assert!(!Filter::new("\"green\" in tags")
            .expect("parse filter")
            .matches(&obj)
            .expect("run filter"));
    }
}
