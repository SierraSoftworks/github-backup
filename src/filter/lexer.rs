use crate::errors::{self, Error};

use super::{location::Loc, token::Token};

pub struct Scanner<'a> {
    source: &'a str,
    chars: std::iter::Peekable<std::str::CharIndices<'a>>,
    line: usize,
    line_start: usize,
}

impl<'a> Scanner<'a> {
    pub fn new(source: &'a str) -> Self {
        Self {
            source,
            chars: source.char_indices().peekable(),
            line: 1,
            line_start: 0,
        }
    }

    fn match_char(&mut self, next: char) -> bool {
        if let Some((idx, c)) = self.chars.peek() {
            if *c == '\n' {
                self.line += 1;
                self.line_start = *idx + 1;
            }

            if *c == next {
                self.chars.next();
                return true;
            }
        }

        false
    }

    fn advance_while_fn<F: Fn(usize, char) -> bool>(&mut self, f: F) -> usize {
        let mut length = 0;
        while let Some((idx, c)) = self.chars.peek() {
            if *c == '\n' {
                self.line += 1;
                self.line_start = *idx + 1;
            }

            if !f(*idx, *c) {
                break;
            }

            self.chars.next();
            length += 1;
        }

        length
    }

    fn read_string(&mut self, start: usize) -> Result<Token<'a>, Error> {
        let start_loc = Loc::new(self.line, 1 + start - self.line_start);
        while let Some((idx, c)) = self.chars.next() {
            match c {
                '\n' => {
                    self.line += 1;
                    self.line_start = idx + 1;
                }
                '"' => {
                    return Ok(Token::String(start_loc, &self.source[start + 1..idx]));
                }
                '\\' if self.match_char('"') => {}
                _ => {}
            }
        }

        Err(errors::user(
            &format!("Reached the end of the filter without finding the closing quote for a string starting at {}.", start_loc),
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

        Ok(Token::Number(
            Loc::new(self.line, 1 + start - self.line_start),
            &self.source[start..end + 1],
        ))
    }

    fn read_identifier(&mut self, start: usize) -> Result<Token<'a>, Error> {
        let end = start
            + self.advance_while_fn(|_, c| c.is_alphanumeric() || c == '_' || c == '.' || c == '-');
        let lexeme = &self.source[start..end + 1];
        let location = Loc::new(self.line, 1 + start - self.line_start);

        match lexeme {
            "false" => Ok(Token::False(location)),
            "null" => Ok(Token::Null(location)),
            "true" => Ok(Token::True(location)),
            "contains" => Ok(Token::Contains(location)),
            "in" => Ok(Token::In(location)),
            "startswith" => Ok(Token::StartsWith(location)),
            "endswith" => Ok(Token::EndsWith(location)),
            lexeme => Ok(Token::Property(location, lexeme)),
        }
    }
}

impl<'a> Iterator for Scanner<'a> {
    type Item = Result<Token<'a>, Error>;

