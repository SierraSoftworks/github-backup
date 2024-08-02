mod expr;
mod lexer;
mod parser;
mod token;
mod value;

use expr::{Expr, ExprVisitor};
use token::Token;
pub use value::*;



pub struct Filter {
    ast: Expr,
}

impl Filter {
    pub fn new(filter: &str) -> Result<Self, crate::Error> {
        let tokens = crate::filter::lexer::Scanner::new(filter);
        let ast = crate::filter::parser::Parser::parse(tokens.into_iter())?;
        Ok(Self { ast })
    }

    pub fn matches<T: Filterable>(&self, target: &T) -> Result<bool, crate::Error> {
        let mut context = FilterContext { target };
        Ok(context.visit_expr(&self.ast).is_truthy())
    }
}

impl Default for Filter {
    fn default() -> Self {
        Self {
            ast: Expr::Literal(FilterValue::Bool(true)),
        }
    }
}

impl<'de> serde::Deserialize<'de> for Filter {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de> {
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
                        D: serde::Deserializer<'de>, {
                    deserializer.deserialize_str(self)
                }

                fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
                    where
                        E: serde::de::Error, {
                    Filter::new(v).map_err(serde::de::Error::custom)
                }
            }

        deserializer.deserialize_option(FilterVisitor)
    }
}

pub struct FilterContext<'a, T: Filterable> {
    target: &'a T,
}

impl<'a, T: Filterable> ExprVisitor<FilterValue> for FilterContext<'a, T> {
    fn visit_literal(&mut self, value: &FilterValue) -> FilterValue {
        value.clone()
    }

    fn visit_property(&mut self, name: &str) -> FilterValue {
        self.target.get(name).clone()
    }

    fn visit_binary(
        &mut self,
        left: &Expr,
        operator: &Token,
        right: &Expr,
    ) -> FilterValue {
        let left = self.visit_expr(left);
        match operator {
            Token::Equals => (left == self.visit_expr(right)).into(),
            Token::NotEquals => (left != self.visit_expr(right)).into(),
            Token::Contains => {
                if let FilterValue::String(left) = left {
                    let right = self.visit_expr(right);
                    if let FilterValue::String(right) = right {
                        left.to_lowercase().contains(&right.to_lowercase()).into()
                    } else {
                        false.into()
                    }
                } else {
                    false.into()
                }
            }
            token => unreachable!("Encountered an unexpected binary operator '{token}'"),
        }
    }

    fn visit_logical(
        &mut self,
        left: &Expr,
        operator: &Token,
        right: &Expr,
    ) -> FilterValue {
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
    }

    impl Filterable for TestObject {
        fn get(&self, property: &str) -> FilterValue {
            match property {
                "name" => self.name.clone().into(),
                "age" => self.age.into(),
                _ => FilterValue::Null,
            }
        }
    }

    #[test]
    fn test_filter() {
        let obj = TestObject {
            name: "John Doe".to_string(),
            age: 30,
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
    }

    #[test]
    fn test_string_comparisons() {
        let obj = TestObject {
            name: "John Doe".to_string(),
            age: 30,
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
    }
}
