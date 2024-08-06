use std::fmt::{Debug, Display};

use serde::Deserialize;

#[allow(dead_code)]
#[derive(Default, Clone, Deserialize, PartialEq)]
pub enum Credentials {
    #[default]
    None,
    Token(String),
    UsernamePassword {
        username: String,
        password: String,
    },
}

impl Display for Credentials {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Credentials::None => write!(f, "No credentials"),
            Credentials::Token(..) => write!(f, "Token"),
            Credentials::UsernamePassword { .. } => write!(f, "Username+Password"),
        }
    }
}

impl Debug for Credentials {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Credentials::None => write!(f, "None"),
            Credentials::Token(..) => write!(f, "Token"),
            Credentials::UsernamePassword { .. } => write!(f, "UsernamePassword"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[rstest]
    #[case::none(Credentials::None, "No credentials")]
    #[case::token(Credentials::Token("token".to_string()), "Token")]
    #[case::username_password(Credentials::UsernamePassword { username: "admin".to_string(), password: "pass".to_string() }, "Username+Password")]
    fn test_display(#[case] credentials: Credentials, #[case] expected: &str) {
        assert_eq!(format!("{}", credentials), expected);
    }

    #[rstest]
    #[case::none(Credentials::None, "None")]
    #[case::token(Credentials::Token("token".to_string()), "Token")]
    #[case::username_password(Credentials::UsernamePassword { username: "admin".to_string(), password: "pass".to_string() }, "UsernamePassword")]
    fn test_debug(#[case] credentials: Credentials, #[case] expected: &str) {
        assert_eq!(format!("{:?}", credentials), expected);
    }
}
