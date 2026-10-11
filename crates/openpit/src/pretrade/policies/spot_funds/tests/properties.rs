// Copyright The Pit Project Owners. All rights reserved.
// SPDX-License-Identifier: Apache-2.0
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.
//
// Please see https://openpit.dev and the OWNERS file for details.

use super::*;

use std::collections::HashMap;

use proptest::prelude::*;
use proptest::sample::Index;

use crate::param::RoundingStrategy;
use crate::pretrade::policies::proptest_config::config;

// Sized for about 3 s per property in a debug build.
const CASES: u32 = 1280;
const MAX_OPS: usize = 40;
// The largest generated order quantity, in hundredths.
const MAX_QUANTITY: i64 = 5_000;

const ACCOUNTS: [u64; 2] = [99_224_416, 11_223_344];
const ASSETS: [&str; 3] = ["AAPL", "MSFT", "USD"];
const INSTRUMENTS: [(&str, &str); 4] = [
    ("AAPL", "USD"),
    ("MSFT", "USD"),
    ("AAPL", "MSFT"),
    ("USD", "USD"),
];
// Positive prices with terminating inverses, so most quantities derived
// through them land a projection exactly on its target.
const BOUNDARY_PRICES: [&str; 10] = [
    "0.2", "0.25", "0.5", "0.8", "1", "1.25", "2", "4", "5", "20",
];

fn universe() -> impl Iterator<Item = (AccountId, Asset)> {
    ACCOUNTS.into_iter().flat_map(|id| {
        ASSETS
            .into_iter()
            .map(move |code| (account(id), asset(code)))
    })
}

// A count of hundredths as a decimal literal.
fn hundredths(count: i64) -> String {
    let sign = if count < 0 { "-" } else { "" };
    let magnitude = count.unsigned_abs();
    format!("{sign}{}.{:02}", magnitude / 100, magnitude % 100)
}

fn pre_trade_lock(price: Price) -> PreTradeLock {
    PreTradeLock::from_entries([(DEFAULT_POLICY_GROUP_ID, price)])
}

#[derive(Debug, Clone)]
struct Case {
    mode: SpotFundsLimitMode,
    // Seeded available and optional position limit, in universe order.
    slots: [(PositionSize, Option<Quantity>); 6],
    ops: Vec<Op>,
    exclusions: &'static [Exclusion],
}

#[derive(Debug, Clone)]
enum Op {
    Order {
        account_id: AccountId,
        instrument: Instrument,
        side: Side,
        amount: TradeAmount,
        price: Price,
        commit: bool,
    },
    // A quantity order whose projection on a limited leg asset lands on the
    // limit moved outward by `delta`.
    BoundaryOrder {
        account_id: AccountId,
        instrument: Instrument,
        side: Side,
        price: Price,
        leg: Index,
        delta: PositionSize,
        commit: bool,
    },
    Fill {
        order: Index,
        quantity: FillQuantity,
        price: TradePrice,
        fee: Option<MonetaryAmount>,
        is_final: bool,
    },
    Cancel {
        order: Index,
    },
    FeeOnly {
        account_id: AccountId,
        instrument: Instrument,
        side: Side,
        fee: MonetaryAmount,
    },
}

#[derive(Debug, Clone, Copy)]
enum FillQuantity {
    Full,
    // A share of the remainder in percent, rounded down to hundredths.
    Partial(i64),
    Over(Quantity),
}

#[derive(Debug, Clone, Copy)]
enum TradePrice {
    AtLock,
    Other(Price),
}

#[derive(Debug, Clone, Copy)]
enum Profile {
    Arbitrary,
    // Fills at the lock price within the order quantity, no fees, and at
    // least one limited pair.
    ExactExecution,
    // Fills within the order quantity at any price, fees allowed.
    WithinQuantity,
}

// An input domain a gate variant leaves out while the defect it triggers
// is open.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Exclusion {
    // Orders whose derived quantity, a volume order's v/p or a boundary
    // order's quotient, is rounded; an exact one stays in.
    InexactQuantities,
    NegativePriceBuys,
}

fn order_price() -> impl Strategy<Value = Price> {
    prop_oneof![1 => Just(0i64), 2 => -10_000i64..=-1, 5 => 1i64..=10_000]
        .prop_map(|count| px(&hundredths(count)))
}

fn amount() -> impl Strategy<Value = TradeAmount> {
    prop_oneof![
        (1i64..=MAX_QUANTITY).prop_map(|count| TradeAmount::Quantity(qty(&hundredths(count)))),
        (1i64..=500_000).prop_map(|count| TradeAmount::Volume(vol(&hundredths(count)))),
    ]
}

fn fee() -> impl Strategy<Value = MonetaryAmount> {
    (-500i64..=500, prop::sample::select(ASSETS.to_vec()))
        .prop_map(|(count, currency)| money_fee(&hundredths(count), currency))
}

fn commit() -> impl Strategy<Value = bool> {
    prop_oneof![3 => Just(true), 1 => Just(false)]
}

