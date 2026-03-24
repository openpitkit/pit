extern crate self as openpit;

use openpit_derive::RequestFields;

pub struct RequestFieldAccessError;

pub mod param {
    #[derive(Clone, Debug, PartialEq, Eq)]
    pub struct Asset;
}

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
struct Wrapper<T> {
    inner: T,
    #[request_fields(HasInstrument(instrument -> &Instrument))]
    operation: Operation,
}

fn main() {}
