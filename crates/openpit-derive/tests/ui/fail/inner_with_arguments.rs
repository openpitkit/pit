extern crate self as openpit;

use openpit_derive::RequestFields;

pub struct RequestFieldAccessError;

#[derive(RequestFields)]
struct Wrapper {
    #[openpit(inner(something))]
    operation: u64,
}

fn main() {}
