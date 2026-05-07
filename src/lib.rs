pub mod api_map;
pub mod codegen;
pub mod error;
pub mod ir;
pub mod lexer;
pub mod parser;

use error::FarmError;

pub fn compile_source(source: &str) -> Result<String, FarmError> {
    let tokens = lexer::lex(source)?;
    let program = parser::parse(&tokens)?;
    Ok(codegen::generate(&program))
}
