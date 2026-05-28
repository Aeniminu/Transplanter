pub mod api_map;
pub mod codegen;
pub mod error;
pub mod ir;
pub mod lexer;
pub mod parser;
pub mod prelude;
mod strict;

use crate::transplanter::Converter;
pub use error::RustToPythonError;

#[derive(Debug, Clone, Copy, Default)]
pub struct RustToPython;

impl Converter for RustToPython {
    type Error = RustToPythonError;

    fn name(&self) -> &'static str {
        "rust-to-python"
    }

    fn source_language(&self) -> &'static str {
        "Rust"
    }

    fn target_language(&self) -> &'static str {
        "The Farmer Was Replaced Python"
    }

    fn check(&self, source: &str) -> Result<(), Self::Error> {
        parse_source(source).map(|_| ())
    }

    fn compile(&self, source: &str) -> Result<String, Self::Error> {
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
