pub mod transplanter;

pub use transplanter::{Converter, Transplanter};
pub use transplanter_rust::{RustToPython, RustToPythonError, prelude};

impl Converter for RustToPython {
    type Error = RustToPythonError;

    fn name(&self) -> &'static str {
        RustToPython::name(self)
    }

    fn source_language(&self) -> &'static str {
        RustToPython::source_language(self)
    }

    fn target_language(&self) -> &'static str {
        RustToPython::target_language(self)
    }

    fn check(&self, source: &str) -> Result<(), Self::Error> {
        RustToPython::check(self, source)
    }

    fn compile(&self, source: &str) -> Result<String, Self::Error> {
        RustToPython::compile(self, source)
    }
}

pub mod error {
    pub use transplanter_rust::RustToPythonError;
}

pub fn rust_to_python_transplanter() -> Transplanter<RustToPython> {
    Transplanter::new(RustToPython)
}

pub fn compile_source(source: &str) -> Result<String, RustToPythonError> {
    rust_to_python_transplanter().compile(source)
}

pub fn check_source(source: &str) -> Result<(), RustToPythonError> {
    rust_to_python_transplanter().check(source)
}

pub fn compile_module_source(source: &str) -> Result<String, RustToPythonError> {
    transplanter_rust::compile_module_source(source)
}

pub fn check_module_source(source: &str) -> Result<(), RustToPythonError> {
    transplanter_rust::check_module_source(source)
}

pub fn external_modules(source: &str) -> Result<Vec<String>, RustToPythonError> {
    transplanter_rust::external_modules(source)
}
