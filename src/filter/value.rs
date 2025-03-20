use std::cmp::Ordering;
use std::fmt::{Debug, Display};

/// A trait for types which can be filtered by the filter system.
///
/// Types which implement this trait can be filtered through the use
/// of filter DSL expressions. A filter expression might look something
/// like the following:
///
/// ```
/// repo.public && !repo.fork && repo.name in ["git-tool", "grey"]
/// ```
///
/// In this case, the [`Filter`] would call [`Filterable::get`] with the
/// property keys it intends to retrieve, in thise case: `repo.public`,
/// `repo.fork`, and `repo.name`. The [`Filterable`] implementation would
/// then return the appropriate [`FilterValue`] for each key.
pub trait Filterable {
    /// Retrieve the value of a property key.
    ///
    /// This method should return the value of the property key as it
    /// pertains to the filterable object. If the key is not present,
    /// the method should return a [`FilterValue::Null`] value. The
    /// [`crate::filter::NULL`] constant is provided for this purpose.
    fn get(&self, key: &str) -> FilterValue;
}

/// A value describing the
#[derive(Clone, Default)]
pub enum FilterValue {
    #[default]
    Null,
    Bool(bool),
    Number(f64),
    String(String),
    Tuple(Vec<FilterValue>),
}

impl FilterValue {
    pub fn is_truthy(&self) -> bool {
        match self {
            FilterValue::Null => false,
            FilterValue::Bool(b) => *b,
            FilterValue::Number(n) => *n != 0.0,
            FilterValue::String(s) => !s.is_empty(),
            FilterValue::Tuple(v) => !v.is_empty(),
        }
    }

    pub fn contains(&self, other: &FilterValue) -> bool {
        match (self, other) {
            (FilterValue::Tuple(a), b) => a.iter().any(|ai| ai == b),
            (FilterValue::String(a), FilterValue::String(b)) => {
                a.to_lowercase().contains(&b.to_lowercase())
            }
            _ => false,
        }
    }

    pub fn startswith(&self, other: &FilterValue) -> bool {
        match (self, other) {
            (FilterValue::Tuple(a), b) => a.iter().any(|ai| ai == b),
            (FilterValue::String(a), FilterValue::String(b)) => {
                a.to_lowercase().starts_with(&b.to_lowercase())
            }
            _ => false,
        }
    }

    pub fn endswith(&self, other: &FilterValue) -> bool {
        match (self, other) {
            (FilterValue::Tuple(a), b) => a.iter().any(|ai| ai == b),
            (FilterValue::String(a), FilterValue::String(b)) => {
                a.to_lowercase().ends_with(&b.to_lowercase())
            }
            _ => false,
        }
    }
}

impl PartialEq for FilterValue {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (FilterValue::Null, FilterValue::Null) => true,
            (FilterValue::Bool(a), FilterValue::Bool(b)) => a == b,
            (FilterValue::Number(a), FilterValue::Number(b)) => a == b,
            (FilterValue::String(a), FilterValue::String(b)) => a.eq_ignore_ascii_case(b),
            (FilterValue::Tuple(a), FilterValue::Tuple(b)) => {
                a.len() == b.len() && a.iter().zip(b.iter()).all(|(a, b)| a == b)
            }
            _ => false,
        }
    }
}

impl PartialOrd for FilterValue {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        match (self, other) {
            (FilterValue::Null, FilterValue::Null) => Some(Ordering::Equal),
            (FilterValue::Bool(a), FilterValue::Bool(b)) => a.partial_cmp(b),
            (FilterValue::Number(a), FilterValue::Number(b)) => a.partial_cmp(b),
            (FilterValue::String(a), FilterValue::String(b)) => a.partial_cmp(b),
            (FilterValue::Tuple(a), FilterValue::Tuple(b)) => {
                if a.len() != b.len() {
                    a.len().partial_cmp(&b.len())
                } else {
                    a.iter()
                        .zip(b.iter())
                        .map(|(x, y)| x.partial_cmp(y))
                        .find(|&cmp| cmp != Some(Ordering::Equal))
                        .unwrap_or(Some(Ordering::Equal))
                }
            }
            _ => None, // Return None for non-comparable types
        }
    }

    fn lt(&self, other: &Self) -> bool {
        match (self, other) {
            (FilterValue::Null, FilterValue::Null) => true,
            (FilterValue::Bool(a), FilterValue::Bool(b)) => a < b,
            (FilterValue::Number(a), FilterValue::Number(b)) => a < b,
            (FilterValue::String(a), FilterValue::String(b)) => a < b,
            (FilterValue::Tuple(a), FilterValue::Tuple(b)) => {
                a.len() <= b.len() && a.iter().zip(b.iter()).all(|(a, b)| a < b)
            }
            _ => false,
        }
    }

    fn le(&self, other: &Self) -> bool {
        match (self, other) {
            (FilterValue::Null, FilterValue::Null) => true,
            (FilterValue::Bool(a), FilterValue::Bool(b)) => a <= b,
            (FilterValue::Number(a), FilterValue::Number(b)) => a <= b,
            (FilterValue::String(a), FilterValue::String(b)) => a <= b,
            (FilterValue::Tuple(a), FilterValue::Tuple(b)) => {
                a.len() <= b.len() && a.iter().zip(b.iter()).all(|(a, b)| a <= b)
            }
            _ => false,
        }
    }

    fn gt(&self, other: &Self) -> bool {
        match (self, other) {
            (FilterValue::Null, FilterValue::Null) => true,
            (FilterValue::Bool(a), FilterValue::Bool(b)) => a > b,
            (FilterValue::Number(a), FilterValue::Number(b)) => a > b,
            (FilterValue::String(a), FilterValue::String(b)) => a > b,
            (FilterValue::Tuple(a), FilterValue::Tuple(b)) => {
                a.len() >= b.len() && a.iter().zip(b.iter()).all(|(a, b)| a > b)
            }
            _ => false,
        }
    }

    fn ge(&self, other: &Self) -> bool {
        match (self, other) {
            (FilterValue::Null, FilterValue::Null) => true,
            (FilterValue::Bool(a), FilterValue::Bool(b)) => a >= b,
            (FilterValue::Number(a), FilterValue::Number(b)) => a >= b,
            (FilterValue::String(a), FilterValue::String(b)) => a >= b,
            (FilterValue::Tuple(a), FilterValue::Tuple(b)) => {
                a.len() >= b.len() && a.iter().zip(b.iter()).all(|(a, b)| a >= b)
            }
            _ => false,
        }
    }
}

