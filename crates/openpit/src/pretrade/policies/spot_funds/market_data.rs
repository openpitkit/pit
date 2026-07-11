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

//! Market-data wiring and runtime settings for
//! [`SpotFundsPolicy`](super::SpotFundsPolicy).
//!
//! The slippage / pricing-source / override cascade is runtime-updatable and
//! lives in [`SpotFundsSettings`], stored behind the policy's settings cell.
//! The market-data service handle in [`SpotFundsMarketData`] is fixed for the
//! policy's lifetime and is *not* part of the settings.

use std::collections::HashMap;
use std::fmt::{Display, Formatter};

use super::{
    SpotFundsLimitMode, SpotFundsPnlBoundsAccountBarrier, SpotFundsPnlBoundsAccountBarrierUpdate,
    SpotFundsPnlBoundsAccountGroupBarrier, SpotFundsPnlBoundsBarrier,
};
use crate::core::instrument::Instrument;
use crate::marketdata::{
    AccountInfo, InstrumentId, MarketDataService, MarketDataSync, Quote, QuoteResolution,
};
use crate::param::{AccountGroupId, AccountId, Asset, Pnl, Price};

use super::super::pnl_bounds;
use super::market_order_pricer::WithSlippage;

/// Upper bound (inclusive) on any slippage value, in basis points.
const MAX_SLIPPAGE_BPS: u16 = 10_000;

// ─── SpotFundsConfigError ─────────────────────────────────────────────────────

/// Error returned when building or updating [`SpotFundsSettings`].
#[non_exhaustive]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SpotFundsConfigError {
    /// The slippage value is out of the accepted range (0..=10 000 bps).
    SlippageOutOfRange {
        /// The bps value that triggered the error.
        bps: u16,
    },
    /// A P&L bounds barrier has neither lower nor upper bound.
    NoPnlBoundsConfigured {
        /// Account currency whose barrier is empty.
        account_currency: Asset,
    },
    /// A P&L-bounds policy was built without any barriers.
    NoPnlBarriersConfigured,
}

impl Display for SpotFundsConfigError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SlippageOutOfRange { bps } => {
                write!(
                    f,
                    "slippage {bps} bps is out of range (must be <= 10 000 bps)"
                )
            }
            Self::NoPnlBoundsConfigured { account_currency } => {
                write!(
                    f,
                    "spot-funds P&L bounds for account currency {account_currency} \
                     must configure at least one bound"
                )
            }
            Self::NoPnlBarriersConfigured => {
                write!(f, "spot funds P&L bounds require at least one barrier")
            }
        }
    }
}

impl std::error::Error for SpotFundsConfigError {}

/// Validates a slippage value against the accepted range.
fn check_slippage_bps(bps: u16) -> Result<(), SpotFundsConfigError> {
    if bps > MAX_SLIPPAGE_BPS {
        return Err(SpotFundsConfigError::SlippageOutOfRange { bps });
    }
    Ok(())
}

// ─── SpotFundsPriceError ──────────────────────────────────────────────────────

/// Error returned by the effective-price helpers on
/// [`SpotFundsMarketData`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum SpotFundsPriceError {
    /// No usable quote is available for the instrument.
    QuoteUnavailable,
    /// The effective price could not be computed (decimal overflow or
    /// non-positive result).
    CalculationFailed,
}

// ─── SpotFundsPricingSource ───────────────────────────────────────────────────

/// Source the policy uses to derive the base price for a market order
/// before slippage is applied.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SpotFundsPricingSource {
    /// Use the stored `mark` price. The order would notionally be priced at
    /// `mark * (1 + slippage)` for buys and `mark * (1 - slippage)` for
    /// sells.
    ///
    /// Returns `SpotFundsPriceError::QuoteUnavailable` if the stored quote
    /// has no `mark` field.
    #[default]
    Mark,
    /// Use the side of the book that the order would cross: `ask` for buys
    /// and `bid` for sells. Slippage is added as a cushion on top
    /// (`ask * (1 + slippage)` / `bid * (1 - slippage)`).
    ///
    /// Returns `SpotFundsPriceError::QuoteUnavailable` if the relevant
    /// side of the book is missing from the stored quote - no implicit
    /// fallback to `mark`.
    BookTop,
}

// ─── SpotFundsOverride ────────────────────────────────────────────────────────

/// Override *value* applied at a slippage-cascade target.
///
/// Holds the slippage knob a [`SpotFundsOverrideTarget`] applies to the
/// instrument/account/account group it selects. Every field is optional: a `None`
/// means "fall back to the next tier of the cascade and ultimately the global
/// setting configured on the settings". The struct is named to flag its role as
/// the container for override knobs - future settings land here as additional
/// `Option<_>` fields, and call sites that initialise it via
/// `..Default::default()` keep compiling.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SpotFundsOverride {
    /// Slippage in basis points applied at the target. `None` defers to the
    /// next tier of the cascade (and ultimately the global `slippage_bps`
    /// configured on [`SpotFundsSettings::new`]).
    pub slippage_bps: Option<u16>,
}

// ─── SpotFundsOverrideTarget ──────────────────────────────────────────────────

/// Selects which accounts a [`SpotFundsOverride`] applies to within the
/// slippage resolution cascade.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SpotFundsOverrideTarget {
    /// Instrument-level default: applies when no account- or account-group-scoped
    /// override matches the order's account.
    Instrument(InstrumentId),
    /// Applies to the instrument only for this exact account (highest priority).
    InstrumentAccount(InstrumentId, AccountId),
    /// Applies to the instrument only for accounts in this account group.
    InstrumentAccountGroup(InstrumentId, AccountGroupId),
}

// ─── SpotFundsSettings ────────────────────────────────────────────────────────

