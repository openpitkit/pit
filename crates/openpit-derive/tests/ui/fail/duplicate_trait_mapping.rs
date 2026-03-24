extern crate self as openpit;

use openpit_derive::RequestFields;

pub struct RequestFieldAccessError;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Instrument;

pub trait HasInstrument {
    fn instrument(&self) -> Result<&Instrument, RequestFieldAccessError>;
}

struct Operation {
    instrument: Instrument,
}

impl Operation {
    fn instrument(&self) -> Result<&Instrument, RequestFieldAccessError> {
        Ok(&self.instrument)
    }
}

#[derive(RequestFields)]
struct Wrapper {
    #[openpit(HasInstrument(instrument -> Result<&Instrument, RequestFieldAccessError>))]
    left: Operation,
    #[openpit(HasInstrument(instrument -> Result<&Instrument, RequestFieldAccessError>))]
    right: Operation,
}

fn main() {}
