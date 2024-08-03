use std::fmt::Display;

#[derive(Debug, PartialEq)]
pub enum Token<'a> {
    LeftParen,
    RightParen,
    LeftBracket,
    RightBracket,
    Comma,

    Property(&'a str),

    Null,
    True,
    False,
    String(&'a str),
    Number(&'a str),

    Equals,
    NotEquals,
    Contains,
    In,
    StartsWith,
    EndsWith,

    Not,
    And,
    Or,
}

impl Token<'_> {
    pub fn lexeme(&self) -> &str {
        match self {
            Token::LeftParen => "(",
            Token::RightParen => ")",
            Token::LeftBracket => "[",
            Token::RightBracket => "]",
            Token::Comma => ",",

            Token::Property(s) => s,

            Token::Null => "null",
            Token::True => "true",
            Token::False => "false",
            Token::String(s) => s,
            Token::Number(s) => s,

            Token::Equals => "==",
            Token::NotEquals => "!=",
            Token::Contains => "contains",
            Token::In => "in",
            Token::StartsWith => "startswith",
            Token::EndsWith => "endswith",

            Token::Not => "!",
            Token::And => "&&",
            Token::Or => "||",
        }
    }
}

impl Display for Token<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Token::String(s) => write!(f, "\"{s}\""),
            t => write!(f, "{}", t.lexeme()),
        }
    }
}
