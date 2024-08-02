use std::iter::Peekable;

use crate::errors::{self, Error};

use super::{expr::Expr, token::Token};

pub struct Parser<I: Iterator<Item = Result<Token, Error>>> {
    tokens: Peekable<I>,
}

impl<I: Iterator<Item = Result<Token, Error>>> Parser<I> {
    pub fn parse(tokens: I) -> Result<Expr, Error> {
        let mut parser = Parser {
            tokens: tokens.peekable(),
        };

        let expr = parser.or()?;
        parser.ensure_end()?;

        Ok(expr)
    }

    fn ensure_end(&mut self) -> Result<(), Error> {
        if self.tokens.peek().is_some() {
            Err(errors::user(
                "Your filter expression contained an unexpected '{}'.",
                "Make sure that you have written a valid filter query.",
            ))
        } else {
            Ok(())
        }
    }

    fn or(&mut self) -> Result<Expr, Error> {
        let mut expr = self.and()?;

        while matches!(self.tokens.peek(), Some(Ok(Token::Or))) {
            let right = self.and()?;
            expr = Expr::Logical(Box::new(expr), Token::Or, Box::new(right));
        }

        Ok(expr)
    }

    fn and(&mut self) -> Result<Expr, Error> {
        let mut expr = self.equality()?;

        while matches!(self.tokens.peek(), Some(Ok(Token::And))) {
            let right = self.equality()?;
            expr = Expr::Logical(Box::new(expr), Token::And, Box::new(right));
        }

        Ok(expr)
    }

    fn equality(&mut self) -> Result<Expr, Error> {
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

    fn comparison(&mut self) -> Result<Expr, Error> {
        let mut expr = self.unary()?;

        if matches!(self.tokens.peek(), Some(Ok(Token::Contains))) {
            let token = self.tokens.next().unwrap().unwrap();
            let right = self.unary()?;
            expr = Expr::Binary(Box::new(expr), token, Box::new(right));
        }

        Ok(expr)
    }

    fn unary(&mut self) -> Result<Expr, Error> {
        if matches!(self.tokens.peek(), Some(Ok(Token::Not))) {
            let token = self.tokens.next().unwrap().unwrap();
            let right = self.unary()?;
            Ok(Expr::Unary(token, Box::new(right)))
        } else {
            self.primary()
        }
    }

    fn primary(&mut self) -> Result<Expr, Error> {
        match self.tokens.next() {
            Some(Ok(Token::True)) => Ok(Expr::Literal(true.into())),
            Some(Ok(Token::False)) => Ok(Expr::Literal(false.into())),
            Some(Ok(Token::Number(n))) => Ok(Expr::Literal(super::FilterValue::Number(n.parse().map_err(|e| errors::user_with_internal(
              "Failed to parse the number '{n}' which you provided.",
              "Please make sure that the number is well formatted. It should be in the form 123, or 123.45.",
              e,
            ))?))),
            Some(Ok(Token::String(s))) => Ok(Expr::Literal(s.replace("\\\"", "\"").replace("\\\\", "\\").into())),
            Some(Ok(Token::Null)) => Ok(Expr::Literal(super::FilterValue::Null)),
            Some(Ok(Token::LeftParen)) => {
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
            Some(Ok(Token::Property(p))) => Ok(Expr::Property(p)),
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
}