fn subject() -> impl Strategy<Value = (AccountId, Instrument, Side)> {
    (
        prop::sample::select(ACCOUNTS.to_vec()).prop_map(account),
        prop::sample::select(INSTRUMENTS.to_vec())
            .prop_map(|(underlying, settlement)| instr(underlying, settlement)),
        prop_oneof![Just(Side::Buy), Just(Side::Sell)],
    )
}

// Seeded available and an optional limit no tighter than the seeded
// position.
fn universe_slot() -> impl Strategy<Value = (PositionSize, Option<Quantity>)> {
    let available = prop_oneof![
        1 => Just(0i64),
        1 => -1_000_000i64..=-1,
        4 => 1i64..=1_000_000,
    ];
    let headroom = prop::option::of(prop_oneof![Just(0i64), 0i64..=10_000, 0i64..=500_000]);
    (available, headroom).prop_map(|(available, headroom)| {
        let limit = headroom.map(|headroom| qty(&hundredths(available.abs() + headroom)));
        (ps(&hundredths(available)), limit)
    })
}

fn op(profile: Profile, exclusions: &'static [Exclusion]) -> BoxedStrategy<Op> {
    let no_negative_price_buys = exclusions.contains(&Exclusion::NegativePriceBuys);
    let order = (subject(), amount(), order_price(), commit())
        .prop_filter(
            "buy at a negative price",
            move |((_, _, side), _, price, _)| {
                !(no_negative_price_buys && *side == Side::Buy && *price < Price::ZERO)
            },
        )
        .prop_map(
            |((account_id, instrument, side), amount, price, commit)| Op::Order {
                account_id,
                instrument,
                side,
                amount,
                price,
                commit,
            },
        );
    let boundary = (
        subject(),
        prop::sample::select(BOUNDARY_PRICES.to_vec()),
        any::<Index>(),
        -1i64..=1,
        commit(),
    )
        .prop_map(
            |((account_id, instrument, side), price, leg, delta, commit)| Op::BoundaryOrder {
                account_id,
                instrument,
                side,
                price: px(price),
                leg,
                delta: ps(&hundredths(delta)),
                commit,
            },
        );
    let quantity = match profile {
        Profile::Arbitrary => prop_oneof![
            Just(FillQuantity::Full),
            (1i64..=99).prop_map(FillQuantity::Partial),
            (1i64..=500).prop_map(|count| FillQuantity::Over(qty(&hundredths(count)))),
        ]
        .boxed(),
        Profile::ExactExecution | Profile::WithinQuantity => prop_oneof![
            Just(FillQuantity::Full),
            (1i64..=99).prop_map(FillQuantity::Partial),
        ]
        .boxed(),
    };
    let (trade_price, fill_fee) = match profile {
        Profile::ExactExecution => (
            Just(TradePrice::AtLock).boxed(),
            Just(None::<MonetaryAmount>).boxed(),
        ),
        Profile::Arbitrary | Profile::WithinQuantity => (
            prop_oneof![
                Just(TradePrice::AtLock),
                order_price().prop_map(TradePrice::Other)
            ]
            .boxed(),
            prop::option::of(fee()).boxed(),
        ),
    };
    let fill = (
        any::<Index>(),
        quantity,
        trade_price,
        fill_fee,
        any::<bool>(),
    )
        .prop_map(|(order, quantity, price, fee, is_final)| Op::Fill {
            order,
            quantity,
            price,
            fee,
            is_final,
        });
    let cancel = any::<Index>().prop_map(|order| Op::Cancel { order });
    let fee_only =
        (subject(), fee()).prop_map(|((account_id, instrument, side), fee)| Op::FeeOnly {
            account_id,
            instrument,
            side,
            fee,
        });
    match profile {
        Profile::ExactExecution => {
            prop_oneof![4 => order, 2 => boundary, 4 => fill, 1 => cancel].boxed()
        }
        Profile::Arbitrary | Profile::WithinQuantity => prop_oneof![
            4 => order,
            2 => boundary,
            4 => fill,
            1 => cancel,
            1 => fee_only,
        ]
        .boxed(),
    }
}

fn case(profile: Profile, exclusions: &'static [Exclusion]) -> impl Strategy<Value = Case> {
    let mode = prop_oneof![
        Just(SpotFundsLimitMode::Enforce),
        Just(SpotFundsLimitMode::TrackOnly)
    ];
    let slots = prop::array::uniform6(universe_slot()).prop_filter(
        "exact execution needs a limited pair",
        move |slots| {
            !matches!(profile, Profile::ExactExecution)
                || slots.iter().any(|(_, limit)| limit.is_some())
        },
    );
    (
        mode,
        slots,
        prop::collection::vec(op(profile, exclusions), 1..=MAX_OPS),
    )
        .prop_map(move |(mode, slots, ops)| Case {
            mode,
            slots,
            ops,
            exclusions,
        })
}