/// Runtime-updatable settings of [`SpotFundsPolicy`](super::SpotFundsPolicy).
///
/// Carries the slippage / pricing-source / override cascade and the funds
/// limit-mode cascade. Slippage resolves per order along three override scopes -
/// per `(instrument, account_id)`, per `(instrument, account_group_id)`, and per
/// `instrument` - falling back to the global slippage. The limit mode resolves
/// per order along an account-centric cascade - per `account_id`, per
/// `account_group_id`, then global - and decides whether a reservation that
/// exceeds available funds is rejected ([`SpotFundsLimitMode::Enforce`]) or
/// merely tracked ([`SpotFundsLimitMode::TrackOnly`]). The validated maps are
/// precomputed here so hot-path reads through the policy's settings cell
/// allocate nothing and never recompute.
///
/// Built via [`SpotFundsSettings::new`] and handed to
/// [`SpotFundsPolicy::new`](super::SpotFundsPolicy::new); the slippage knobs and
/// the limit mode are then mutable at runtime through the setters.
#[derive(Clone, Debug)]
pub struct SpotFundsSettings {
    account_overrides: HashMap<(InstrumentId, AccountId), WithSlippage>,
    account_group_overrides: HashMap<(InstrumentId, AccountGroupId), WithSlippage>,
    instrument_overrides: HashMap<InstrumentId, WithSlippage>,
    account_limit_modes: HashMap<AccountId, SpotFundsLimitMode>,
    account_group_limit_modes: HashMap<AccountGroupId, SpotFundsLimitMode>,
    pnl_global_barriers: HashMap<Asset, SpotFundsPnlBoundsBarrier>,
    pnl_account_group_barriers:
        HashMap<AccountGroupId, HashMap<Asset, SpotFundsPnlBoundsAccountGroupBarrier>>,
    pnl_account_barriers:
        HashMap<AccountId, HashMap<Asset, SpotFundsPnlBoundsAccountBarrierUpdate>>,
    initial_pnl: HashMap<(AccountId, Asset), Pnl>,
    global_pricer: WithSlippage,
    pricing_source: SpotFundsPricingSource,
    global_limit_mode: SpotFundsLimitMode,
}

impl SpotFundsSettings {
    /// Builds the cascade from the full set of configuration parameters.
    ///
    /// Pass `SpotFundsPricingSource::Mark` for the default source and `[]` (or
    /// any empty iterator) for no overrides. The instance starts with the
    /// default policy group; assign a tag via
    /// [`SpotFundsPolicy::with_policy_group_id`](super::SpotFundsPolicy::with_policy_group_id).
    ///
    /// Each `(target, override)` pair places a [`SpotFundsOverride`] into one
    /// of three slippage scopes selected by [`SpotFundsOverrideTarget`]: per
    /// `(instrument, account_id)`, per `(instrument, account_group_id)`, or
    /// per-instrument default. The slippage for an order resolves
    /// account -> account group -> instrument -> global. An override whose
    /// `slippage_bps` is `None` is ignored (the cascade falls through to the
    /// next tier).
    ///
    /// Returns [`SpotFundsConfigError::SlippageOutOfRange`] when
    /// `slippage_bps > 10_000` or when any override carries a `slippage_bps`
    /// above the same bound.
    ///
    /// The funds limit-mode cascade starts at its defaults: the global mode is
    /// [`SpotFundsLimitMode::Enforce`] and no per-account or per-account-group
    /// override is set. Configure it through
    /// [`set_global_limit_mode`](Self::set_global_limit_mode),
    /// [`set_account_limit_mode`](Self::set_account_limit_mode) and
    /// [`set_account_group_limit_mode`](Self::set_account_group_limit_mode).
    pub fn new<Overrides>(
        slippage_bps: u16,
        pricing_source: SpotFundsPricingSource,
        overrides: Overrides,
    ) -> Result<Self, SpotFundsConfigError>
    where
        Overrides: IntoIterator<Item = (SpotFundsOverrideTarget, SpotFundsOverride)>,
    {
        check_slippage_bps(slippage_bps)?;
        let global_pricer = WithSlippage::new(slippage_bps);
        let mut account_overrides = HashMap::new();
        let mut account_group_overrides = HashMap::new();
        let mut instrument_overrides = HashMap::new();
        for (target, ovr) in overrides {
            let Some(bps) = ovr.slippage_bps else {
                continue;
            };
            check_slippage_bps(bps)?;
            let pricer = WithSlippage::new(bps);
            match target {
                SpotFundsOverrideTarget::Instrument(instrument_id) => {
                    instrument_overrides.insert(instrument_id, pricer);
                }
                SpotFundsOverrideTarget::InstrumentAccount(instrument_id, account_id) => {
                    account_overrides.insert((instrument_id, account_id), pricer);
                }
                SpotFundsOverrideTarget::InstrumentAccountGroup(
                    instrument_id,
                    account_group_id,
                ) => {
                    account_group_overrides.insert((instrument_id, account_group_id), pricer);
                }
            }
        }
        Ok(Self {
            account_overrides,
            account_group_overrides,
            instrument_overrides,
            account_limit_modes: HashMap::new(),
            account_group_limit_modes: HashMap::new(),
            pnl_global_barriers: HashMap::new(),
            pnl_account_group_barriers: HashMap::new(),
            pnl_account_barriers: HashMap::new(),
            initial_pnl: HashMap::new(),
            global_pricer,
            pricing_source,
            global_limit_mode: SpotFundsLimitMode::Enforce,
        })
    }

