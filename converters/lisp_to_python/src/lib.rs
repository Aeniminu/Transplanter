pub mod codegen;
pub mod error;
pub mod ir;
pub mod lexer;
pub mod parser;

pub use error::LispToPythonError;

#[derive(Debug, Clone, Copy, Default)]
pub struct LispToPython;

impl LispToPython {
    pub fn name(&self) -> &'static str {
        "lisp-to-python"
    }

    pub fn source_language(&self) -> &'static str {
        "Scheme-like Lisp"
    }

    pub fn target_language(&self) -> &'static str {
        "The Farmer Was Replaced Python"
    }

    pub fn check(&self, source: &str) -> Result<(), LispToPythonError> {
        parse_source(source).map(|_| ())
    }

    pub fn compile(&self, source: &str) -> Result<String, LispToPythonError> {
        let forms = parse_source(source)?;
        codegen::generate(&forms)
    }
}

pub fn compile_source(source: &str) -> Result<String, LispToPythonError> {
    LispToPython.compile(source)
}

pub fn check_source(source: &str) -> Result<(), LispToPythonError> {
    LispToPython.check(source)
}

fn parse_source(source: &str) -> Result<Vec<ir::Expr>, LispToPythonError> {
    let tokens = lexer::lex(source)?;
    parser::parse(&tokens)
}
