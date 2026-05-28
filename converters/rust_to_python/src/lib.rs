pub mod api_map;
pub mod codegen;
pub mod error;
pub mod ir;
pub mod lexer;
pub mod parser;
pub mod prelude;
mod strict;

pub use error::RustToPythonError;

#[derive(Debug, Clone, Copy, Default)]
pub struct RustToPython;

impl RustToPython {
    pub fn name(&self) -> &'static str {
        "rust-to-python"
    }

    pub fn source_language(&self) -> &'static str {
        "Rust"
    }

    pub fn target_language(&self) -> &'static str {
        "The Farmer Was Replaced Python"
    }

    pub fn check(&self, source: &str) -> Result<(), RustToPythonError> {
        parse_source(source).map(|_| ())
    }

    pub fn compile(&self, source: &str) -> Result<String, RustToPythonError> {
        let program = parse_source(source)?;
        Ok(codegen::generate(&program))
    }
}

pub fn compile_source(source: &str) -> Result<String, RustToPythonError> {
    RustToPython.compile(source)
}

pub fn check_source(source: &str) -> Result<(), RustToPythonError> {
    RustToPython.check(source)
}

fn parse_source(source: &str) -> Result<ir::Program, RustToPythonError> {
    let tokens = lexer::lex(source)?;
    parser::parse(&tokens)
}