    /// Replaces the global slippage applied when no override matches.
    ///
    /// Returns [`SpotFundsConfigError::SlippageOutOfRange`] when
    /// `slippage_bps > 10_000`; the prior value is left unchanged on error.
    pub fn set_global_slippage_bps(
        &mut self,
        slippage_bps: u16,
    ) -> Result<(), SpotFundsConfigError> {
        check_slippage_bps(slippage_bps)?;
        self.global_pricer = WithSlippage::new(slippage_bps);
        Ok(())
    }

    /// Sets the source used to derive the base price before slippage.
    pub fn set_pricing_source(&mut self, pricing_source: SpotFundsPricingSource) {
        self.pricing_source = pricing_source;
    }

    /// Replaces the global funds limit mode applied when no per-account or
    /// per-account-group override matches.
    ///
    /// [`SpotFundsLimitMode::TrackOnly`] disables the insufficient-funds reject
    /// for orders that resolve to the global tier; [`SpotFundsLimitMode::Enforce`]
    /// restores the gating behavior.
    pub fn set_global_limit_mode(&mut self, mode: SpotFundsLimitMode) {
        self.global_limit_mode = mode;
    }

    /// Inserts or clears the funds limit-mode override for one account.
    ///
    /// `Some(mode)` pins the account to `mode`, winning over the account-group
    /// and global tiers. `None` clears any previous override, so the cascade
    /// falls through to the account-group tier and ultimately the global mode.
    pub fn set_account_limit_mode(
        &mut self,
        account_id: AccountId,
        mode: Option<SpotFundsLimitMode>,
    ) {
        pnl_bounds::set_or_clear(&mut self.account_limit_modes, account_id, mode);
    }

    /// Inserts or clears the funds limit-mode override for one account group.
    ///
    /// `Some(mode)` applies `mode` to every account in the group that has no
    /// per-account override. `None` clears any previous override, so the cascade
    /// falls through to the global mode for accounts in the group.
    pub fn set_account_group_limit_mode(
        &mut self,
        account_group_id: AccountGroupId,
        mode: Option<SpotFundsLimitMode>,
    ) {
        pnl_bounds::set_or_clear(&mut self.account_group_limit_modes, account_group_id, mode);
    }

    /// Replaces the global account-currency P&L bounds.
    ///
    /// Global P&L bounds apply to every account whose account currency matches
    /// and that has no more specific account-group or account override.
    pub fn set_pnl_global_barriers(
        &mut self,
        barriers: impl IntoIterator<Item = SpotFundsPnlBoundsBarrier>,
    ) -> Result<(), SpotFundsConfigError> {
        let barriers = collect_pnl_global_barriers(barriers)?;
        self.pnl_global_barriers = barriers;
        Ok(())
    }

    /// Replaces account-group account-currency P&L bounds.
    pub fn set_pnl_account_group_barriers(
        &mut self,
        barriers: impl IntoIterator<Item = SpotFundsPnlBoundsAccountGroupBarrier>,
    ) -> Result<(), SpotFundsConfigError> {
        let barriers = collect_pnl_account_group_barriers(barriers)?;
        self.pnl_account_group_barriers = barriers;
        Ok(())
    }

    /// Replaces account-specific account-currency P&L bounds without changing
    /// accumulated P&L.
    pub fn set_pnl_account_barriers(
        &mut self,
        barriers: impl IntoIterator<Item = SpotFundsPnlBoundsAccountBarrierUpdate>,
    ) -> Result<(), SpotFundsConfigError> {
        let barriers = collect_pnl_account_barrier_updates(barriers)?;
        self.pnl_account_barriers = barriers;
        Ok(())
    }

    /// Sets account-specific account-currency P&L bounds and records
    /// construction-time P&L seeds, consuming and returning the settings.
    ///
    /// This is a build-time-only builder step: it takes `self` by value so it
    /// cannot be called from the runtime configuration closure, which only ever
    /// hands out a `&mut SpotFundsSettings` (moving out of that borrow does not
    /// compile). Seeds must therefore be supplied before the settings are handed
    /// to [`SpotFundsPolicy::new`](super::SpotFundsPolicy::new), which is the only
    /// place they are consumed. Runtime configuration uses
    /// [`Self::set_pnl_account_barriers`], which carries no seed and cannot reset
    /// accumulated P&L.
    pub fn with_initial_pnl_account_barriers(
        mut self,
        barriers: impl IntoIterator<Item = SpotFundsPnlBoundsAccountBarrier>,
    ) -> Result<Self, SpotFundsConfigError> {
        let mut account = HashMap::new();
        let mut initial_pnl = HashMap::new();
        for barrier in barriers {
            validate_pnl_bounds(&barrier.barrier)?;
            let account_id = barrier.account_id;
            let account_currency = barrier.barrier.account_currency.clone();
            initial_pnl.insert((account_id, account_currency.clone()), barrier.initial_pnl);
            account
                .entry(account_id)
                .or_insert_with(HashMap::new)
                .insert(
                    account_currency,
                    SpotFundsPnlBoundsAccountBarrierUpdate {
                        barrier: barrier.barrier,
                        account_id,
                    },
                );
        }
        self.pnl_account_barriers = account;
        self.initial_pnl = initial_pnl;
        Ok(self)
    }

    /// Installs the complete construction-time P&L-bounds configuration.
    ///
    /// # Errors
    ///
    /// Returns [`SpotFundsConfigError::NoPnlBarriersConfigured`] when all
    /// three inputs are empty. Individual barriers retain the bounds
    /// validation performed by the corresponding settings operations.
    pub fn with_pnl_barriers(
        mut self,
        global_barriers: impl IntoIterator<Item = SpotFundsPnlBoundsBarrier>,
        account_group_barriers: impl IntoIterator<Item = SpotFundsPnlBoundsAccountGroupBarrier>,
        account_barriers: impl IntoIterator<Item = SpotFundsPnlBoundsAccountBarrier>,
    ) -> Result<Self, SpotFundsConfigError> {
        self.set_pnl_global_barriers(global_barriers)?;
        self.set_pnl_account_group_barriers(account_group_barriers)?;
        self = self.with_initial_pnl_account_barriers(account_barriers)?;
        if self.pnl_global_barriers.is_empty()
            && self.pnl_account_group_barriers.is_empty()
            && self.pnl_account_barriers.is_empty()
        {
            return Err(SpotFundsConfigError::NoPnlBarriersConfigured);
        }
        Ok(self)
    }