    fn next(&mut self) -> Option<Self::Item> {
        while let Some((idx, c)) = self.chars.next() {
            match c {
                ' ' | '\t' => {}
                '\n' => {
                    self.line += 1;
                    self.line_start = idx + 1;
                }
                '(' => {
                    return Some(Ok(Token::LeftParen(Loc::new(
                        self.line,
                        1 + idx - self.line_start,
                    ))));
                }
                ')' => {
                    return Some(Ok(Token::RightParen(Loc::new(
                        self.line,
                        1 + idx - self.line_start,
                    ))));
                }
                '[' => {
                    return Some(Ok(Token::LeftBracket(Loc::new(
                        self.line,
                        1 + idx - self.line_start,
                    ))));
                }
                ']' => {
                    return Some(Ok(Token::RightBracket(Loc::new(
                        self.line,
                        1 + idx - self.line_start,
                    ))));
                }
                ',' => {
                    return Some(Ok(Token::Comma(Loc::new(
                        self.line,
                        1 + idx - self.line_start,
                    ))));
                }
                '&' => {
                    if self.match_char('&') {
                        return Some(Ok(Token::And(Loc::new(
                            self.line,
                            1 + idx - self.line_start,
                        ))));
                    } else {
                        return Some(Err(errors::user(
                          &format!("Filter included an orphaned '&' at {} which is not a valid operator.", Loc::new(self.line, 1 + idx - self.line_start)),
                          "Ensure that you are using the '&&' operator to implement a logical AND within your filter."
                        )));
                    }
                }
                '|' => {
                    if self.match_char('|') {
                        return Some(Ok(Token::Or(Loc::new(
                            self.line,
                            1 + idx - self.line_start,
                        ))));
                    } else {
                        return Some(Err(errors::user(
                          &format!("Filter included an orphaned '|' at {} which is not a valid operator.", Loc::new(self.line, 1 + idx - self.line_start)),
                          "Ensure that you are using the '||' operator to implement a logical OR within your filter."
                        )));
                    }
                }
                '=' => {
                    if self.match_char('=') {
                        return Some(Ok(Token::Equals(Loc::new(
                            self.line,
                            1 + idx - self.line_start,
                        ))));
                    } else {
                        return Some(Err(errors::user(
                          &format!("Filter included an orphaned '=' at {} which is not a valid operator.", Loc::new(self.line, 1 + idx - self.line_start)),
                          "Ensure that you are using the '==' operator to implement a logical equality within your filter."
                        )));
                    }
                }
                '!' => {
                    if self.match_char('=') {
                        return Some(Ok(Token::NotEquals(Loc::new(
                            self.line,
                            1 + idx - self.line_start,
                        ))));
                    } else {
                        return Some(Ok(Token::Not(Loc::new(
                            self.line,
                            1 + idx - self.line_start,
                        ))));
                    }
                }
                '>' => {
                    if self.match_char('=') {
                        return Some(Ok(Token::GreaterEqual(Loc::new(
                            self.line,
                            1 + idx - self.line_start,
                        ))));
                    } else {
                        return Some(Ok(Token::GreaterThan(Loc::new(
                            self.line,
                            idx - self.line_start,
                        ))));
                    }
                }
                '<' => {
                    if self.match_char('=') {
                        return Some(Ok(Token::SmallerEqual(Loc::new(
                            self.line,
                            1 + idx - self.line_start,
                        ))));
                    } else {
                        return Some(Ok(Token::SmallerThan(Loc::new(
                            self.line,
                            idx - self.line_start,
                        ))));
                    }
                }
                '"' => {
                    return Some(self.read_string(idx));
                }
                c if c.is_numeric() => {
                    return Some(self.read_number(idx));
                }
                _ => {
                    return Some(self.read_identifier(idx));
                }
            }
        }

        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    macro_rules! assert_sequence {
      ($filter:expr $(, $item:pat)* $(,)?) => {
        let mut scanner = Scanner::new($filter);
        $(
          match scanner.next() {
            Some(Ok($item)) => {},
            Some(Ok(item)) => panic!("Expected '{}' but got '{:?}'", stringify!($item), item),
            Some(Err(e)) => panic!("Error: {}", e),
            None => panic!("Expected '{}' but got the end of the parse sequence instead", stringify!($item)),
          }
        )*

        assert!(scanner.next().is_none(), "expected end of sequence, but got an item");
      };
    }

    #[test]
    fn test_empty() {
        assert_sequence!("");
    }

    #[test]
    fn test_whitespace() {
        assert_sequence!("  \t\n");
    }

    #[test]
    fn test_parens() {
        assert_sequence!(
            "() []",
            Token::LeftParen(..),
            Token::RightParen(..),
            Token::LeftBracket(..),
            Token::RightBracket(..),
        );
    }

    #[test]
    fn test_logical_operators() {
        assert_sequence!("&& ||", Token::And(..), Token::Or(..));
    }

    #[test]
    fn test_comparison_operators() {
        assert_sequence!(
            "== != contains in startswith endswith > >= < <=",
            Token::Equals(..),
            Token::NotEquals(..),
            Token::Contains(..),
            Token::In(..),
            Token::StartsWith(..),
            Token::EndsWith(..),
            Token::GreaterThan(..),
            Token::GreaterEqual(..),
            Token::SmallerThan(..),
            Token::SmallerEqual(..),
        );
    }

    #[test]
    fn test_string() {
        assert_sequence!("\"hello world\"", Token::String(.., "hello world"));

        assert_sequence!(
            "\"hello \\\"world\\\"\"",
            Token::String(.., "hello \\\"world\\\""),
        );
    }

    #[test]
    fn test_number() {
        assert_sequence!("123.456", Token::Number(.., "123.456"));
    }

    #[test]
    fn test_identifiers() {
        assert_sequence!(
            "true false null foo.bar-baz",
            Token::True(..),
            Token::False(..),
            Token::Null(..),
            Token::Property(.., "foo.bar-baz"),
        );
    }

    #[test]
    fn test_mixed() {
        assert_sequence!(
            "foo == \"bar\" && baz != 123",
            Token::Property(.., "foo"),
            Token::Equals(..),
            Token::String(.., "bar"),
            Token::And(..),
            Token::Property(.., "baz"),
            Token::NotEquals(..),
            Token::Number(.., "123"),
        );
    }

    #[test]
    fn test_negation() {
        assert_sequence!(
            "repo.public && !release.prerelease && !artifact.source-code",
            Token::Property(.., "repo.public"),
            Token::And(..),
            Token::Not(..),
            Token::Property(.., "release.prerelease"),
            Token::And(..),
            Token::Not(..),
            Token::Property(.., "artifact.source-code"),
        );
    }

    #[test]
    fn test_location() {
        assert_sequence!(
            "true !=\nfalse",
            Token::True(Loc { line: 1, column: 1 }),
            Token::NotEquals(Loc { line: 1, column: 6 }),
            Token::False(Loc { line: 2, column: 1 })
        );
    }
}
