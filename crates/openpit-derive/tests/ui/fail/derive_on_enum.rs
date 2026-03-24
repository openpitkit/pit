extern crate self as openpit;

use openpit_derive::RequestFields;

#[derive(RequestFields)]
enum Wrapper {
    Item,
}

fn main() {}