    pub(super) fn take_initial_pnl(&mut self) -> HashMap<(AccountId, Asset), Pnl> {
        std::mem::take(&mut self.initial_pnl)
    }

    pub(super) fn pnl_barrier_for(
        &self,
        account_id: AccountId,
        account_group_id: Option<AccountGroupId>,
        account_currency: &Asset,
    ) -> Option<&SpotFundsPnlBoundsBarrier> {
        if let Some(barrier) = self
            .pnl_account_barriers
            .get(&account_id)
            .and_then(|m| m.get(account_currency))
        {
            return Some(&barrier.barrier);
        }
        if let Some(account_group_id) = account_group_id {
            if let Some(barrier) = self
                .pnl_account_group_barriers
                .get(&account_group_id)
                .and_then(|m| m.get(account_currency))
            {
                return Some(&barrier.barrier);
            }
        }
        self.pnl_global_barriers.get(account_currency)
    }

    /// Reads the global funds limit mode applied when no override matches.
    #[cfg(test)]
    pub(super) fn global_limit_mode(&self) -> SpotFundsLimitMode {
        self.global_limit_mode
    }

    /// Inserts or replaces a slippage override at the given cascade target.
    ///
    /// A `slippage_bps` of `None` clears any override previously set at the
    /// target, so the cascade falls through to the next tier. Returns
    /// [`SpotFundsConfigError::SlippageOutOfRange`] when the value exceeds
    /// 10 000 bps; the prior override is left unchanged on error.
    pub fn set_override(
        &mut self,
        target: SpotFundsOverrideTarget,
        ovr: SpotFundsOverride,
    ) -> Result<(), SpotFundsConfigError> {
        if let Some(bps) = ovr.slippage_bps {
            check_slippage_bps(bps)?;
        }
        let pricer = ovr.slippage_bps.map(WithSlippage::new);
        match target {
            SpotFundsOverrideTarget::Instrument(instrument_id) => {
                pnl_bounds::set_or_clear(&mut self.instrument_overrides, instrument_id, pricer);
            }
            SpotFundsOverrideTarget::InstrumentAccount(instrument_id, account_id) => {
                pnl_bounds::set_or_clear(
                    &mut self.account_overrides,
                    (instrument_id, account_id),
                    pricer,
                );
            }
            SpotFundsOverrideTarget::InstrumentAccountGroup(instrument_id, account_group_id) => {
                pnl_bounds::set_or_clear(
                    &mut self.account_group_overrides,
                    (instrument_id, account_group_id),
                    pricer,
                );
            }
        }
        Ok(())
    }

    /// Selects the slippage pricer for an order via the resolution cascade.
    fn pricer_for(
        &self,
        instrument_id: InstrumentId,
        account_id: AccountId,
        account_info: &impl AccountInfo,
    ) -> &WithSlippage {
        if let Some(p) = self.account_overrides.get(&(instrument_id, account_id)) {
            return p;
        }
        if let Some(account_group_id) = account_info.group() {
            if let Some(p) = self
                .account_group_overrides
                .get(&(instrument_id, account_group_id))
            {
                return p;
            }
        }
        if let Some(p) = self.instrument_overrides.get(&instrument_id) {
            return p;
        }
        &self.global_pricer
    }

    /// Resolves the funds limit mode for an order via the account-centric
    /// cascade: per-account override, then per-account-group override, then the
    /// global mode. Mirrors the structure of [`Self::pricer_for`].
    pub(super) fn limit_mode_for(
        &self,
        account_id: AccountId,
        account_info: &impl AccountInfo,
    ) -> SpotFundsLimitMode {
        if let Some(mode) = self.account_limit_modes.get(&account_id) {
            return *mode;
        }
        if let Some(account_group_id) = account_info.group() {
            if let Some(mode) = self.account_group_limit_modes.get(&account_group_id) {
                return *mode;
            }
        }
        self.global_limit_mode
    }

    /// Raw quote field used as the base for buy-side pricing before slippage
    /// is applied; `None` when the relevant field is missing from the quote.
    fn pricing_base_for_buy(&self, quote: &Quote) -> Option<Price> {
        match self.pricing_source {
            SpotFundsPricingSource::Mark => quote.mark,
            SpotFundsPricingSource::BookTop => quote.ask,
        }
    }

    /// Raw quote field used as the base for sell-side pricing before slippage
    /// is applied; `None` when the relevant field is missing from the quote.
    fn pricing_base_for_sell(&self, quote: &Quote) -> Option<Price> {
        match self.pricing_source {
            SpotFundsPricingSource::Mark => quote.mark,
            SpotFundsPricingSource::BookTop => quote.bid,
        }
    }

    /// Effective buy price for `quote` under the resolved slippage tier.
    pub(super) fn effective_buy_price(
        &self,
        quote: &Quote,
        instrument_id: InstrumentId,
        account_id: AccountId,
        account_info: &impl AccountInfo,
    ) -> Result<Price, SpotFundsPriceError> {
        let base = self
            .pricing_base_for_buy(quote)
            .ok_or(SpotFundsPriceError::QuoteUnavailable)?;
        self.pricer_for(instrument_id, account_id, account_info)
            .effective_buy_price(base)
            .map_err(|_| SpotFundsPriceError::CalculationFailed)
    }

