use std::iter::Peekable;

use crate::errors::{self, Error};

use super::{expr::Expr, token::Token, FilterValue};

pub struct Parser<'a, I: Iterator<Item = Result<Token<'a>, Error>>> {
    tokens: Peekable<I>,
}

impl<'a, I: Iterator<Item = Result<Token<'a>, Error>>> Parser<'a, I> {
    pub fn parse(tokens: I) -> Result<Expr<'a>, Error> {
        let mut parser = Parser {
            tokens: tokens.peekable(),
        };

        let expr = parser.or()?;
        parser.ensure_end()?;

        Ok(expr)
    }

    fn ensure_end(&mut self) -> Result<(), Error> {
        if let Some(result) = self.tokens.next() {
            let token = result?;
            Err(errors::user(
                &format!(
                    "Your filter expression contained an unexpected '{}'.",
                    token
                ),
                "Make sure that you have written a valid filter query.",
            ))
        } else {
            Ok(())
        }
    }

    fn or(&mut self) -> Result<Expr<'a>, Error> {
        let mut expr = self.and()?;

        while matches!(self.tokens.peek(), Some(Ok(Token::Or))) {
            let right = self.and()?;
            expr = Expr::Logical(Box::new(expr), Token::Or, Box::new(right));
        }

        Ok(expr)
    }

    fn and(&mut self) -> Result<Expr<'a>, Error> {
        let mut expr = self.equality()?;

        while matches!(self.tokens.peek(), Some(Ok(Token::And))) {
            let right = self.equality()?;
            expr = Expr::Logical(Box::new(expr), Token::And, Box::new(right));
        }

        Ok(expr)
    }

    fn equality(&mut self) -> Result<Expr<'a>, Error> {
        let mut expr = self.comparison()?;

        if matches!(
            self.tokens.peek(),
            Some(Ok(Token::Equals) | Ok(Token::NotEquals))
        ) {
            let token = self.tokens.next().unwrap().unwrap();
            let right = self.comparison()?;
            expr = Expr::Binary(Box::new(expr), token, Box::new(right));
        }

        Ok(expr)
    }

    fn comparison(&mut self) -> Result<Expr<'a>, Error> {
        let mut expr = self.unary()?;

        if matches!(self.tokens.peek(), Some(Ok(Token::Contains))) {
            let token = self.tokens.next().unwrap().unwrap();
            let right = self.unary()?;
            expr = Expr::Binary(Box::new(expr), token, Box::new(right));
        }

        Ok(expr)
    }

    fn unary(&mut self) -> Result<Expr<'a>, Error> {
        if matches!(self.tokens.peek(), Some(Ok(Token::Not))) {
            let token = self.tokens.next().unwrap().unwrap();
            let right = self.unary()?;
            Ok(Expr::Unary(token, Box::new(right)))
        } else {
            self.primary()
        }
    }

    fn primary(&mut self) -> Result<Expr<'a>, Error> {
        match self.tokens.peek() {
            Some(Ok(Token::LeftParen)) => {
                self.tokens.next();
                let expr = self.or()?;
                if let Some(Ok(Token::RightParen)) = self.tokens.next() {
                    Ok(expr)
                } else {
                    Err(errors::user(
                        "When attempting to parse a grouped filter expression, we didn't find the closing ')' where we expected to.",
                        "Make sure that you have balanced your parentheses correctly.",
                    ))
                }
            }
            Some(Ok(Token::LeftBracket)) => {
              self.tokens.next();
                let mut items = Vec::new();
                while !matches!(self.tokens.peek(), Some(Ok(Token::RightBracket))) {
                    items.push(self.literal()?);
                    if matches!(self.tokens.peek(), Some(Ok(Token::Comma))) {
                        self.tokens.next();
                    } else {
                        break;
                    }
                }

                if let Some(Ok(Token::RightBracket)) = self.tokens.next() {
                  Ok(Expr::Literal(items.into()))
                } else {
                  Err(errors::user(
                      "When attempting to parse a list filter expression, we didn't find the closing ']' where we expected to.",
                      "Make sure that you have closed your tuple brackets correctly.",
                  ))
                }
            }
            Some(Ok(Token::Property(..))) => {
              if let Some(Ok(Token::Property(p))) = self.tokens.next() {
                Ok(Expr::Property(p))
              } else {
                unreachable!()
              }
            },
            Some(Ok(..)) => self.literal().map(|l| Expr::Literal(l)),
            Some(Err(..)) => Err(self.tokens.next().unwrap().unwrap_err()),
            None => Err(errors::user(
                "We reached the end of your filter expression while waiting for a [true, false, \"string\", number, (group), or property.name].",
                "Make sure that you have written a valid filter query and that you haven't forgotten part of it.",
            )),
        }
    }

    fn literal(&mut self) -> Result<FilterValue, Error> {
        match self.tokens.next() {
            Some(Ok(Token::True)) => Ok(true.into()),
            Some(Ok(Token::False)) => Ok(false.into()),
            Some(Ok(Token::Number(n))) => Ok(super::FilterValue::Number(n.parse().map_err(|e| errors::user_with_internal(
              "Failed to parse the number '{n}' which you provided.",
              "Please make sure that the number is well formatted. It should be in the form 123, or 123.45.",
              e,
            ))?)),
            Some(Ok(Token::String(s))) => Ok(s.replace("\\\"", "\"").replace("\\\\", "\\").into()),
            Some(Ok(Token::Null)) => Ok(super::FilterValue::Null),
            Some(Ok(token)) => Err(errors::user(
                &format!("While parsing your filter, we found an unexpected '{}'.", token),
                "Make sure that you have written a valid filter query.",
            )),
            Some(Err(err)) => Err(err),
            None => Err(errors::user(
                "We reached the end of your filter expression while waiting for a [true, false, \"string\", number, (group), or property.name].",
                "Make sure that you have written a valid filter query and that you haven't forgotten part of it.",
            )),
          }
    }
}

#[cfg(test)]
mod tests {
    use crate::filter::FilterValue;

    use super::*;

    fn assert_ast(filter: &str, tree: Expr) {
        let tokens = crate::filter::lexer::Scanner::new(filter);
        match Parser::parse(tokens.into_iter()) {
            Ok(ast) => assert_eq!(tree, ast, "Expected {ast} to be {tree}"),
            Err(e) => panic!("Error: {}", e),
        }
    }

    #[test]
    fn test_literals() {
        assert_ast("true", Expr::Literal(true.into()));
        assert_ast("false", Expr::Literal(false.into()));
        assert_ast("\"hello\"", Expr::Literal("hello".into()));
        assert_ast("123", Expr::Literal(123.0.into()));
        assert_ast("null", Expr::Literal(FilterValue::Null));
    }

    #[test]
    fn test_tuples() {
        assert_ast("[]", Expr::Literal(vec![].into()));

        assert_ast(
            "[true, false, \"hello\", 123, null]",
            Expr::Literal(
                vec![
                    true.into(),
                    false.into(),
                    "hello".into(),
                    123.0.into(),
                    FilterValue::Null,
                ]
                .into(),
            ),
        );
    }
}
