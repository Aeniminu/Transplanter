pub mod api_map;
pub mod codegen;
pub mod error;
pub mod ir;
pub mod lexer;
pub mod parser;
pub mod prelude;
mod strict;

pub use error::RustToPythonError;

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum OutputMode {
    Entry,
    Module,
}

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
        self.check_with_mode(source, OutputMode::Entry)
    }

    pub fn compile(&self, source: &str) -> Result<String, RustToPythonError> {
        self.compile_with_mode(source, OutputMode::Entry)
    }

    pub fn check_with_mode(&self, source: &str, mode: OutputMode) -> Result<(), RustToPythonError> {
        parse_source(source, mode).map(|_| ())
    }

    pub fn compile_with_mode(
        &self,
        source: &str,
        mode: OutputMode,
    ) -> Result<String, RustToPythonError> {
        let program = parse_source(source, mode)?;
        Ok(codegen::generate(&program, mode))
    }
}

pub fn compile_source(source: &str) -> Result<String, RustToPythonError> {
    RustToPython.compile(source)
}

pub fn check_source(source: &str) -> Result<(), RustToPythonError> {
    RustToPython.check(source)
}

pub fn compile_module_source(source: &str) -> Result<String, RustToPythonError> {
    RustToPython.compile_with_mode(source, OutputMode::Module)
}

pub fn check_module_source(source: &str) -> Result<(), RustToPythonError> {
    RustToPython.check_with_mode(source, OutputMode::Module)
}

pub fn external_modules(source: &str) -> Result<Vec<String>, RustToPythonError> {
    Ok(parse_source(source, OutputMode::Module)?.external_modules)
}

fn parse_source(source: &str, mode: OutputMode) -> Result<ir::Program, RustToPythonError> {
    let tokens = lexer::lex(source)?;
    parser::parse(&tokens, mode)
}