// Reference state of one (account, asset) slot; an absent slot is zero.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Slot {
    available: PositionSize,
    held: PositionSize,
    incoming: PositionSize,
}

impl Slot {
    const ZERO: Self = Self {
        available: PositionSize::ZERO,
        held: PositionSize::ZERO,
        incoming: PositionSize::ZERO,
    };
}

// One reservation step: the held outflow and incoming inflow of one asset.
struct Step {
    asset: Asset,
    held: PositionSize,
    incoming: PositionSize,
}

#[derive(Clone)]
struct OpenOrder {
    account_id: AccountId,
    instrument: Instrument,
    side: Side,
    lock: Price,
    remaining: Quantity,
}

struct Admission {
    slots: HashMap<Asset, Slot>,
    order: OpenOrder,
}

// One leg of a fill or a release: held -= consumed, available += consumed +
// flow, incoming -= drained.
struct Leg {
    consumed: PositionSize,
    flow: PositionSize,
    drained: PositionSize,
}

struct Model {
    mode: SpotFundsLimitMode,
    limits: HashMap<(AccountId, Asset), PositionSize>,
    slots: HashMap<(AccountId, Asset), Slot>,
    open: Vec<OpenOrder>,
}

impl Model {
    fn slot(&self, key: &(AccountId, Asset)) -> Slot {
        self.slots.get(key).copied().unwrap_or(Slot::ZERO)
    }

    fn admit(&self, order: &TestOrder) -> Result<Admission, RejectCode> {
        let price = order.price.expect("model orders carry a price");
        let (quantity, steps) =
            order_steps(order.side, &order.instrument, order.trade_amount, price)?;
        // A step that neither holds nor projects does not touch its slot.
        let steps: Vec<Step> = steps
            .into_iter()
            .filter(|step| !step.held.is_zero() || !step.incoming.is_zero())
            .collect();
        let mut after: HashMap<Asset, Slot> = HashMap::new();
        for (index, step) in steps.iter().enumerate() {
            let key = (order.account_id, step.asset.clone());
            let pre = self.slot(&key);
            let current = after.get(&step.asset).copied().unwrap_or(pre);
            if matches!(self.mode, SpotFundsLimitMode::Enforce) && !step.held.is_zero() {
                let spendable = current
                    .available
                    .checked_add(current.held.min(PositionSize::ZERO))
                    .expect("spendable funds");
                if step.held > spendable {
                    return Err(RejectCode::InsufficientFunds);
                }
            }
            let next = Slot {
                available: current
                    .available
                    .checked_sub(step.held)
                    .expect("available after hold"),
                held: current
                    .held
                    .checked_add(step.held)
                    .expect("held after hold"),
                incoming: current
                    .incoming
                    .checked_add(step.incoming)
                    .expect("incoming after projection"),
            };
            after.insert(step.asset.clone(), next);
            let Some(limit) = self.limits.get(&key) else {
                continue;
            };
            let on_asset: Vec<&Step> = steps
                .iter()
                .filter(|other| other.asset == step.asset)
                .collect();
            if steps[index + 1..]
                .iter()
                .all(|later| later.asset != step.asset)
            {
                barrier(pre, &on_asset, *limit)?;
            } else if net(&on_asset) != Ok(PositionSize::ZERO) {
                // The first of two steps on one asset proves its own write
                // exact unless the order's net on the asset is zero.
                exact_sub(pre.available, step.held)?;
                exact_add(pre.held, step.held)?;
                exact_add(pre.incoming, step.incoming)?;
            }
        }
        Ok(Admission {
            slots: after,
            order: OpenOrder {
                account_id: order.account_id,
                instrument: order.instrument.clone(),
                side: order.side,
                lock: price,
                remaining: quantity,
            },
        })
    }

    fn commit(&mut self, admission: Admission) {
        for (asset, slot) in admission.slots {
            self.slots.insert((admission.order.account_id, asset), slot);
        }
        self.open.push(admission.order);
    }

    fn trade(&mut self, order: &OpenOrder, quantity: Quantity, price: Price) {
        let filled = quantity.to_position_size();
        let at_lock = order
            .lock
            .calculate_position_size(quantity)
            .expect("lock notional");
        let paid = price
            .calculate_position_size(quantity)
            .expect("trade notional");
        let (underlying, settlement) = match order.side {
            Side::Buy => (
                Leg {
                    consumed: PositionSize::ZERO,
                    flow: filled,
                    drained: filled,
                },
                Leg {
                    consumed: at_lock.max(PositionSize::ZERO),
                    flow: -paid,
                    drained: PositionSize::ZERO,
                },
            ),
            Side::Sell => (
                Leg {
                    consumed: filled,
                    flow: -filled,
                    drained: PositionSize::ZERO,
                },
                Leg {
                    consumed: (-at_lock).max(PositionSize::ZERO),
                    flow: paid,
                    drained: at_lock.max(PositionSize::ZERO),
                },
            ),
        };
        self.settle(order, underlying, settlement);
    }

