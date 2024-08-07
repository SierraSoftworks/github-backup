use std::fmt::{Debug, Display};

use super::{token::Token, FilterValue};

// WARNING: We cannot have clone/copy semantics here because the [`Filter`] relies on
// pinning pointers to ensure that this struct can be safely used without additional
// allocations.
#[derive(PartialEq)]
pub enum Expr<'a> {
    Literal(FilterValue),
    Property(&'a str),
    Binary(Box<Expr<'a>>, Token<'a>, Box<Expr<'a>>),
    Logical(Box<Expr<'a>>, Token<'a>, Box<Expr<'a>>),
    Unary(Token<'a>, Box<Expr<'a>>),
}

pub trait ExprVisitor<T> {
    fn visit_expr(&mut self, expr: &Expr) -> T {
        match expr {
            Expr::Literal(value) => self.visit_literal(value),
            Expr::Property(name) => self.visit_property(name),
            Expr::Binary(left, operator, right) => self.visit_binary(left, operator, right),
            Expr::Logical(left, operator, right) => self.visit_logical(left, operator, right),
            Expr::Unary(operator, right) => self.visit_unary(operator, right),
        }
    }

    fn visit_literal(&mut self, value: &FilterValue) -> T;
    fn visit_property(&mut self, name: &str) -> T;
    fn visit_binary(&mut self, left: &Expr, operator: &Token, right: &Expr) -> T;
    fn visit_logical(&mut self, left: &Expr, operator: &Token, right: &Expr) -> T;
    fn visit_unary(&mut self, operator: &Token, right: &Expr) -> T;
}

impl Display for Expr<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut printer = ExprPrinter(f);
        printer.visit_expr(self)?;
        Ok(())
    }
}

impl Debug for Expr<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut printer = ExprPrinter(f);
        printer.visit_expr(self)?;
        Ok(())
    }
}

struct ExprPrinter<'a, 'b>(&'a mut std::fmt::Formatter<'b>);
impl<'a, 'b> ExprVisitor<std::fmt::Result> for ExprPrinter<'a, 'b> {
    fn visit_literal(&mut self, value: &FilterValue) -> std::fmt::Result {
        write!(self.0, "{}", value)
    }

    fn visit_property(&mut self, name: &str) -> std::fmt::Result {
        write!(self.0, "(property {})", name)
    }

    fn visit_binary(&mut self, left: &Expr, operator: &Token, right: &Expr) -> std::fmt::Result {
        write!(self.0, "({operator} ")?;
        self.visit_expr(left)?;
        write!(self.0, " ")?;
        self.visit_expr(right)?;
        write!(self.0, ")")
    }

    fn visit_logical(&mut self, left: &Expr, operator: &Token, right: &Expr) -> std::fmt::Result {
        write!(self.0, "({operator} ")?;
        self.visit_expr(left)?;
        write!(self.0, " ")?;
        self.visit_expr(right)?;
        write!(self.0, ")")
    }

    fn visit_unary(&mut self, operator: &Token, right: &Expr) -> std::fmt::Result {
        write!(self.0, "{}", operator.lexeme())?;
        self.visit_expr(right)
    }
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use crate::filter::location::Loc;

    use super::*;

    #[rstest]
    #[case(Expr::Literal("value".into()), "\"value\"")]
    #[case(Expr::Property("test"), "(property test)")]
    #[case(
        Expr::Binary(
            Box::new(Expr::Literal("value".into())),
            Token::In(Loc::new(1, 8)),
            Box::new(Expr::Property("test")),
        ),
        "(in \"value\" (property test))"
    )]
    #[case(
        Expr::Logical(
            Box::new(Expr::Literal("value".into())),
            Token::And(Loc::new(1, 8)),
            Box::new(Expr::Property("test")),
        ),
        "(&& \"value\" (property test))"
    )]
    fn expression_visualization(#[case] expr: Expr<'_>, #[case] view: &str) {
        assert_eq!(view, format!("{expr}"));
    }
}