impl Display for FilterValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FilterValue::Null => write!(f, "null"),
            FilterValue::Bool(b) => write!(f, "{}", b),
            FilterValue::Number(n) => write!(f, "{}", n),
            FilterValue::String(s) => {
                write!(f, "\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""))
            }
            FilterValue::Tuple(v) => {
                write!(f, "[")?;
                for (i, value) in v.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}", value)?;
                }
                write!(f, "]")
            }
        }
    }
}

impl Debug for FilterValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self)
    }
}

impl From<bool> for FilterValue {
    fn from(b: bool) -> Self {
        FilterValue::Bool(b)
    }
}

macro_rules! number {
    ($t:ty) => {
        impl From<$t> for FilterValue {
            fn from(n: $t) -> Self {
                FilterValue::Number(n as f64)
            }
        }
    };
}

number!(i8);
number!(u8);
number!(i16);
number!(u16);
number!(f32);
number!(i32);
number!(u32);
number!(f64);
number!(i64);
number!(u64);

impl From<&str> for FilterValue {
    fn from(s: &str) -> Self {
        FilterValue::String(s.to_string())
    }
}

impl From<String> for FilterValue {
    fn from(s: String) -> Self {
        FilterValue::String(s)
    }
}

impl<T> From<Option<T>> for FilterValue
where
    T: Into<FilterValue>,
{
    fn from(o: Option<T>) -> Self {
        o.map_or(FilterValue::Null, Into::into)
    }
}

impl From<Vec<FilterValue>> for FilterValue {
    fn from(v: Vec<FilterValue>) -> Self {
        FilterValue::Tuple(v)
    }
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::*;

    #[rstest]
    #[case(FilterValue::Null, false)]
    #[case(FilterValue::Bool(false), false)]
    #[case(FilterValue::Bool(true), true)]
    #[case(FilterValue::Number(0.0), false)]
    #[case(FilterValue::Number(1.0), true)]
    #[case(FilterValue::String("".to_string()), false)]
    #[case(FilterValue::String("hello".to_string()), true)]
    #[case(FilterValue::Tuple(vec![]), false)]
    #[case(FilterValue::Tuple(vec![FilterValue::Bool(true)]), true)]
    fn test_truthy<V: Into<FilterValue>>(#[case] value: V, #[case] truthy: bool) {
        assert_eq!(value.into().is_truthy(), truthy);
    }

    #[test]
    fn test_bool_comparison() {
        assert!(FilterValue::Bool(false) < FilterValue::Bool(true));
        assert!(FilterValue::Bool(true) > FilterValue::Bool(false));
        assert_eq!(FilterValue::Bool(true), FilterValue::Bool(true));
        assert_eq!(FilterValue::Bool(false), FilterValue::Bool(false));
    }

    #[test]
    fn test_number_comparison() {
        assert!(FilterValue::Number(1.0) < FilterValue::Number(2.0));
        assert!(FilterValue::Number(2.0) > FilterValue::Number(1.0));
        assert_eq!(FilterValue::Number(2.0), FilterValue::Number(2.0));
    }

    #[test]
    fn test_string_comparison() {
        assert!(
            FilterValue::String(String::from("abc")) < FilterValue::String(String::from("xyz"))
        );
        assert!(
            FilterValue::String(String::from("xyz")) > FilterValue::String(String::from("abc"))
        );
        assert_eq!(
            FilterValue::String(String::from("abc")),
            FilterValue::String(String::from("abc"))
        );
    }
}