    fn release(&mut self, order: &OpenOrder, quantity: Quantity) {
        let released = quantity.to_position_size();
        let at_lock = order
            .lock
            .calculate_position_size(quantity)
            .expect("lock notional");
        let (underlying, settlement) = match order.side {
            Side::Buy => (
                Leg {
                    consumed: PositionSize::ZERO,
                    flow: PositionSize::ZERO,
                    drained: released,
                },
                Leg {
                    consumed: at_lock.max(PositionSize::ZERO),
                    flow: PositionSize::ZERO,
                    drained: PositionSize::ZERO,
                },
            ),
            Side::Sell => (
                Leg {
                    consumed: released,
                    flow: PositionSize::ZERO,
                    drained: PositionSize::ZERO,
                },
                Leg {
                    consumed: (-at_lock).max(PositionSize::ZERO),
                    flow: PositionSize::ZERO,
                    drained: at_lock.max(PositionSize::ZERO),
                },
            ),
        };
        self.settle(order, underlying, settlement);
    }

    // The outflow side moves first: a buy's settlement, a sell's underlying.
    fn settle(&mut self, order: &OpenOrder, underlying: Leg, settlement: Leg) {
        let underlying = (order.instrument.underlying_asset(), underlying);
        let settlement = (order.instrument.settlement_asset(), settlement);
        let legs = match order.side {
            Side::Buy => [settlement, underlying],
            Side::Sell => [underlying, settlement],
        };
        for (asset, leg) in legs {
            let credit = leg
                .consumed
                .checked_add(leg.flow)
                .expect("available credit");
            let slot = self
                .slots
                .entry((order.account_id, asset.clone()))
                .or_insert(Slot::ZERO);
            slot.held = slot.held.checked_sub(leg.consumed).expect("held");
            slot.available = slot.available.checked_add(credit).expect("available");
            slot.incoming = slot.incoming.checked_sub(leg.drained).expect("incoming");
        }
    }

    // A positive fee amount is a charge and a negative one a rebate, so the
    // slot moves by the fee's outflow, never touching held.
    fn fee(&mut self, account_id: AccountId, fee: &MonetaryAmount) {
        let slot = self
            .slots
            .entry((account_id, fee.currency.clone()))
            .or_insert(Slot::ZERO);
        slot.available = slot
            .available
            .checked_add(fee.amount.to_position_size())
            .expect("available after fee");
    }
}

// The reservation steps of a priced order in step order, settlement first
// for a buy and the underlying first for a sell, and the order quantity.
fn order_steps(
    side: Side,
    instrument: &Instrument,
    amount: TradeAmount,
    price: Price,
) -> Result<(Quantity, [Step; 2]), RejectCode> {
    let quantity = match amount {
        TradeAmount::Quantity(quantity) => quantity,
        TradeAmount::Volume(_) if price.is_zero() => {
            return Err(RejectCode::OrderValueCalculationFailed)
        }
        TradeAmount::Volume(volume) => volume
            .calculate_quantity(price)
            .expect("volume order quantity"),
    };
    let notional = price
        .calculate_position_size(quantity)
        .expect("order notional");
    let underlying = instrument.underlying_asset().clone();
    let settlement = instrument.settlement_asset().clone();
    let steps = match side {
        Side::Buy => {
            // A volume buy pays the volume itself, and only at a positive
            // price.
            let paid = match amount {
                TradeAmount::Volume(volume) if price > Price::ZERO => volume.to_position_size(),
                TradeAmount::Volume(_) => PositionSize::ZERO,
                TradeAmount::Quantity(_) => notional.max(PositionSize::ZERO),
            };
            [
                Step {
                    asset: settlement,
                    held: paid,
                    incoming: PositionSize::ZERO,
                },
                Step {
                    asset: underlying,
                    held: PositionSize::ZERO,
                    incoming: quantity.to_position_size(),
                },
            ]
        }
        Side::Sell => [
            Step {
                asset: underlying,
                held: quantity.to_position_size(),
                incoming: PositionSize::ZERO,
            },
            Step {
                asset: settlement,
                held: (-notional).max(PositionSize::ZERO),
                incoming: notional.max(PositionSize::ZERO),
            },
        ],
    };
    Ok((quantity, steps))
}

// Exact or rejected: the inverse operation must give back each operand.
fn exact_add(left: PositionSize, right: PositionSize) -> Result<PositionSize, RejectCode> {
    left.checked_add(right)
        .ok()
        .filter(|sum| {
            sum.checked_sub(left).ok() == Some(right) && sum.checked_sub(right).ok() == Some(left)
        })
        .ok_or(RejectCode::ArithmeticOverflow)
}

fn exact_sub(left: PositionSize, right: PositionSize) -> Result<PositionSize, RejectCode> {
    left.checked_sub(right)
        .ok()
        .filter(|difference| {
            difference.checked_add(right).ok() == Some(left)
                && left.checked_sub(*difference).ok() == Some(right)
        })
        .ok_or(RejectCode::ArithmeticOverflow)
}

