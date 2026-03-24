extern crate self as openpit;

use openpit_derive::RequestFields;

pub struct RequestFieldAccessError;

pub trait HasPnl {
    fn pnl(&self) -> Result<u32, RequestFieldAccessError>;
}

#[derive(RequestFields)]
struct Wrapper<T, U> {
    #[openpit(inner, HasPnl(pnl -> Result<u32, RequestFieldAccessError>))]
    left: T,
    #[openpit(inner, HasPnl(pnl -> Result<u32, RequestFieldAccessError>))]
    right: U,
}

fn main() {}
