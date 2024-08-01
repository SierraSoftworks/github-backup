use std::fmt::{Debug, Display};

use serde::Deserialize;

#[allow(dead_code)]
#[derive(Default, Clone, Deserialize)]
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