// The order's net effect on one asset: incoming minus held over its steps.
fn net(steps: &[&Step]) -> Result<PositionSize, RejectCode> {
    steps.iter().try_fold(PositionSize::ZERO, |net, step| {
        exact_add(net, exact_sub(step.incoming, step.held)?)
    })
}

// The worst-case position on the side the order moves, against the
// pre-order slot; a zero net is not checked.
fn barrier(pre: Slot, steps: &[&Step], limit: PositionSize) -> Result<(), RejectCode> {
    let net = net(steps)?;
    if net.is_zero() {
        return Ok(());
    }
    let owned = exact_add(pre.available, pre.held)?;
    let exceeded = if net > PositionSize::ZERO {
        exact_add(exact_add(owned, pre.incoming.max(PositionSize::ZERO))?, net)? > limit
    } else {
        exact_add(exact_sub(owned, pre.held.max(PositionSize::ZERO))?, net)? < -limit
    };
    if exceeded {
        Err(RejectCode::PositionLimitExceeded)
    } else {
        Ok(())
    }
}

// The order's net effect on `asset` per unit of quantity at a positive
// price.
fn unit_net(side: Side, instrument: &Instrument, asset: &Asset, price: Price) -> Price {
    let one = px("1");
    let (underlying, settlement) = match side {
        Side::Buy => (one, price.checked_neg().expect("negated price")),
        Side::Sell => (one.checked_neg().expect("negated unit"), price),
    };
    let mut net = Price::ZERO;
    if instrument.underlying_asset() == asset {
        net = net.checked_add(underlying).expect("underlying unit net");
    }
    if instrument.settlement_asset() == asset {
        net = net.checked_add(settlement).expect("settlement unit net");
    }
    net
}

// `percent` of the remainder, rounded down to hundredths.
fn share(remaining: Quantity, percent: i64) -> Quantity {
    let exact = remaining
        .checked_mul_i64(percent)
        .and_then(|scaled| scaled.checked_div_i64(100))
        .expect("partial quantity");
    Quantity::from_str_rounded(&exact.to_string(), 2, RoundingStrategy::Down)
        .expect("partial quantity in hundredths")
}

// Whether `volume` at `price` derives its quantity without rounding.
fn is_exact(volume: Volume, price: Price) -> bool {
    if price.is_zero() {
        return true;
    }
    let quantity = volume.calculate_quantity(price).expect("derived quantity");
    let (dividend, divisor) = (volume.to_decimal(), price.to_decimal());
    // Exact iff dividend x 10^s / divisor is whole at the quotient's scale s;
    // only a rounded quotient drops s below the operands' scale gap.
    let Some(shift) =
        (quantity.to_decimal().scale() + divisor.scale()).checked_sub(dividend.scale())
    else {
        return false;
    };
    let modulus = divisor.mantissa().unsigned_abs();
    let remainder = (0..shift).fold(dividend.mantissa().unsigned_abs() % modulus, |rest, _| {
        rest * 10 % modulus
    });
    remainder == 0
}

struct Runner {
    policy: TestPolicy,
    model: Model,
    exclusions: &'static [Exclusion],
}

impl Runner {
    fn new(case: &Case) -> Self {
        let mut settings = settings(0);
        settings.set_global_limit_mode(case.mode);
        let mut limits = HashMap::new();
        for ((account_id, asset), (_, limit)) in universe().zip(&case.slots) {
            if let Some(limit) = limit {
                settings.set_position_limit(account_id, asset.clone(), Some(*limit));
                limits.insert((account_id, asset), limit.to_position_size());
            }
        }
        let policy = build_policy_from_settings(settings, None);
        let mut slots = HashMap::new();
        for ((account_id, asset), (available, _)) in universe().zip(&case.slots) {
            if !available.is_zero() {
                seed(&policy, account_id, asset.clone(), &available.to_string());
            }
            let slot = Slot {
                available: *available,
                ..Slot::ZERO
            };
            slots.insert((account_id, asset), slot);
        }
        Self {
            policy,
            model: Model {
                mode: case.mode,
                limits,
                slots,
                open: Vec::new(),
            },
            exclusions: case.exclusions,
        }
    }

