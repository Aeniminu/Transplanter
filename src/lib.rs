pub mod converters;
pub mod transplanter;

pub use converters::rust_to_python::{
    RustToPython, RustToPythonError, check_source, compile_source, prelude,
};
pub use transplanter::{Converter, Transplanter};

pub mod error {
    pub use crate::RustToPythonError;
}

pub fn rust_to_python_transplanter() -> Transplanter<RustToPython> {
    Transplanter::new(RustToPython)
}
