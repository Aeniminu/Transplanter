pub mod api_map;
pub mod codegen;
pub mod error;
pub mod ir;
pub mod lexer;
pub mod parser;
pub mod prelude;

use error::FarmError;

pub fn compile_source(source: &str) -> Result<String, FarmError> {
    let program = parse_source(source)?;
    Ok(codegen::generate(&program))
}

pub fn check_source(source: &str) -> Result<(), FarmError> {
    parse_source(source).map(|_| ())
}

fn parse_source(source: &str) -> Result<ir::FarmIr, FarmError> {
    let tokens = lexer::lex(source)?;
    parser::parse(&tokens)
}
