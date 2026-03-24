extern crate self as openpit;

use openpit_derive::RequestFields;

pub struct RequestFieldAccessError;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Instrument;

pub trait HasInstrument {
    fn instrument(&self) -> Result<&Instrument, RequestFieldAccessError>;
}

struct Operation;

impl Operation {
    fn instrument(&self) -> Result<&Instrument, RequestFieldAccessError> {
        unimplemented!()
    }
}

#[derive(RequestFields)]
struct Wrapper {
    #[openpit(HasInstrument)]
    operation: Operation,
}

fn main() {}
