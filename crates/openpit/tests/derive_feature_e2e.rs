#![cfg(feature = "derive")]

use openpit::param::{Asset, Fee, Pnl};
use openpit::pretrade::policies::{PnlBoundsBarrier, PnlBoundsKillSwitchPolicy};
use openpit::{
    Engine, HasFee, HasInstrument, HasPnl, Instrument, RequestFieldAccessError, RequestFields,
};

trait HasStrategyTag {
    fn strategy_tag(&self) -> Result<&str, RequestFieldAccessError>;
}

struct OrderCore {
    instrument: Instrument,
}

impl HasInstrument for OrderCore {
    fn instrument(&self) -> Result<&Instrument, RequestFieldAccessError> {
        Ok(&self.instrument)
    }
}

struct OrderInner {
    strategy_tag: String,
}

impl HasStrategyTag for OrderInner {
    fn strategy_tag(&self) -> Result<&str, RequestFieldAccessError> {
        Ok(self.strategy_tag.as_str())
    }
}

#[derive(RequestFields)]
struct DerivedOrder {
    #[openpit(HasInstrument(instrument -> Result<&Instrument, RequestFieldAccessError>))]
    operation: OrderCore,
    #[openpit(inner, HasStrategyTag(-> Result<&str, RequestFieldAccessError>))]
    inner: OrderInner,
}

struct ReportCore {
    instrument: Instrument,
    pnl: Pnl,
}

impl HasInstrument for ReportCore {
    fn instrument(&self) -> Result<&Instrument, RequestFieldAccessError> {
        Ok(&self.instrument)
    }
}

impl HasPnl for ReportCore {
    fn pnl(&self) -> Result<Pnl, RequestFieldAccessError> {
        Ok(self.pnl)
    }
}

struct ReportInner {
    fee: Fee,
}

impl HasFee for ReportInner {
    fn fee(&self) -> Result<Fee, RequestFieldAccessError> {
        Ok(self.fee)
    }
}

#[derive(RequestFields)]
struct DerivedReport {
    #[openpit(
        HasInstrument(-> Result<&Instrument, RequestFieldAccessError>),
        HasPnl(-> Result<Pnl, RequestFieldAccessError>)
    )]
    payload: ReportCore,
    #[openpit(inner, HasFee(-> Result<Fee, RequestFieldAccessError>))]
    inner: ReportInner,
}

#[test]
fn derive_feature_reexport_builds_wrappers_and_engine_smoke_path() {
    let instrument = Instrument::new(
        Asset::new("SPX").expect("must be valid"),
        Asset::new("USD").expect("must be valid"),
    );
    let order = DerivedOrder {
        operation: OrderCore {
            instrument: instrument.clone(),
        },
        inner: OrderInner {
            strategy_tag: "alpha-1".to_owned(),
        },
    };
    let report = DerivedReport {
        payload: ReportCore {
            instrument: instrument.clone(),
            pnl: Pnl::from_str("-10").expect("must be valid"),
        },
        inner: ReportInner {
            fee: Fee::from_str("-1").expect("must be valid"),
        },
    };

    assert_eq!(order.instrument(), Ok(&instrument));
    assert_eq!(order.strategy_tag(), Ok("alpha-1"));
    assert_eq!(report.instrument(), Ok(&instrument));
    assert_eq!(
        report.pnl(),
        Ok(Pnl::from_str("-10").expect("must be valid"))
    );
    assert_eq!(
        report.fee(),
        Ok(Fee::from_str("-1").expect("must be valid"))
    );

    let pnl_policy = PnlBoundsKillSwitchPolicy::new(
        PnlBoundsBarrier {
            settlement_asset: Asset::new("USD").expect("must be valid"),
            lower_bound: Some(Pnl::from_str("-500").expect("must be valid")),
            upper_bound: None,
            initial_pnl: Pnl::ZERO,
        },
        [],
    )
    .expect("must build policy");
    let engine = Engine::<DerivedOrder, DerivedReport>::builder()
        .check_pre_trade_start_policy(pnl_policy)
        .build()
        .expect("must build engine");

    let request = engine
        .start_pre_trade(order)
        .expect("start stage must accept order");
    let _reservation = request.execute().expect("main stage must accept order");
    let post_trade = engine.apply_execution_report(&report);

    assert!(!post_trade.kill_switch_triggered);
}
