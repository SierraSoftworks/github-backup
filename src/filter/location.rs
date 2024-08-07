use std::fmt::Display;

#[derive(PartialEq, Eq, PartialOrd, Ord, Debug, Copy, Clone, Hash, Default)]
pub struct Loc {
    line: usize,
    column: usize,
}

impl Loc {
    pub fn new(line: usize, column: usize) -> Self {
        Self { line, column }
    }
}

impl Display for Loc {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "line {}, column {}", self.line, self.column)
    }
}
