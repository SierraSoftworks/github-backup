use std::fmt::Display;

use super::location::Loc;

#[derive(Debug, PartialEq)]
pub enum Token<'a> {
    LeftParen(Loc),
    RightParen(Loc),
    LeftBracket(Loc),
    RightBracket(Loc),
    Comma(Loc),

    Property(Loc, &'a str),

    Null(Loc),
    True(Loc),
    False(Loc),
    String(Loc, &'a str),
    Number(Loc, &'a str),

    Equals(Loc),
    NotEquals(Loc),
    Contains(Loc),
    In(Loc),
    StartsWith(Loc),
    EndsWith(Loc),

    Not(Loc),
    And(Loc),
    Or(Loc),
}

impl Token<'_> {
    pub fn lexeme(&self) -> &str {
        match self {
            Token::LeftParen(..) => "(",
            Token::RightParen(..) => ")",
            Token::LeftBracket(..) => "[",
            Token::RightBracket(..) => "]",
            Token::Comma(..) => ",",

            Token::Property(.., s) => s,

            Token::Null(..) => "null",
            Token::True(..) => "true",
            Token::False(..) => "false",
            Token::String(.., s) => s,
            Token::Number(.., s) => s,

            Token::Equals(..) => "==",
            Token::NotEquals(..) => "!=",
            Token::Contains(..) => "contains",
            Token::In(..) => "in",
            Token::StartsWith(..) => "startswith",
            Token::EndsWith(..) => "endswith",

            Token::Not(..) => "!",
            Token::And(..) => "&&",
            Token::Or(..) => "||",
        }
    }

    pub fn location(&self) -> Loc {
        match self {
            Token::LeftParen(loc) => *loc,
            Token::RightParen(loc) => *loc,
            Token::LeftBracket(loc) => *loc,
            Token::RightBracket(loc) => *loc,
            Token::Comma(loc) => *loc,

            Token::Property(loc, ..) => *loc,

            Token::Null(loc) => *loc,
            Token::True(loc) => *loc,
            Token::False(loc) => *loc,
            Token::String(loc, ..) => *loc,
            Token::Number(loc, ..) => *loc,

            Token::Equals(loc) => *loc,
            Token::NotEquals(loc) => *loc,
            Token::Contains(loc) => *loc,
            Token::In(loc) => *loc,
            Token::StartsWith(loc) => *loc,
            Token::EndsWith(loc) => *loc,

            Token::Not(loc) => *loc,
            Token::And(loc) => *loc,
            Token::Or(loc) => *loc,
        }
    }
}

impl Display for Token<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Token::String(s, ..) => write!(f, "\"{s}\""),
            t => write!(f, "{}", t.lexeme()),
        }
    }
}
