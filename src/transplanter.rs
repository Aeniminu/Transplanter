pub trait Converter {
    type Error;

    fn name(&self) -> &'static str;
    fn source_language(&self) -> &'static str;
    fn target_language(&self) -> &'static str;
    fn check(&self, source: &str) -> Result<(), Self::Error>;
    fn compile(&self, source: &str) -> Result<String, Self::Error>;
}

#[derive(Debug, Clone, Copy)]
pub struct Transplanter<C> {
    converter: C,
}

impl<C> Transplanter<C> {
    pub fn new(converter: C) -> Self {
        Self { converter }
    }
}

impl<C> Transplanter<C>
where
    C: Converter,
{
    pub fn converter(&self) -> &C {
        &self.converter
    }

    pub fn check(&self, source: &str) -> Result<(), C::Error> {
        self.converter.check(source)
    }

    pub fn compile(&self, source: &str) -> Result<String, C::Error> {
        self.converter.compile(source)
    }
}