    fn run(&mut self, op: &Op, step: usize) -> Result<(), TestCaseError> {
        match op {
            Op::Order {
                amount: TradeAmount::Volume(volume),
                price,
                ..
            } if self.excludes(*volume, *price) => {}
            Op::Order {
                account_id,
                instrument,
                side,
                amount,
                price,
                commit,
            } => {
                let order = make_order(
                    *account_id,
                    instrument.clone(),
                    *side,
                    *amount,
                    Some(*price),
                );
                self.order(&order, *commit, step)?;
            }
            Op::BoundaryOrder {
                account_id,
                instrument,
                side,
                price,
                leg,
                delta,
                commit,
            } => {
                let quantity =
                    self.boundary_quantity(*account_id, instrument, *side, *price, *leg, *delta);
                if let Some(quantity) = quantity {
                    let order = make_order(
                        *account_id,
                        instrument.clone(),
                        *side,
                        TradeAmount::Quantity(quantity),
                        Some(*price),
                    );
                    self.order(&order, *commit, step)?;
                }
            }
            Op::Fill {
                order,
                quantity,
                price,
                fee,
                is_final,
            } => {
                if !self.model.open.is_empty() {
                    let index = order.index(self.model.open.len());
                    self.fill(index, *quantity, *price, fee.as_ref(), *is_final, step)?;
                }
            }
            Op::Cancel { order } => {
                if !self.model.open.is_empty() {
                    self.cancel(order.index(self.model.open.len()), step)?;
                }
            }
            Op::FeeOnly {
                account_id,
                instrument,
                side,
                fee,
            } => {
                let report = fee_only_report(
                    *account_id,
                    instrument.clone(),
                    *side,
                    "0",
                    false,
                    fee.clone(),
                );
                self.report(&report, step)?;
                self.model.fee(*account_id, fee);
            }
        }
        self.assert_matches_model(step)
    }

    fn order(&mut self, order: &TestOrder, commit: bool, step: usize) -> Result<(), TestCaseError> {
        let before = self.snapshot();
        let dry = dry_run_check(&self.policy, order);
        prop_assert_eq!(
            self.snapshot(),
            before,
            "dry run moved holdings for {:?} at step {}",
            order,
            step
        );
        let mut mutations = Mutations::new();
        let real = pre_trade_full(&self.policy, order, &mut mutations);
        match (&dry, &real) {
            (Ok(Some(dry)), Ok(real)) => {
                prop_assert_eq!(
                    &dry.lock_prices,
                    &real.lock_prices,
                    "dry-run lock prices for {:?} at step {}",
                    order,
                    step
                );
                prop_assert_eq!(
                    &dry.account_adjustments,
                    &real.account_adjustments,
                    "dry-run outcome for {:?} at step {}",
                    order,
                    step
                );
            }
            (Err(dry), Err(real)) => prop_assert_eq!(
                dry,
                real,
                "dry-run rejects for {:?} at step {}",
                order,
                step
            ),
            _ => prop_assert!(
                false,
                "dry run {:?} differs from real check {:?} for {:?} at step {}",
                dry,
                real,
                order,
                step
            ),
        }
        let admission = self.model.admit(order);
        let verdict = real.as_ref().map(|_| ()).map_err(|rejects| {
            rejects
                .iter()
                .map(|reject| (reject.code, reject.scope.clone()))
                .collect::<Vec<_>>()
        });
        let expected = admission
            .as_ref()
            .map(|_| ())
            .map_err(|code| vec![(*code, RejectScope::Order)]);
        prop_assert_eq!(
            verdict,
            expected,
            "verdict differs from model for {:?} at step {}",
            order,
            step
        );
        if let (Ok(real), Ok(admission)) = (&real, &admission) {
            prop_assert_eq!(
                real.lock_prices.as_slice(),
                &[admission.order.lock],
                "lock prices differ from model for {:?} at step {}",
                order,
                step
            );
        }
        match admission {
            Ok(admission) if commit => {
                prop_assert!(
                    !mutations.commit_all().failed(),
                    "commit failed at step {}",
                    step
                );
                self.model.commit(admission);
            }
            Ok(_) | Err(_) => prop_assert!(
                !mutations.rollback_all().callback_failed(),
                "rollback failed at step {}",
                step
            ),
        }
        Ok(())
    }

    // The quantity whose projection on a limited leg asset lands on the
    // limit moved outward by `delta`; none when no leg asset is limited, the
    // quantity would not be positive, it is inexact while the case excludes
    // inexact quantities, or it would exceed `MAX_QUANTITY`, past which
    // boundary orders and off-lock fills could compound it until a fill
    // notional overflows.
    fn boundary_quantity(
        &self,
        account_id: AccountId,
        instrument: &Instrument,
        side: Side,
        price: Price,
        leg: Index,
        delta: PositionSize,
    ) -> Option<Quantity> {
        let mut assets = vec![instrument.underlying_asset().clone()];
        if instrument.settlement_asset() != instrument.underlying_asset() {
            assets.push(instrument.settlement_asset().clone());
        }
        let candidates: Vec<(Asset, PositionSize, Price)> = assets
            .into_iter()
            .filter_map(|asset| {
                let limit = *self.model.limits.get(&(account_id, asset.clone()))?;
                let unit = unit_net(side, instrument, &asset, price);
                (!unit.is_zero()).then_some((asset, limit, unit))
            })
            .collect();
        if candidates.is_empty() {
            return None;
        }
        let (asset, limit, unit) = leg.get(&candidates);
        let slot = self.model.slot(&(account_id, asset.clone()));
        let owned = slot.available.checked_add(slot.held).expect("owned");
        let edge = limit.checked_add(delta).expect("boundary");
        let gap = if *unit > Price::ZERO {
            edge.checked_sub(owned)
                .and_then(|gap| gap.checked_sub(slot.incoming.max(PositionSize::ZERO)))
        } else {
            (-edge)
                .checked_sub(owned)
                .and_then(|gap| gap.checked_add(slot.held.max(PositionSize::ZERO)))
        }
        .expect("gap to the boundary");
        if gap.is_zero() || (gap > PositionSize::ZERO) != (*unit > Price::ZERO) {
            return None;
        }
        let magnitude = gap.max(-gap);
        let volume = vol(&magnitude.to_string());
        let quantity = volume.calculate_quantity(*unit).expect("boundary quantity");
        (quantity <= qty(&hundredths(MAX_QUANTITY)) && !self.excludes(volume, *unit))
            .then_some(quantity)
    }

