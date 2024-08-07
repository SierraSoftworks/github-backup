use crate::errors::Error;

pub struct GlobexScanner<'a> {
    source: &'a str,
    chars: std::iter::Peekable<std::str::CharIndices<'a>>,
}

pub enum GlobexToken {
    Literal(String),
    WildcardOne,
    WildcardMany
}

impl<'a> GlobexScanner<'a> {
    pub fn new(source: &'a str) -> Self {
        GlobexScanner {
            source,
            chars: source.char_indices().peekable(),
        }
    }

    pub fn is_reserved_char(c: char) -> bool {
        c == '*' || c == '?'
    }

    // Need to refactor GlobexScanner and Scanner to use a few common methods.
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

    fn read_literal(&mut self, start: usize) -> Result<GlobexToken, Error> {
        let end = start
            + self.advance_while_fn(|_, c| !GlobexScanner::is_reserved_char(c));
        let lexeme = &self.source[start..end + 1];

        Ok(GlobexToken::Literal(lexeme.to_string()))
    }
}


impl<'a> Iterator for GlobexScanner<'a> {
    type Item = Result<GlobexToken, Error>;

    fn next(&mut self) -> Option<Self::Item> {
        while let Some((loc, c)) = self.chars.next() {
            match c {
                '*' => { return Some(Ok(GlobexToken::WildcardMany)); },
                '?' => { return Some(Ok(GlobexToken::WildcardOne)); },
                _ =>  { return Some(self.read_literal(loc)); }
            }
        }

        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty() {
        assert_sequence("", &[]);
    }
}
