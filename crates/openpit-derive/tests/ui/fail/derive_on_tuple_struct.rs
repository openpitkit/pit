extern crate self as openpit;

use openpit_derive::RequestFields;

#[derive(RequestFields)]
struct Wrapper(u64);

fn main() {}
