extern crate self as openpit;

use openpit_derive::RequestFields;

#[derive(Debug)]
pub struct RequestFieldAccessError;

pub mod param {
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub struct AccountId(pub u64);

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub struct Pnl;
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Instrument;

pub trait HasAccountId {
    fn account_id(&self) -> Result<param::AccountId, RequestFieldAccessError>;
}

pub trait HasInstrument {
    fn instrument(&self) -> Result<&Instrument, RequestFieldAccessError>;
}

pub trait HasPnl {
    fn pnl(&self) -> Result<param::Pnl, RequestFieldAccessError>;
}

pub trait HasStrategyTag {
    fn strategy_tag(&self) -> &'static str;
}

struct Operation {
    instrument: Instrument,
    account_id: param::AccountId,
}

impl Operation {
    fn instrument(&self) -> Result<&Instrument, RequestFieldAccessError> {
        Ok(&self.instrument)
    }

    fn account_id(&self) -> Result<param::AccountId, RequestFieldAccessError> {
        Ok(self.account_id)
    }
}

struct Base;

impl HasPnl for Base {
    fn pnl(&self) -> Result<param::Pnl, RequestFieldAccessError> {
        Ok(param::Pnl)
    }
}

impl HasStrategyTag for Base {
    fn strategy_tag(&self) -> &'static str {
        "alpha"
    }
}

#[derive(RequestFields)]
struct Wrapper<T> {
    #[openpit(
        inner,
        HasPnl(pnl -> Result<param::Pnl, RequestFieldAccessError>),
        HasStrategyTag(-> &'static str)
    )]
    base: T,
    #[openpit(
        HasInstrument(instrument -> Result<&Instrument, RequestFieldAccessError>),
        HasAccountId(account_id -> Result<param::AccountId, RequestFieldAccessError>)
    )]
    operation: Operation,
}

fn assert_impls<T>(value: &Wrapper<T>)
where
    Wrapper<T>: HasInstrument + HasAccountId + HasPnl + HasStrategyTag,
{
    let _ = value.instrument();
    let _ = value.account_id();
    let _ = value.pnl();
    let _ = value.strategy_tag();
}

fn main() {
    let w = Wrapper {
        base: Base,
        operation: Operation {
            instrument: Instrument,
            account_id: param::AccountId(7),
        },
    };
    assert_impls(&w);
}