    // Whether the case leaves out the quantity `volume` derives at `price`,
    // which skips the order carrying it.
    fn excludes(&self, volume: Volume, price: Price) -> bool {
        self.exclusions.contains(&Exclusion::InexactQuantities) && !is_exact(volume, price)
    }

    fn fill(
        &mut self,
        index: usize,
        quantity: FillQuantity,
        price: TradePrice,
        fee: Option<&MonetaryAmount>,
        is_final: bool,
        step: usize,
    ) -> Result<(), TestCaseError> {
        let open = self.model.open[index].clone();
        let filled = match quantity {
            FillQuantity::Full => open.remaining,
            FillQuantity::Partial(percent) => share(open.remaining, percent),
            FillQuantity::Over(extra) => open
                .remaining
                .checked_add(extra)
                .expect("over-fill quantity"),
        };
        if filled.is_zero() {
            return Ok(());
        }
        let price = match price {
            TradePrice::AtLock => open.lock,
            TradePrice::Other(price) => price,
        };
        let remainder = if filled < open.remaining {
            open.remaining.checked_sub(filled).expect("remainder")
        } else {
            Quantity::ZERO
        };
        let mut report = make_report(
            open.account_id,
            open.instrument.clone(),
            open.side,
            Some(Trade {
                price,
                quantity: filled,
            }),
            if is_final { remainder } else { Quantity::ZERO },
            is_final,
            Some(pre_trade_lock(open.lock)),
        );
        report.fee = fee.cloned();
        self.report(&report, step)?;
        self.model.trade(&open, filled, price);
        if let Some(fee) = fee {
            self.model.fee(open.account_id, fee);
        }
        if !is_final {
            self.model.open[index].remaining = remainder;
            return Ok(());
        }
        self.model.open.remove(index);
        if !remainder.is_zero() {
            self.model.release(&open, remainder);
        }
        Ok(())
    }

    fn cancel(&mut self, index: usize, step: usize) -> Result<(), TestCaseError> {
        let open = self.model.open.remove(index);
        let report = make_report(
            open.account_id,
            open.instrument.clone(),
            open.side,
            None,
            open.remaining,
            true,
            Some(pre_trade_lock(open.lock)),
        );
        self.report(&report, step)?;
        if !open.remaining.is_zero() {
            self.model.release(&open, open.remaining);
        }
        Ok(())
    }

    fn report(&self, report: &TestReport, step: usize) -> Result<(), TestCaseError> {
        let blocks = report_blocks(&self.policy, report);
        prop_assert!(
            blocks.is_empty(),
            "account blocks {:?} at step {}",
            blocks,
            step
        );
        Ok(())
    }

    fn snapshot(&self) -> Vec<Option<Holdings>> {
        universe()
            .map(|(account_id, asset)| holdings_of(&self.policy, account_id, &asset))
            .collect()
    }

    fn holdings(&self, key: &(AccountId, Asset)) -> Holdings {
        holdings_of(&self.policy, key.0, &key.1).unwrap_or_else(Holdings::zero)
    }

    fn assert_matches_model(&self, step: usize) -> Result<(), TestCaseError> {
        for key in universe() {
            let holdings = self.holdings(&key);
            let actual = Slot {
                available: holdings.available(),
                held: holdings.held(),
                incoming: holdings.incoming(),
            };
            prop_assert_eq!(
                actual,
                self.model.slot(&key),
                "account {} asset {} after step {}",
                key.0,
                key.1,
                step
            );
        }
        Ok(())
    }

    fn assert_within_limits(&self, step: usize) -> Result<(), TestCaseError> {
        for key in universe() {
            let Some(limit) = self.model.limits.get(&key) else {
                continue;
            };
            let holdings = self.holdings(&key);
            let owned = holdings
                .available()
                .checked_add(holdings.held())
                .expect("owned position");
            prop_assert!(
                -*limit <= owned && owned <= *limit,
                "account {} asset {} owns {} beyond limit {} after step {}",
                key.0,
                key.1,
                owned,
                limit,
                step
            );
        }
        Ok(())
    }

