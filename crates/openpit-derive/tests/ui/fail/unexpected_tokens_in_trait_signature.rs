extern crate self as openpit;

use openpit_derive::RequestFields;

pub struct RequestFieldAccessError;

pub trait HasInstrument {
    fn instrument(&self) -> Result<u32, RequestFieldAccessError>;
}

#[derive(RequestFields)]
struct Wrapper {
    #[openpit(HasInstrument(instrument -> Result<u32, RequestFieldAccessError>, extra))]
    operation: u64,
}

fn main() {}
