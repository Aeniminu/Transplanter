use std::error::Error;
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FarmError {
    pub message: String,
    pub line: usize,
    pub column: usize,
}

impl FarmError {
    pub fn new(message: impl Into<String>, line: usize, column: usize) -> Self {
        Self {
            message: message.into(),
            line,
            column,
        }
    }

    pub fn unsupported(syntax: impl Into<String>, line: usize, column: usize) -> Self {
        Self::new(
            format!("unsupported syntax: {}", syntax.into()),
            line,
            column,
        )
    }
}

impl fmt::Display for FarmError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "error: {} at {}:{}",
            self.message, self.line, self.column
        )
    }
}

impl Error for FarmError {}
