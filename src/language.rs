use std::path::Path;

use crate::paths::{is_lisp_file, is_rs_file, is_source_file};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum LanguageMode {
    #[default]
    Auto,
    Rust,
    Lisp,
}

impl LanguageMode {
    pub fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "auto" => Some(Self::Auto),
            "rust" => Some(Self::Rust),
            "lisp" => Some(Self::Lisp),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Rust => "rust",
            Self::Lisp => "lisp",
        }
    }

    pub fn display_name(self) -> &'static str {
        match self {
            Self::Auto => "自動",
            Self::Rust => "Rust",
            Self::Lisp => "Lisp",
        }
    }

    pub fn next(self) -> Self {
        match self {
            Self::Rust => Self::Lisp,
            Self::Lisp => Self::Auto,
            Self::Auto => Self::Rust,
        }
    }

    pub fn includes_rust(self) -> bool {
        matches!(self, Self::Auto | Self::Rust)
    }

    pub fn accepts_path(self, path: &Path) -> bool {
        match self {
            Self::Auto => is_source_file(path),
            Self::Rust => is_rs_file(path),
            Self::Lisp => is_lisp_file(path),
        }
    }
}
