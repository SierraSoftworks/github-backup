use super::{
    expr::{Expr, ExprVisitor},
    token::Token,
    FilterValue, Filterable,
};

pub struct FilterContext<'a, T: Filterable> {
    target: &'a T,
}

impl<'a, T: Filterable> FilterContext<'a, T> {
    pub fn new(target: &'a T) -> Self {
        Self { target }
    }
}

impl<T: Filterable> ExprVisitor<FilterValue> for FilterContext<'_, T> {
    fn visit_literal(&mut self, value: &FilterValue) -> FilterValue {
        value.clone()
    }

    fn visit_property(&mut self, name: &str) -> FilterValue {
        self.target.get(name).clone()
    }

    fn visit_binary(&mut self, left: &Expr, operator: &Token, right: &Expr) -> FilterValue {
        let left = self.visit_expr(left);
        let right = self.visit_expr(right);
        match operator {
            Token::Equals(..) => (left == right).into(),
            Token::NotEquals(..) => (left != right).into(),
            Token::Contains(..) => left.contains(&right).into(),
            Token::In(..) => right.contains(&left).into(),
            Token::StartsWith(..) => left.startswith(&right).into(),
            Token::EndsWith(..) => left.endswith(&right).into(),
            Token::GreaterThan(..) => (left > right).into(),
            Token::SmallerThan(..) => (left < right).into(),
            Token::GreaterEqual(..) => (left >= right).into(),
            Token::SmallerEqual(..) => (left <= right).into(),
            token => unreachable!("Encountered an unexpected binary operator '{token}'"),
        }
    }

    fn visit_logical(&mut self, left: &Expr, operator: &Token, right: &Expr) -> FilterValue {
        let left = self.visit_expr(left);

        match operator {
            Token::And(..) if left.is_truthy() => self.visit_expr(right),
            Token::And(..) => left,
            Token::Or(..) if !left.is_truthy() => self.visit_expr(right),
            Token::Or(..) => left,
            token => unreachable!("Encountered an unexpected logical operator '{token}'"),
        }
    }

    fn visit_unary(&mut self, operator: &Token, right: &Expr) -> FilterValue {
        let right = self.visit_expr(right);

        match operator {
            Token::Not(..) => {
                if right.is_truthy() {
                    false.into()
                } else {
                    true.into()
                }
            }
            token => unreachable!("Encountered an unexpected unary operator '{token}'"),
        }
    }
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use crate::filter::lexer::Scanner;

    use super::*;

    #[derive(Debug, PartialEq)]
    struct TestFilterable;

    impl TestFilterable {
        pub fn matches(filter: &str) -> bool {
            use crate::filter::parser::Parser;

            let tokens = Scanner::new(filter);
            let expr = Parser::parse(tokens).expect("parse the filter");
            let mut context = FilterContext::new(&Self);
            let result = context.visit_expr(&expr);
            result.is_truthy()
        }
    }

    impl Filterable for TestFilterable {
        fn get(&self, property: &str) -> FilterValue {
            match property {
                "boolean" => true.into(),
                "string" => "Alice".into(),
                "number" => 1.into(),
                "null" => FilterValue::Null,
                "tuple" => vec![true.into(), false.into()].into(),
                _ => FilterValue::Null,
            }
        }
    }

    #[rstest]
    #[case("true", true)]
    #[case("false", false)]
    #[case("null", false)]
    #[case("1", true)]
    #[case("0", false)]
    #[case("\"\"", false)]
    #[case("\"Alice\"", true)]
    fn literals(#[case] filter: &str, #[case] expected: bool) {
        assert_eq!(TestFilterable::matches(filter), expected);
    }

    #[rstest]
    #[case("boolean", true)]
    #[case("string", true)]
    #[case("number", true)]
    #[case("tuple", true)]
    #[case("null", false)]
    #[case("unknown", false)]
    fn properties(#[case] filter: &str, #[case] expected: bool) {
        assert_eq!(TestFilterable::matches(filter), expected);
    }

    #[rstest]
    #[case("boolean == true", true)]
    #[case("boolean == false", false)]
    #[case("string == \"Alice\"", true)]
    #[case("string == \"Bob\"", false)]
    #[case("number == 1", true)]
    #[case("number == 2", false)]
    #[case("tuple == [true, false]", true)]
    #[case("tuple == [false, true]", false)]
    #[case("tuple == []", false)]
    #[case("null == null", true)]
    fn equals(#[case] filter: &str, #[case] expected: bool) {
        assert_eq!(TestFilterable::matches(filter), expected);
    }

    #[rstest]
    #[case("2 > 1", true)]
    #[case("1 > 2", false)]
    #[case("2 >= 1", true)]
    #[case("2 >= 2", true)]
    fn greater_than(#[case] filter: &str, #[case] expected: bool) {
        assert_eq!(TestFilterable::matches(filter), expected);
    }

    #[rstest]
    #[case("1 < 2", true)]
    #[case("2 < 1", false)]
    #[case("1 <= 2", true)]
    #[case("1 <= 1", true)]
    #[case("2 <= 1", false)]
    fn smaller(#[case] filter: &str, #[case] expected: bool) {
        assert_eq!(TestFilterable::matches(filter), expected);
    }

    #[rstest]
    #[case("boolean != true", false)]
    #[case("boolean != false", true)]
    #[case("string != \"Alice\"", false)]
    #[case("string != \"Bob\"", true)]
    #[case("number != 1", false)]
    #[case("number != 2", true)]
    #[case("tuple != [true, false]", false)]
    #[case("tuple != [false, true]", true)]
    #[case("tuple != []", true)]
    #[case("null != null", false)]
    fn not_equals(#[case] filter: &str, #[case] expected: bool) {
        assert_eq!(TestFilterable::matches(filter), expected);
    }

    #[rstest]
    #[case("string contains \"Ali\"", true)]
    #[case("string contains \"Bob\"", false)]
    #[case("tuple contains true", true)]
    #[case("tuple contains false", true)]
    #[case("tuple contains null", false)]
    #[case("null contains null", false)]
    fn contains(#[case] filter: &str, #[case] expected: bool) {
        assert_eq!(TestFilterable::matches(filter), expected);
    }

    #[rstest]
    #[case("string in \"Alice\"", true)]
    #[case("\"Ali\" in string", true)]
    #[case("string in \"Bob\"", false)]
    #[case("\"Bob\" in string", false)]
    #[case("true in tuple", true)]
    #[case("false in tuple", true)]
    #[case("null in tuple", false)]
    #[case("number in 1", false)]
    #[case("null in null", false)]
    fn in_(#[case] filter: &str, #[case] expected: bool) {
        assert_eq!(TestFilterable::matches(filter), expected);
    }

    #[rstest]
    #[case("string startswith \"Ali\"", true)]
    #[case("string startswith \"Bob\"", false)]
    #[case("string startswith null", false)]
    #[case("null startswith null", false)]
    fn startswith(#[case] filter: &str, #[case] expected: bool) {
        assert_eq!(TestFilterable::matches(filter), expected);
    }

    #[rstest]
    #[case("string endswith \"ce\"", true)]
    #[case("string endswith \"ob\"", false)]
    #[case("string endswith null", false)]
    #[case("null endswith null", false)]
    fn endswith(#[case] filter: &str, #[case] expected: bool) {
        assert_eq!(TestFilterable::matches(filter), expected);
    }

    #[rstest]
    #[case("!boolean", false)]
    #[case("!string", false)]
    #[case("!number", false)]
    #[case("!tuple", false)]
    #[case("!null", true)]
    fn not(#[case] filter: &str, #[case] expected: bool) {
        assert_eq!(TestFilterable::matches(filter), expected);
    }

    #[rstest]
    #[case("true && true", true)]
    #[case("true && false", false)]
    #[case("false && true", false)]
    #[case("false && false", false)]
    #[case("string && number", true)]
    #[case("string && null", false)]
    fn and(#[case] filter: &str, #[case] expected: bool) {
        assert_eq!(TestFilterable::matches(filter), expected);
    }

    #[rstest]
    #[case("true || true", true)]
    #[case("true || false", true)]
    #[case("false || true", true)]
    #[case("false || false", false)]
    #[case("string || number", true)]
    #[case("string || null", true)]
    #[case("null || null", false)]
    fn or(#[case] filter: &str, #[case] expected: bool) {
        assert_eq!(TestFilterable::matches(filter), expected);
    }

    #[rstest]
    #[case("true && (false || true)", true)]
    #[case("true && (false || false)", false)]
    #[case("true && (string || null)", true)]
    #[case("false && (string || null)", false)]
    fn grouping(#[case] filter: &str, #[case] expected: bool) {
        assert_eq!(TestFilterable::matches(filter), expected);
    }

    #[rstest]
    #[case("true && false || true", true)]
    #[case("true && false || false", false)]
    #[case("false && true || false", false)]
    #[case("false && false || true", true)]
    fn precedence(#[case] filter: &str, #[case] expected: bool) {
        assert_eq!(TestFilterable::matches(filter), expected);
    }
}
