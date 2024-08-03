use crate::errors::{self, Error};

use super::token::Token;

pub struct Scanner<'a> {
    source: &'a str,
    chars: std::iter::Peekable<std::str::CharIndices<'a>>,
}

impl<'a> Scanner<'a> {
    pub fn new(source: &'a str) -> Self {
        Self {
            source,
            chars: source.char_indices().peekable(),
        }
    }

    fn match_char(&mut self, next: char) -> bool {
        if let Some((_, c)) = self.chars.peek() {
            if *c == next {
                self.chars.next();
                return true;
            }
        }

        false
    }

    fn advance_while_fn<F: Fn(usize, char) -> bool>(&mut self, f: F) -> usize {
        let mut length = 0;
        while let Some((loc, c)) = self.chars.peek() {
            if !f(*loc, *c) {
                break;
            }

            self.chars.next();
            length += 1;
        }

        length
    }

    fn read_string(&mut self, start: usize) -> Result<Token<'a>, Error> {
        while let Some((loc, c)) = self.chars.next() {
            match c {
                '"' => {
                    return Ok(Token::String(&self.source[start + 1..loc]));
                }
                '\\' if self.match_char('"') => {}
                _ => {}
            }
        }

        Err(errors::user(
            "Reached the end of the filter without finding the closing quote for a string.",
            "Make sure that you have terminated your string with a '\"' character.",
        ))
    }

    fn read_number(&mut self, start: usize) -> Result<Token<'a>, Error> {
        let mut end = start + self.advance_while_fn(|_, c| c.is_numeric());
        if let Some((loc, c)) = self.chars.peek() {
            if *c == '.'
                && self
                    .source
                    .chars()
                    .nth(loc + 1)
                    .map(|c2| c2.is_numeric())
                    .unwrap_or_default()
            {
                self.chars.next();
                end += 1 + self.advance_while_fn(|_, c| c.is_numeric());
            }
        }

        Ok(Token::Number(&self.source[start..end + 1]))
    }

    fn read_identifier(&mut self, start: usize) -> Result<Token<'a>, Error> {
        let end = start
            + self.advance_while_fn(|_, c| c.is_alphanumeric() || c == '_' || c == '.' || c == '-');
        let lexeme = &self.source[start..end + 1];

        match lexeme {
            "false" => Ok(Token::False),
            "null" => Ok(Token::Null),
            "true" => Ok(Token::True),
            "contains" => Ok(Token::Contains),
            "in" => Ok(Token::In),
            lexeme => Ok(Token::Property(lexeme)),
        }
    }
}

impl<'a> Iterator for Scanner<'a> {
    type Item = Result<Token<'a>, Error>;

    fn next(&mut self) -> Option<Self::Item> {
        while let Some((loc, c)) = self.chars.next() {
            match c {
                ' ' | '\t' | '\n' => {}
                '(' => {
                    return Some(Ok(Token::LeftParen));
                }
                ')' => {
                    return Some(Ok(Token::RightParen));
                }
                '[' => {
                    return Some(Ok(Token::LeftBracket));
                }
                ']' => {
                    return Some(Ok(Token::RightBracket));
                }
                ',' => {
                    return Some(Ok(Token::Comma));
                }
                '&' => {
                    if self.match_char('&') {
                        return Some(Ok(Token::And));
                    } else {
                        return Some(Err(errors::user(
                          "Filter included an orphaned '&' which is not a valid operator.",
                          "Ensure that you are using the '&&' operator to implement a logical AND within your filter."
                        )));
                    }
                }
                '|' => {
                    if self.match_char('|') {
                        return Some(Ok(Token::Or));
                    } else {
                        return Some(Err(errors::user(
                          "Filter included an orphaned '|' which is not a valid operator.",
                          "Ensure that you are using the '||' operator to implement a logical OR within your filter."
                        )));
                    }
                }
                '=' => {
                    if self.match_char('=') {
                        return Some(Ok(Token::Equals));
                    } else {
                        return Some(Err(errors::user(
                          "Filter included an orphaned '=' which is not a valid operator.",
                          "Ensure that you are using the '==' operator to implement a logical equality within your filter."
                        )));
                    }
                }
                '!' => {
                    if self.match_char('=') {
                        return Some(Ok(Token::NotEquals));
                    } else {
                        return Some(Ok(Token::Not));
                    }
                }
                '"' => {
                    return Some(self.read_string(loc));
                }
                c if c.is_numeric() => {
                    return Some(self.read_number(loc));
                }
                _ => {
                    return Some(self.read_identifier(loc));
                }
            }
        }

        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_sequence(filter: &str, tokens: &[Token]) {
        let mut scanner = Scanner::new(filter);
        for token in tokens {
            match scanner.next() {
                Some(Ok(lexed)) => assert_eq!(lexed, *token),
                Some(Err(e)) => panic!("Error: {}", e),
                None => panic!(
                    "Expected '{}' but got the end of the parse sequence instead",
                    token
                ),
            }
        }
        assert!(scanner.next().is_none());
    }

    #[test]
    fn test_empty() {
        assert_sequence("", &[]);
    }

    #[test]
    fn test_whitespace() {
        assert_sequence("  \t\n", &[]);
    }

    #[test]
    fn test_parens() {
        assert_sequence(
            "() []",
            &[
                Token::LeftParen,
                Token::RightParen,
                Token::LeftBracket,
                Token::RightBracket,
            ],
        );
    }

    #[test]
    fn test_logical_operators() {
        assert_sequence("&& ||", &[Token::And, Token::Or]);
    }

    #[test]
    fn test_comparison_operators() {
        assert_sequence(
            "== != contains in",
            &[Token::Equals, Token::NotEquals, Token::Contains, Token::In],
        );
    }

    #[test]
    fn test_string() {
        assert_sequence("\"hello world\"", &[Token::String("hello world")]);

        assert_sequence(
            "\"hello \\\"world\\\"\"",
            &[Token::String("hello \\\"world\\\"")],
        );
    }

    #[test]
    fn test_number() {
        assert_sequence("123.456", &[Token::Number("123.456")]);
    }

    #[test]
    fn test_identifiers() {
        assert_sequence(
            "true false null foo.bar-baz",
            &[
                Token::True,
                Token::False,
                Token::Null,
                Token::Property("foo.bar-baz"),
            ],
        );
    }

    #[test]
    fn test_mixed() {
        assert_sequence(
            "foo == \"bar\" && baz != 123",
            &[
                Token::Property("foo"),
                Token::Equals,
                Token::String("bar"),
                Token::And,
                Token::Property("baz"),
                Token::NotEquals,
                Token::Number("123"),
            ],
        );
    }
}
