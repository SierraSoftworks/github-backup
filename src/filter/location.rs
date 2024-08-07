use std::fmt::Display;

#[derive(PartialEq, Eq, PartialOrd, Ord, Debug, Copy, Clone, Hash, Default)]
pub struct Loc {
    pub line: usize,
    pub column: usize,
}

impl Loc {
    pub fn new(line: usize, column: usize) -> Self {
        Self { line, column }
    }
}

impl Display for Loc {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Loc { line: 0, column: 0 } => {
                write!(f, "unknown location")
            }
            Loc { line, column } => {
                write!(f, "line {}, column {}", line, column)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::*;

    #[rstest]
    #[case(0, 0, "unknown location")]
    #[case(1, 2, "line 1, column 2")]
    fn test_display(#[case] line: usize, #[case] column: usize, #[case] expected: &str) {
        assert_eq!(format!("{}", Loc::new(line, column)), expected);
    }
}
