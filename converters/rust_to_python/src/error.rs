use std::error::Error;
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RustToPythonError {
    pub message: String,
    pub line: usize,
    pub column: usize,
}

impl RustToPythonError {
    pub fn new(message: impl Into<String>, line: usize, column: usize) -> Self {
        Self {
            message: message.into(),
            line,
            column,
        }
    }

    pub fn unsupported(syntax: impl Into<String>, line: usize, column: usize) -> Self {
        Self::new(format!("未対応構文: {}", syntax.into()), line, column)
    }
}

impl fmt::Display for RustToPythonError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "エラー: {}行{}列: {}",
            self.line, self.column, self.message
        )
    }
}

impl Error for RustToPythonError {}