    /// Effective sell price for `quote` under the resolved slippage tier.
    pub(super) fn effective_sell_price(
        &self,
        quote: &Quote,
        instrument_id: InstrumentId,
        account_id: AccountId,
        account_info: &impl AccountInfo,
    ) -> Result<Price, SpotFundsPriceError> {
        let base = self
            .pricing_base_for_sell(quote)
            .ok_or(SpotFundsPriceError::QuoteUnavailable)?;
        self.pricer_for(instrument_id, account_id, account_info)
            .effective_sell_price(base)
            .map_err(|_| SpotFundsPriceError::CalculationFailed)
    }
}

fn validate_pnl_bounds(barrier: &SpotFundsPnlBoundsBarrier) -> Result<(), SpotFundsConfigError> {
    if !pnl_bounds::has_configured_bound(&barrier.lower_bound, &barrier.upper_bound) {
        return Err(SpotFundsConfigError::NoPnlBoundsConfigured {
            account_currency: barrier.account_currency.clone(),
        });
    }
    Ok(())
}

fn collect_pnl_global_barriers(
    barriers: impl IntoIterator<Item = SpotFundsPnlBoundsBarrier>,
) -> Result<HashMap<Asset, SpotFundsPnlBoundsBarrier>, SpotFundsConfigError> {
    let mut out = HashMap::new();
    for barrier in barriers {
        validate_pnl_bounds(&barrier)?;
        out.insert(barrier.account_currency.clone(), barrier);
    }
    Ok(out)
}

fn collect_pnl_account_group_barriers(
    barriers: impl IntoIterator<Item = SpotFundsPnlBoundsAccountGroupBarrier>,
) -> Result<
    HashMap<AccountGroupId, HashMap<Asset, SpotFundsPnlBoundsAccountGroupBarrier>>,
    SpotFundsConfigError,
> {
    let mut out = HashMap::new();
    for barrier in barriers {
        validate_pnl_bounds(&barrier.barrier)?;
        out.entry(barrier.account_group_id)
            .or_insert_with(HashMap::new)
            .insert(barrier.barrier.account_currency.clone(), barrier);
    }
    Ok(out)
}

fn collect_pnl_account_barrier_updates(
    barriers: impl IntoIterator<Item = SpotFundsPnlBoundsAccountBarrierUpdate>,
) -> Result<
    HashMap<AccountId, HashMap<Asset, SpotFundsPnlBoundsAccountBarrierUpdate>>,
    SpotFundsConfigError,
> {
    let mut out = HashMap::new();
    for barrier in barriers {
        validate_pnl_bounds(&barrier.barrier)?;
        out.entry(barrier.account_id)
            .or_insert_with(HashMap::new)
            .insert(barrier.barrier.account_currency.clone(), barrier);
    }
    Ok(out)
}

// ─── SpotFundsMarketData ──────────────────────────────────────────────────────

/// Market-data service handle for [`SpotFundsPolicy`](super::SpotFundsPolicy).
///
/// Wraps the shared [`MarketDataService`] handle the policy consults to price
/// market orders. The handle is fixed for the policy's lifetime; the slippage
/// and pricing cascade applied on top of the quotes lives in the
/// runtime-updatable [`SpotFundsSettings`].
///
/// Pass `None` to [`SpotFundsPolicy::new`](super::SpotFundsPolicy::new) to
/// disable market orders entirely (rejected with
/// [`crate::pretrade::RejectCode::UnsupportedOrderType`]).
pub struct SpotFundsMarketData<Sync: MarketDataSync> {
    pub(super) market_data: Sync::Shared<MarketDataService<Sync>>,
}

impl<Sync: MarketDataSync> SpotFundsMarketData<Sync> {
    /// Wraps the shared market-data service handle.
    pub fn new(market_data: Sync::Shared<MarketDataService<Sync>>) -> Self {
        Self { market_data }
    }

    /// Latest usable quote for `(instrument_id, account_id)` under the widest
    /// resolution; `None` when no usable quote is available.
    pub(super) fn quote(
        &self,
        instrument_id: InstrumentId,
        account_id: AccountId,
        account_info: &impl AccountInfo,
    ) -> Option<Quote> {
        self.market_data
            .get(
                instrument_id,
                account_id,
                account_info,
                QuoteResolution::AccountThenGroupThenDefault,
            )
            .ok()
    }

