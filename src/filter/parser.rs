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

        while matches!(self.tokens.peek(), Some(Ok(Token::Or(..)))) {
            let token = self.tokens.next().unwrap()?;
            let right = self.and()?;
            expr = Expr::Logical(Box::new(expr), token, Box::new(right));
        }

        Ok(expr)
    }

    fn and(&mut self) -> Result<Expr<'a>, Error> {
        let mut expr = self.equality()?;

        while matches!(self.tokens.peek(), Some(Ok(Token::And(..)))) {
            let token = self.tokens.next().unwrap()?;
            let right = self.equality()?;
            expr = Expr::Logical(Box::new(expr), token, Box::new(right));
        }

        Ok(expr)
    }

    fn equality(&mut self) -> Result<Expr<'a>, Error> {
        let mut expr = self.comparison()?;

        if matches!(
            self.tokens.peek(),
            Some(Ok(Token::Equals(..)) | Ok(Token::NotEquals(..)))
        ) {
            let token = self.tokens.next().unwrap().unwrap();
            let right = self.comparison()?;
            expr = Expr::Binary(Box::new(expr), token, Box::new(right));
        }

        Ok(expr)
    }

    fn comparison(&mut self) -> Result<Expr<'a>, Error> {
        let mut expr = self.unary()?;

        if matches!(
            self.tokens.peek(),
            Some(Ok(Token::In(..)))
                | Some(Ok(Token::Contains(..)))
                | Some(Ok(Token::StartsWith(..)))
                | Some(Ok(Token::EndsWith(..)))
        ) {
            let token = self.tokens.next().unwrap().unwrap();
            let right = self.unary()?;
            expr = Expr::Binary(Box::new(expr), token, Box::new(right));
        }

        Ok(expr)
    }

    fn unary(&mut self) -> Result<Expr<'a>, Error> {
        if matches!(self.tokens.peek(), Some(Ok(Token::Not(..)))) {
            let token = self.tokens.next().unwrap().unwrap();
            let right = self.unary()?;
            Ok(Expr::Unary(token, Box::new(right)))
        } else {
            self.primary()
        }
    }

    fn primary(&mut self) -> Result<Expr<'a>, Error> {
        match self.tokens.peek() {
            Some(Ok(Token::LeftParen(..))) => {
                self.tokens.next();
                let expr = self.or()?;
                if let Some(Ok(Token::RightParen(..))) = self.tokens.next() {
                    Ok(expr)
                } else {
                    Err(errors::user(
                        "When attempting to parse a grouped filter expression, we didn't find the closing ')' where we expected to.",
                        "Make sure that you have balanced your parentheses correctly.",
                    ))
                }
            }
            Some(Ok(Token::LeftBracket(..))) => {
              self.tokens.next();
                let mut items = Vec::new();
                while !matches!(self.tokens.peek(), Some(Ok(Token::RightBracket(..)))) {
                    items.push(self.literal()?);
                    if matches!(self.tokens.peek(), Some(Ok(Token::Comma(..)))) {
                        self.tokens.next();
                    } else {
                        break;
                    }
                }

                if let Some(Ok(Token::RightBracket(..))) = self.tokens.next() {
                  Ok(Expr::Literal(items.into()))
                } else {
                  Err(errors::user(
                      "When attempting to parse a list filter expression, we didn't find the closing ']' where we expected to.",
                      "Make sure that you have closed your tuple brackets correctly.",
                  ))
                }
            }
            Some(Ok(Token::Property(..))) => {
              if let Some(Ok(Token::Property(.., p))) = self.tokens.next() {
                Ok(Expr::Property(p))
              } else {
                unreachable!()
              }
            },
            Some(Ok(..)) => self.literal().map(Expr::Literal),
            Some(Err(..)) => Err(self.tokens.next().unwrap().unwrap_err()),
            None => Err(errors::user(
                "We reached the end of your filter expression while waiting for a [true, false, \"string\", number, (group), or property.name].",
                "Make sure that you have written a valid filter query and that you haven't forgotten part of it.",
            )),
        }
    }

    fn literal(&mut self) -> Result<FilterValue, Error> {
        match self.tokens.next() {
            Some(Ok(Token::True(..))) => Ok(true.into()),
            Some(Ok(Token::False(..))) => Ok(false.into()),
            Some(Ok(Token::Number(.., n))) => Ok(super::FilterValue::Number(n.parse().map_err(|e| errors::user_with_internal(
              "Failed to parse the number '{n}' which you provided.",
              "Please make sure that the number is well formatted. It should be in the form 123, or 123.45.",
              e,
            ))?)),
            Some(Ok(Token::String(.., s))) => Ok(s.replace("\\\"", "\"").replace("\\\\", "\\").into()),
            Some(Ok(Token::Null(..))) => Ok(super::FilterValue::Null),
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
    use rstest::rstest;

    use crate::filter::{location::Loc, FilterValue};

    use super::*;

    #[rstest]
    #[case("true", true.into())]
    #[case("false", false.into())]
    #[case("\"hello\"", "hello".into())]
    #[case("123", 123.0.into())]
    #[case("null", FilterValue::Null)]
    #[case("[]", FilterValue::Tuple(vec![]))]
    #[case("[true]", FilterValue::Tuple(vec![true.into()]))]
    #[case("[true, false, \"test\", 123, null]", FilterValue::Tuple(vec![true.into(), false.into(), "test".into(), 123.into(), FilterValue::Null]))]
    fn parsing_literals(#[case] input: &str, #[case] value: FilterValue) {
        let tokens = crate::filter::lexer::Scanner::new(input);
        match Parser::parse(tokens.into_iter()) {
            Ok(Expr::Literal(ast)) => assert_eq!(value, ast, "Expected {ast} to be {value}"),
            Ok(expr) => panic!("Expected a literal, got {:?}", expr),
            Err(e) => panic!("Error: {}", e),
        }
    }

    #[rstest]
    #[case("!true", Expr::Unary(Token::Not(Loc::new(1, 1)), Box::new(Expr::Literal(true.into()))))]
    #[case("!false", Expr::Unary(Token::Not(Loc::new(1, 1)), Box::new(Expr::Literal(false.into()))))]
    #[case("!\"hello\"", Expr::Unary(Token::Not(Loc::new(1, 1)), Box::new(Expr::Literal("hello".into()))))]
    fn parsing_unary_expressions(#[case] input: &str, #[case] ast: Expr) {
        let tokens = crate::filter::lexer::Scanner::new(input);
        match Parser::parse(tokens.into_iter()) {
            Ok(expr) => assert_eq!(ast, expr, "Expected {ast} to be {expr}"),
            Err(e) => panic!("Error: {}", e),
        }
    }

    #[rstest]
    #[case("true == false", Expr::Binary(Box::new(Expr::Literal(true.into())), Token::Equals(Loc::new(1, 6)), Box::new(Expr::Literal(false.into()))))]
    #[case("true != false", Expr::Binary(Box::new(Expr::Literal(true.into())), Token::NotEquals(Loc::new(1, 6)), Box::new(Expr::Literal(false.into()))))]
    #[case("\"xyz\" startswith \"x\"", Expr::Binary(Box::new(Expr::Literal("xyz".into())), Token::StartsWith(Loc::new(1, 7)), Box::new(Expr::Literal("x".into()))))]
    #[case("\"xyz\" endswith \"z\"", Expr::Binary(Box::new(Expr::Literal("xyz".into())), Token::EndsWith(Loc::new(1, 7)), Box::new(Expr::Literal("z".into()))))]
    fn parse_comparison_expressions(#[case] input: &str, #[case] ast: Expr) {
        let tokens = crate::filter::lexer::Scanner::new(input);
        match Parser::parse(tokens.into_iter()) {
            Ok(expr) => assert_eq!(ast, expr, "Expected {ast} to be {expr}"),
            Err(e) => panic!("Error: {}", e),
        }
    }

    #[rstest]
    #[case("true && false", Expr::Logical(Box::new(Expr::Literal(true.into())), Token::And(Loc::new(1, 6)), Box::new(Expr::Literal(false.into()))))]
    #[case("true || false", Expr::Logical(Box::new(Expr::Literal(true.into())), Token::Or(Loc::new(1, 6)), Box::new(Expr::Literal(false.into()))))]
    #[case("true && (true || false)", Expr::Logical(Box::new(Expr::Literal(true.into())), Token::And(Loc::new(1, 6)), Box::new(Expr::Logical(Box::new(Expr::Literal(true.into())), Token::Or(Loc::new(1, 15)), Box::new(Expr::Literal(false.into()))))))]
    fn parsing_logical_expressions(#[case] input: &str, #[case] ast: Expr) {
        let tokens = crate::filter::lexer::Scanner::new(input);
        match Parser::parse(tokens.into_iter()) {
            Ok(expr) => assert_eq!(ast, expr, "Expected {ast} to be {expr}"),
            Err(e) => panic!("Error: {}", e),
        }
    }
}
