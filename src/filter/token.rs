use std::fmt::Display;

#[derive(Debug, PartialEq)]
pub enum Token {
    LeftParen,
    RightParen,

    Property(String),

    Null,
    True,
    False,
    String(String),
    Number(String),

    Equals,
    NotEquals,
    Contains,

    Not,
    And,
    Or,
}

impl Token {
    pub fn lexeme(&self) -> &str {
        match self {
            Token::LeftParen => "(",
            Token::RightParen => ")",
            Token::Property(s) => s,

            Token::Null => "null",
            Token::True => "true",
            Token::False => "false",
            Token::String(s) => s,
            Token::Number(s) => s,

            Token::Equals => "==",
            Token::NotEquals => "!=",
            Token::Contains => "contains",

            Token::Not => "!",
            Token::And => "&&",
            Token::Or => "||",
        }
    }
}

impl Display for Token {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Token::String(s) => write!(f, "\"{s}\""),
            t => write!(f, "{}", t.lexeme()),
        }
    }
}