    pub(super) fn resolve(&self, instrument: &Instrument) -> Option<InstrumentId> {
        self.market_data.resolve(instrument)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;
    use crate::marketdata::{MarketDataBuilder, Quote, QuoteTtl};
    use crate::param::{Asset, Price};
    use crate::FullSync;

    fn px(s: &str) -> Price {
        Price::from_str(s).expect("valid price")
    }

    fn asset(s: &str) -> Asset {
        Asset::new(s).expect("valid asset")
    }

    fn account(n: u64) -> AccountId {
        AccountId::from_u64(n)
    }

    fn group(n: u32) -> AccountGroupId {
        AccountGroupId::from_u32(n).expect("valid account group id")
    }

    #[test]
    fn pnl_barrier_builder_rejects_an_empty_configuration() {
        let error = SpotFundsSettings::new(0, SpotFundsPricingSource::Mark, [])
            .expect("base settings must build")
            .with_pnl_barriers([], [], [])
            .expect_err("empty P&L barriers must fail");

        assert_eq!(error, SpotFundsConfigError::NoPnlBarriersConfigured);
        assert_eq!(
            error.to_string(),
            "spot funds P&L bounds require at least one barrier"
        );
    }

    #[test]
    fn pnl_barrier_builder_accepts_a_global_barrier() {
        let result = SpotFundsSettings::new(0, SpotFundsPricingSource::Mark, [])
            .expect("base settings must build")
            .with_pnl_barriers(
                [SpotFundsPnlBoundsBarrier {
                    account_currency: asset("USD"),
                    lower_bound: Some(Pnl::from_str("-100").expect("valid pnl")),
                    upper_bound: None,
                }],
                [],
                [],
            );

        assert!(result.is_ok());
    }

    /// Registers `AAPL/USD` with `mark = 100` in the default bucket and returns
    /// the service handle plus the instrument id. The default bucket is
    /// reachable by any account under `AccountThenGroupThenDefault`, so every
    /// effective-price call below shares the same base price and any change in
    /// the result is attributable solely to the slippage tier selected.
    fn service_with_mark_100() -> (Arc<MarketDataService<FullSync>>, InstrumentId) {
        let svc = MarketDataBuilder::<FullSync>::new(QuoteTtl::Infinite).build();
        let id = svc
            .register(Instrument::new(asset("AAPL"), asset("USD")))
            .expect("register must succeed");
        svc.push(id, Quote::new().with_mark(px("100")))
            .expect("push must succeed");
        (svc, id)
    }

    /// Resolves the effective buy price for `settings` against `svc`,
    /// mirroring the policy's quote-then-price hot path.
    fn buy_price_of(
        svc: &Arc<MarketDataService<FullSync>>,
        settings: &SpotFundsSettings,
        id: InstrumentId,
        account_id: AccountId,
        account_info: &impl AccountInfo,
    ) -> Result<Price, SpotFundsPriceError> {
        let md = SpotFundsMarketData::<FullSync>::new(Arc::clone(svc));
        let quote = md
            .quote(id, account_id, account_info)
            .ok_or(SpotFundsPriceError::QuoteUnavailable)?;
        settings.effective_buy_price(&quote, id, account_id, account_info)
    }

    /// Resolves the effective sell price for `settings` against `svc`,
    /// mirroring the policy's quote-then-price hot path.
    fn sell_price_of(
        svc: &Arc<MarketDataService<FullSync>>,
        settings: &SpotFundsSettings,
        id: InstrumentId,
        account_id: AccountId,
        account_info: &impl AccountInfo,
    ) -> Result<Price, SpotFundsPriceError> {
        let md = SpotFundsMarketData::<FullSync>::new(Arc::clone(svc));
        let quote = md
            .quote(id, account_id, account_info)
            .ok_or(SpotFundsPriceError::QuoteUnavailable)?;
        settings.effective_sell_price(&quote, id, account_id, account_info)
    }

    fn buy_price<Overrides>(
        slippage_bps: u16,
        overrides: Overrides,
        account_id: AccountId,
        account_info: &impl AccountInfo,
    ) -> Result<Price, SpotFundsPriceError>
    where
        Overrides: IntoIterator<Item = (SpotFundsOverrideTarget, SpotFundsOverride)>,
    {
        let (svc, id) = service_with_mark_100();
        let settings =
            SpotFundsSettings::new(slippage_bps, SpotFundsPricingSource::Mark, overrides)
                .expect("settings must build");
        buy_price_of(&svc, &settings, id, account_id, account_info)
    }

    #[test]
    fn account_override_wins_over_group_instrument_and_global() {
        let (svc, id) = service_with_mark_100();
        let acc = account(7);
        let grp = group(3);
        // global 0, instrument 1000, group 2000, account 3000 bps.
        let overrides = [
            (
                SpotFundsOverrideTarget::Instrument(id),
                SpotFundsOverride {
                    slippage_bps: Some(1000),
                },
            ),
            (
                SpotFundsOverrideTarget::InstrumentAccountGroup(id, grp),
                SpotFundsOverride {
                    slippage_bps: Some(2000),
                },
            ),
            (
                SpotFundsOverrideTarget::InstrumentAccount(id, acc),
                SpotFundsOverride {
                    slippage_bps: Some(3000),
                },
            ),
        ];
        let settings = SpotFundsSettings::new(0, SpotFundsPricingSource::Mark, overrides)
            .expect("settings must build");
        // 100 * (1 + 0.30) = 130 - account tier wins even though the account
        // is also in the matching group.
        assert_eq!(
            buy_price_of(&svc, &settings, id, acc, &Some(grp)),
            Ok(px("130"))
        );
    }

    #[test]
    fn group_override_used_when_no_account_override_matches() {
        let acc = account(7);
        let grp = group(3);
        let (svc, id) = service_with_mark_100();
        let overrides = [
            (
                SpotFundsOverrideTarget::Instrument(id),
                SpotFundsOverride {
                    slippage_bps: Some(1000),
                },
            ),
            (
                SpotFundsOverrideTarget::InstrumentAccountGroup(id, grp),
                SpotFundsOverride {
                    slippage_bps: Some(2000),
                },
            ),
        ];
        let settings = SpotFundsSettings::new(0, SpotFundsPricingSource::Mark, overrides)
            .expect("settings must build");
        // No account override, account is in group 3 -> 100 * 1.20 = 120.
        assert_eq!(
            buy_price_of(&svc, &settings, id, acc, &Some(grp)),
            Ok(px("120"))
        );
    }

    #[test]
    fn instrument_default_used_when_neither_account_nor_group_matches() {
        let acc = account(7);
        let (svc, id) = service_with_mark_100();
        let settings = SpotFundsSettings::new(
            0,
            SpotFundsPricingSource::Mark,
            [(
                SpotFundsOverrideTarget::Instrument(id),
                SpotFundsOverride {
                    slippage_bps: Some(1000),
                },
            )],
        )
        .expect("settings must build");
        // Account info yields no account group, so the account-group tier is
        // skipped entirely and the instrument default (1000 bps) applies -> 110.
        assert_eq!(buy_price_of(&svc, &settings, id, acc, &None), Ok(px("110")));
        // A present but non-matching group still falls through to instrument.
        assert_eq!(
            buy_price_of(&svc, &settings, id, acc, &Some(group(9))),
            Ok(px("110"))
        );
    }

    #[test]
    fn global_used_when_nothing_matches() {
        let acc = account(7);
        // Global 1000 bps, no overrides at all -> 100 * 1.10 = 110.
        assert_eq!(
            buy_price(1000, std::iter::empty(), acc, &None),
            Ok(px("110"))
        );
    }

    #[test]
    fn none_slippage_override_entry_is_treated_as_absent() {
        let acc = account(7);
        let grp = group(3);
        let (svc, id) = service_with_mark_100();
        // Account and group entries both carry None -> ignored, so the cascade
        // falls through to the instrument default (1000 bps = 110).
        let overrides = [
            (
                SpotFundsOverrideTarget::InstrumentAccount(id, acc),
                SpotFundsOverride { slippage_bps: None },
            ),
            (
                SpotFundsOverrideTarget::InstrumentAccountGroup(id, grp),
                SpotFundsOverride { slippage_bps: None },
            ),
            (
                SpotFundsOverrideTarget::Instrument(id),
                SpotFundsOverride {
                    slippage_bps: Some(1000),
                },
            ),
        ];
        let settings = SpotFundsSettings::new(0, SpotFundsPricingSource::Mark, overrides)
            .expect("settings must build");
        assert_eq!(
            buy_price_of(&svc, &settings, id, acc, &Some(grp)),
            Ok(px("110"))
        );
    }

    #[test]
    fn out_of_range_account_override_returns_slippage_out_of_range() {
        let (_svc, id) = service_with_mark_100();
        let result = SpotFundsSettings::new(
            0,
            SpotFundsPricingSource::Mark,
            [(
                SpotFundsOverrideTarget::InstrumentAccount(id, account(7)),
                SpotFundsOverride {
                    slippage_bps: Some(10_001),
                },
            )],
        );
        assert_eq!(
            result.err(),
            Some(SpotFundsConfigError::SlippageOutOfRange { bps: 10_001 })
        );
    }

    #[test]
    fn out_of_range_group_override_returns_slippage_out_of_range() {
        let (_svc, id) = service_with_mark_100();
        let result = SpotFundsSettings::new(
            0,
            SpotFundsPricingSource::Mark,
            [(
                SpotFundsOverrideTarget::InstrumentAccountGroup(id, group(3)),
                SpotFundsOverride {
                    slippage_bps: Some(10_001),
                },
            )],
        );
        assert_eq!(
            result.err(),
            Some(SpotFundsConfigError::SlippageOutOfRange { bps: 10_001 })
        );
    }

    // ── Sell-side cascade tests ───────────────────────────────────────────────
    //
    // Formula: mark * (1 - factor), factor = bps / 10_000, mark = 100.
    //   3000 bps -> 100 * 0.70 = 70
    //   2000 bps -> 100 * 0.80 = 80
    //   1000 bps -> 100 * 0.90 = 90
    //      0 bps -> 100 * 1.00 = 100

    fn sell_price<Overrides>(
        slippage_bps: u16,
        overrides: Overrides,
        account_id: AccountId,
        account_info: &impl AccountInfo,
    ) -> Result<Price, SpotFundsPriceError>
    where
        Overrides: IntoIterator<Item = (SpotFundsOverrideTarget, SpotFundsOverride)>,
    {
        let (svc, id) = service_with_mark_100();
        let settings =
            SpotFundsSettings::new(slippage_bps, SpotFundsPricingSource::Mark, overrides)
                .expect("settings must build");
        sell_price_of(&svc, &settings, id, account_id, account_info)
    }

    #[test]
    fn sell_account_override_wins_over_group_instrument_and_global() {
        let (svc, id) = service_with_mark_100();
        let acc = account(7);
        let grp = group(3);
        // global 0, instrument 1000, group 2000, account 3000 bps.
        let overrides = [
            (
                SpotFundsOverrideTarget::Instrument(id),
                SpotFundsOverride {
                    slippage_bps: Some(1000),
                },
            ),
            (
                SpotFundsOverrideTarget::InstrumentAccountGroup(id, grp),
                SpotFundsOverride {
                    slippage_bps: Some(2000),
                },
            ),
            (
                SpotFundsOverrideTarget::InstrumentAccount(id, acc),
                SpotFundsOverride {
                    slippage_bps: Some(3000),
                },
            ),
        ];
        let settings = SpotFundsSettings::new(0, SpotFundsPricingSource::Mark, overrides)
            .expect("settings must build");
        // 100 * (1 - 0.30) = 70 - account tier wins even though the
        // account is also in the matching group.
        assert_eq!(
            sell_price_of(&svc, &settings, id, acc, &Some(grp)),
            Ok(px("70"))
        );
    }

    #[test]
    fn sell_group_override_used_when_no_account_override_matches() {
        let acc = account(7);
        let grp = group(3);
        let (svc, id) = service_with_mark_100();
        let overrides = [
            (
                SpotFundsOverrideTarget::Instrument(id),
                SpotFundsOverride {
                    slippage_bps: Some(1000),
                },
            ),
            (
                SpotFundsOverrideTarget::InstrumentAccountGroup(id, grp),
                SpotFundsOverride {
                    slippage_bps: Some(2000),
                },
            ),
        ];
        let settings = SpotFundsSettings::new(0, SpotFundsPricingSource::Mark, overrides)
            .expect("settings must build");
        // No account override; account is in group 3 -> 100 * 0.80 = 80.
        assert_eq!(
            sell_price_of(&svc, &settings, id, acc, &Some(grp)),
            Ok(px("80"))
        );
    }

    #[test]
    fn sell_instrument_default_used_when_neither_account_nor_group_matches() {
        let acc = account(7);
        let (svc, id) = service_with_mark_100();
        let settings = SpotFundsSettings::new(
            0,
            SpotFundsPricingSource::Mark,
            [(
                SpotFundsOverrideTarget::Instrument(id),
                SpotFundsOverride {
                    slippage_bps: Some(1000),
                },
            )],
        )
        .expect("settings must build");
        // No account group -> instrument default 1000 bps -> 100 * 0.90 = 90.
        assert_eq!(sell_price_of(&svc, &settings, id, acc, &None), Ok(px("90")));
        // A present but non-matching group still falls through to
        // the instrument default.
        assert_eq!(
            sell_price_of(&svc, &settings, id, acc, &Some(group(9))),
            Ok(px("90"))
        );
    }

    #[test]
    fn sell_global_used_when_nothing_matches() {
        let acc = account(7);
        // Global 1000 bps, no overrides -> 100 * (1 - 0.10) = 90.
        assert_eq!(
            sell_price(1000, std::iter::empty(), acc, &None),
            Ok(px("90"))
        );
    }

    // ── Limit-mode cascade tests ──────────────────────────────────────────────
    //
    // Resolution order mirrors `pricer_for`: account -> account group -> global.

    fn default_settings() -> SpotFundsSettings {
        SpotFundsSettings::new(0, SpotFundsPricingSource::Mark, std::iter::empty())
            .expect("settings must build")
    }

    #[test]
    fn limit_mode_defaults_to_enforce() {
        let settings = default_settings();
        assert_eq!(settings.global_limit_mode(), SpotFundsLimitMode::Enforce);
        assert_eq!(
            settings.limit_mode_for(account(7), &None),
            SpotFundsLimitMode::Enforce
        );
    }

    #[test]
    fn limit_mode_global_applies_when_no_override_matches() {
        let mut settings = default_settings();
        settings.set_global_limit_mode(SpotFundsLimitMode::TrackOnly);
        assert_eq!(settings.global_limit_mode(), SpotFundsLimitMode::TrackOnly);
        // No per-account or per-group override -> global wins, with or without a
        // bound account group.
        assert_eq!(
            settings.limit_mode_for(account(7), &None),
            SpotFundsLimitMode::TrackOnly
        );
        assert_eq!(
            settings.limit_mode_for(account(7), &Some(group(3))),
            SpotFundsLimitMode::TrackOnly
        );
    }

    #[test]
    fn limit_mode_account_override_wins_over_group_and_global() {
        let acc = account(7);
        let grp = group(3);
        let mut settings = default_settings();
        // global Enforce, group TrackOnly, account Enforce -> account wins.
        settings.set_global_limit_mode(SpotFundsLimitMode::TrackOnly);
        settings.set_account_group_limit_mode(grp, Some(SpotFundsLimitMode::TrackOnly));
        settings.set_account_limit_mode(acc, Some(SpotFundsLimitMode::Enforce));
        assert_eq!(
            settings.limit_mode_for(acc, &Some(grp)),
            SpotFundsLimitMode::Enforce
        );
    }

    #[test]
    fn limit_mode_group_override_used_when_no_account_override_matches() {
        let acc = account(7);
        let grp = group(3);
        let mut settings = default_settings();
        // global Enforce, group TrackOnly, no account override -> group wins.
        settings.set_account_group_limit_mode(grp, Some(SpotFundsLimitMode::TrackOnly));
        assert_eq!(
            settings.limit_mode_for(acc, &Some(grp)),
            SpotFundsLimitMode::TrackOnly
        );
        // An account with no override but not in the group falls to global.
        assert_eq!(
            settings.limit_mode_for(acc, &None),
            SpotFundsLimitMode::Enforce
        );
        // A present but non-matching group also falls to global.
        assert_eq!(
            settings.limit_mode_for(acc, &Some(group(9))),
            SpotFundsLimitMode::Enforce
        );
    }

    #[test]
    fn limit_mode_account_override_cleared_falls_back_to_group() {
        let acc = account(7);
        let grp = group(3);
        let mut settings = default_settings();
        settings.set_account_group_limit_mode(grp, Some(SpotFundsLimitMode::TrackOnly));
        settings.set_account_limit_mode(acc, Some(SpotFundsLimitMode::Enforce));
        assert_eq!(
            settings.limit_mode_for(acc, &Some(grp)),
            SpotFundsLimitMode::Enforce
        );
        // Clearing the account override falls back to the group tier.
        settings.set_account_limit_mode(acc, None);
        assert_eq!(
            settings.limit_mode_for(acc, &Some(grp)),
            SpotFundsLimitMode::TrackOnly
        );
    }

    #[test]
    fn limit_mode_group_override_cleared_falls_back_to_global() {
        let acc = account(7);
        let grp = group(3);
        let mut settings = default_settings();
        settings.set_global_limit_mode(SpotFundsLimitMode::Enforce);
        settings.set_account_group_limit_mode(grp, Some(SpotFundsLimitMode::TrackOnly));
        assert_eq!(
            settings.limit_mode_for(acc, &Some(grp)),
            SpotFundsLimitMode::TrackOnly
        );
        // Clearing the group override falls back to the global mode.
        settings.set_account_group_limit_mode(grp, None);
        assert_eq!(
            settings.limit_mode_for(acc, &Some(grp)),
            SpotFundsLimitMode::Enforce
        );
    }
}