    fn assert_released(&self) -> Result<(), TestCaseError> {
        for key in universe() {
            let holdings = self.holdings(&key);
            prop_assert!(
                holdings.held().is_zero() && holdings.incoming().is_zero(),
                "account {} asset {} keeps held {} and incoming {} after every order \
                 is final",
                key.0,
                key.1,
                holdings.held(),
                holdings.incoming()
            );
        }
        Ok(())
    }
}

fn sequences_match_the_model(case: &Case) -> Result<(), TestCaseError> {
    let mut runner = Runner::new(case);
    for (step, op) in case.ops.iter().enumerate() {
        runner.run(op, step)?;
    }
    Ok(())
}

fn positions_stay_within_limits(case: &Case) -> Result<(), TestCaseError> {
    let mut runner = Runner::new(case);
    for (step, op) in case.ops.iter().enumerate() {
        runner.run(op, step)?;
        runner.assert_within_limits(step)?;
    }
    Ok(())
}

fn every_reservation_is_released(case: &Case) -> Result<(), TestCaseError> {
    let mut runner = Runner::new(case);
    for (step, op) in case.ops.iter().enumerate() {
        runner.run(op, step)?;
    }
    let step = case.ops.len();
    while !runner.model.open.is_empty() {
        runner.cancel(0, step)?;
        runner.assert_matches_model(step)?;
    }
    runner.assert_released()
}

proptest! {
    #![proptest_config(config(CASES))]

    // Over arbitrary sequences the dry run equals the real check, the
    // verdict equals the model's, and holdings equal the model.
    #[test]
    #[ignore = "inexact volume quantity v/p rounds a reservation write; rollback misses the slot"]
    fn spot_funds_sequences_match_the_model(case in case(Profile::Arbitrary, &[])) {
        sequences_match_the_model(&case)?;
    }

    // Leaves out orders whose derived quantity is a rounded quotient: its
    // reservation write rounds silently, and the rollback of a rejected or
    // rolled-back order then misses the slot. An exact volume or boundary
    // quantity stays in. The exclusion is removed when that defect is fixed.
    #[test]
    fn spot_funds_sequences_match_the_model_with_exact_quantities(
        case in case(Profile::Arbitrary, &[Exclusion::InexactQuantities])
    ) {
        sequences_match_the_model(&case)?;
    }

    #[test]
    #[ignore = "negative-price buy projects no settlement inflow; inexact v/p breaks slot rollback"]
    fn exact_execution_keeps_positions_within_limits(case in case(Profile::ExactExecution, &[])) {
        positions_stay_within_limits(&case)?;
    }

    // Leaves out orders whose derived quantity is a rounded quotient, whose
    // reservation write rounds silently so a rollback misses the slot, and
    // buys at a negative price, which project no settlement inflow, so a
    // fill bypasses a limit on the settlement asset. An exact volume or
    // boundary quantity stays in. Each exclusion is removed when its defect
    // is fixed.
    #[test]
    fn exact_execution_keeps_positions_within_limits_with_exact_quantities_no_negative_price_buys(
        case in case(
            Profile::ExactExecution,
            &[Exclusion::InexactQuantities, Exclusion::NegativePriceBuys],
        )
    ) {
        positions_stay_within_limits(&case)?;
    }

    #[test]
    #[ignore = "inexact v/p leaves residual held once orders are final and breaks slot rollback"]
    fn final_orders_release_every_reservation(case in case(Profile::WithinQuantity, &[])) {
        every_reservation_is_released(&case)?;
    }

    // Leaves out orders whose derived quantity is a rounded quotient, which
    // leave held behind once every order is final: a volume buy releases
    // lock price times the rounded v/p, not the held volume; a reservation
    // write rounds and its rollback misses the slot; reservations sharing a
    // slot round on the way in but each release subtracts its exact amount.
    // An exact volume or boundary quantity stays in. The exclusion is
    // removed when those defects are fixed.
    #[test]
    fn final_orders_release_every_reservation_with_exact_quantities(
        case in case(Profile::WithinQuantity, &[Exclusion::InexactQuantities])
    ) {
        every_reservation_is_released(&case)?;
    }
}

#[test]
fn is_exact_tells_rounded_quotients_from_exact_ones() {
    for (volume, price, exact) in [
        ("1", "3", false),
        ("1", "0.75", false),
        // Terminates but needs 30 fractional digits, and the rounded quotient
        // multiplies back onto the volume.
        ("0.0000000000000000000000000001", "0.8", false),
        // Rounds onto a scale below the operands' scale gap.
        ("0.0000000000000000000000000019", "2", false),
        ("1", "-3", false),
        ("1", "0", true),
        ("1", "-8", true),
        ("1", "8", true),
        ("100", "1", true),
        ("0.05631351470947265625", "4", true),
    ] {
        assert_eq!(
            is_exact(vol(volume), px(price)),
            exact,
            "{volume} / {price}"
        );
    }
}
