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

// The engine-facing half of the demo. Everything in this file is plain
// `@openpit/engine` SDK usage - the same calls a real integration would make -
// with no DOM and no terminal concerns, so it reads as a self-contained
// pre-trade risk session. `src/main.ts` wraps these methods in typed terminal
// commands.
//
// The import below resolves to the package's BROWSER entry (Vite picks the
// `browser` export condition), which has the wasm base64-inlined. The module
// initializes the wasm synchronously at import, so the engine is ready with no
// `await` anywhere in this file - the headline property of the SDK.

import { Engine } from "@openpit/engine";
import {
  AccountId,
  AdjustmentAmount,
  Instrument,
  TradeAmount,
} from "@openpit/engine/param";
import {
  type AccountAdjustmentInit,
  type ExecutionReportInit,
  type OrderInit,
} from "@openpit/engine/model";
import { type Lock } from "@openpit/engine/pretrade";
import {
  buildOrderValidation,
  buildPnlBoundsKillswitch,
  buildSpotFunds,
  PnlBoundsBrokerBarrier,
} from "@openpit/engine/pretrade/policies";
import {
  type InstrumentId,
  type MarketDataService,
  Quote,
  QuoteResolution,
  QuoteTtl,
} from "@openpit/engine/marketdata";
import { type Reject } from "@openpit/engine/reject";

// =============================================================================
// Scenario constants. One account, one limit-only spot instrument, a P&L kill
// switch wide enough that ordinary fills pass but a big loss trips it. The
// numbers are chosen so each command has a visible, repeatable effect.
// =============================================================================

const ACCOUNT_LABEL = "demo-desk";
const TRADED_ASSET = "BTC";
const SETTLE_ASSET = "USD";
const SEED_FUNDS = "250000"; // initial available USD
const PNL_LOWER_BOUND = "-50000"; // kill switch floor for realized P&L
const PNL_UPPER_BOUND = "100000"; // and ceiling
const DEFAULT_PRICE = "50000"; // BTC/USD limit price used when none is given
const QUOTE_BID = "49995";
const QUOTE_ASK = "50005";
const QUOTE_MARK = "50000";

// CamelCase business reject code SpotFunds returns when settlement cash is
// short. Surfaced so the UI can highlight the canonical insufficient-funds path.
const INSUFFICIENT_FUNDS = "InsufficientFunds";

/** One named field of the engine configuration, for the `config` command. */
export interface ConfigLine {
  readonly label: string;
  readonly value: string;
}

/** A normalized reject row the terminal can print without touching wasm types. */
export interface RejectRow {
  readonly policy: string;
  readonly code: string;
  readonly reason: string;
  readonly details: string;
}

/** Outcome of a `place` command: accepted-and-committed, or rejected. */
export interface PlaceOutcome {
  readonly accepted: boolean;
  readonly side: string;
  readonly quantity: string;
  readonly price: string;
  readonly notional: string;
  readonly availableAfter: string;
  readonly heldAfter: string;
  readonly rejects: RejectRow[];
  // The committed reservation's lock, retained so a later `fill` settles this
  // exact reservation. Null on reject (a rejected order reserves nothing).
  readonly lock: Lock | null;
}

/** Outcome of a `fill` command. */
export interface FillOutcome {
  readonly settled: string;
  readonly blocked: boolean;
  readonly blockReason: string;
}

/** A market-data read for the `quote` command. */
export interface QuoteView {
  readonly bid: string;
  readonly ask: string;
  readonly mark: string;
}

/**
 * A live pre-trade risk session: one engine plus the small amount of book-
 * keeping the terminal needs (running balances and the last committed lock).
 *
 * The engine itself is the source of truth for risk decisions; the cash figures
 * tracked here are a local mirror kept only so the terminal can narrate "X
 * available, Y held" after each command. A production integration would read
 * those from its own ledger.
 */
export class Session {
  private readonly engine: Engine;
  private readonly market: MarketDataService;
  private readonly instrumentId: InstrumentId;

  // Local mirror of the account's settlement cash, in whole USD. The engine
  // enforces the real reservation; these numbers are for narration only.
  private available = Number(SEED_FUNDS);
  private held = 0;

  // The lock from the most recent committed BUY, so `fill` can settle it.
  private lastBuyLock: Lock | null = null;

  constructor() {
    // ---- One staged builder threads the market-data service into the engine.
    // The service MUST be opened from the same builder as the engine, so that
    // SpotFunds and the terminal read the same quote store.
    const builder = Engine.builder();
    this.market = builder.marketData(QuoteTtl.infinite()).build();

    // Register the instrument and publish a top-of-book quote. SpotFunds reads
    // this to price orders; the `quote` command prints it back.
    this.instrumentId = this.market.pushByInstrument(
      new Instrument(TRADED_ASSET, SETTLE_ASSET),
      new Quote({ bid: QUOTE_BID, ask: QUOTE_ASK, mark: QUOTE_MARK }),
    );

    // ---- Three built-in policies wired once, in order.
    //   OrderValidation - structural integrity, rejects malformed orders first.
    //   SpotFunds        - per-account solvency gate over spendable USD, fed by
    //                      the market-data service so it can price orders.
    //   PnlBoundsKillswitch - halts the desk when realized P&L breaches bounds.
    const ready = builder.builtin(buildOrderValidation());
    ready.builtin(buildSpotFunds().marketData(this.market, 0, "Mark", []));
    ready.builtin(
      buildPnlBoundsKillswitch().brokerBarriers([
        new PnlBoundsBrokerBarrier(
          SETTLE_ASSET,
          PNL_LOWER_BOUND,
          PNL_UPPER_BOUND,
        ),
      ]),
    );
    this.engine = ready.build();

    // Seed the desk's available settlement cash through the adjustment pipeline,
    // exactly as a deposit would arrive.
    this.seedFunds(SEED_FUNDS);
  }

  /** Human-readable engine configuration for the `config` command. */
  config(): ConfigLine[] {
    return [
      { label: "runtime", value: "WebAssembly (single-threaded)" },
      {
        label: "account",
        value: `${ACCOUNT_LABEL} (id ${this.account().toString()})`,
      },
      { label: "instrument", value: `${TRADED_ASSET}/${SETTLE_ASSET}` },
      {
        label: "policies",
        value: "OrderValidation, SpotFunds, PnlBoundsKillswitch",
      },
      {
        label: "spot funds",
        value: `seeded ${SEED_FUNDS} ${SETTLE_ASSET}, mark-priced`,
      },
      {
        label: "pnl bounds",
        value: `[${PNL_LOWER_BOUND}, ${PNL_UPPER_BOUND}] ${SETTLE_ASSET} (broker-wide)`,
      },
      {
        label: "top of book",
        value: `bid ${QUOTE_BID} / ask ${QUOTE_ASK} / mark ${QUOTE_MARK}`,
      },
    ];
  }

  /** Current local cash mirror for the `balance` command. */
  balance(): { available: string; held: string } {
    return {
      available: this.available.toString(),
      held: this.held.toString(),
    };
  }

  /** The latest top-of-book quote, read straight from the market-data service. */
  quote(): QuoteView {
    const quote = this.market.getOrErr(
      this.instrumentId,
      this.account(),
      // Minimal accountInfo: an ungrouped account resolves to the default bucket.
      { accountGroup: null },
      QuoteResolution.ACCOUNT_THEN_GROUP_THEN_DEFAULT(),
    );
    return {
      bid: quote.bid?.toString() ?? "-",
      ask: quote.ask?.toString() ?? "-",
      mark: quote.mark?.toString() ?? "-",
    };
  }

  /**
   * Run a BUY or SELL limit order through the full two-stage pre-trade flow and,
   * on accept, commit the reservation. A committed BUY moves cash from available
   * to held; a committed SELL is a position-reducing order that releases none of
   * the demo's tracked cash (its lock is not retained for settlement here).
   */
  place(side: "BUY" | "SELL", quantity: string, price: string): PlaceOutcome {
    const order = this.buildOrder(side, quantity, price);
    const notional = Number(quantity) * Number(price);

    const result = this.engine.executePreTrade(order);
    if (!result.ok) {
      return {
        accepted: false,
        side,
        quantity,
        price,
        notional: notional.toString(),
        availableAfter: this.available.toString(),
        heldAfter: this.held.toString(),
        rejects: result.rejects.map(toRejectRow),
        lock: null,
      };
    }

    // Snapshot the reservation lock BEFORE commit() - lock() throws once the
    // reservation is finalized - then commit to apply the reserved state.
    const reservation = result.reservation;
    if (reservation === undefined) {
      throw new Error("accepted result is missing its reservation");
    }
    const lock = reservation.lock();
    reservation.commit();

    if (side === "BUY") {
      this.available -= notional;
      this.held += notional;
      this.lastBuyLock = lock;
    }

    return {
      accepted: true,
      side,
      quantity,
      price,
      notional: notional.toString(),
      availableAfter: this.available.toString(),
      heldAfter: this.held.toString(),
      rejects: [],
      lock,
    };
  }

  /**
   * Settle the most recently committed BUY in full, optionally booking realized
   * P&L. The execution report carries the BUY's pre-trade lock so SpotFunds
   * settles that exact reservation; a P&L beyond the kill-switch bounds blocks
   * the account.
   */
  fill(pnl: string): FillOutcome {
    if (this.lastBuyLock === null) {
      throw new Error("nothing to fill: place a BUY order first");
    }
    const lock = this.lastBuyLock;
    this.lastBuyLock = null;

    const report = this.buildFillReport(lock, pnl);
    const result = this.engine.applyExecutionReport(report);

    // A full fill settles the held reservation back out of the held bucket.
    const settled = this.held;
    this.held = 0;

    if (result.accountBlocks.length > 0) {
      const block = result.accountBlocks[0]!;
      return {
        settled: settled.toString(),
        blocked: true,
        blockReason: `${block.code}: ${block.reason}`,
      };
    }
    return { settled: settled.toString(), blocked: false, blockReason: "" };
  }

  /** The default price used when `place` is given no explicit price. */
  static get defaultPrice(): string {
    return DEFAULT_PRICE;
  }

  /** The reject code highlighted by the UI for the insufficient-funds path. */
  static get insufficientFunds(): string {
    return INSUFFICIENT_FUNDS;
  }

  // ---------------------------------------------------------------------------
  // Private engine plumbing. Each method wraps one SDK call so the public
  // methods above stay narrative.
  // ---------------------------------------------------------------------------

  /**
   * The scenario account id, for the read paths that need an `AccountId` object
   * (the quote lookup and the `config` display). Orders, reports, and the seed
   * adjustment instead pass the account label string and let the engine hash it.
   */
  private account(): AccountId {
    return AccountId.fromString(ACCOUNT_LABEL);
  }

  /** Set the account's available settlement balance to an absolute amount. */
  private seedFunds(funds: string): void {
    const adjustment: AccountAdjustmentInit = {
      operation: { asset: SETTLE_ASSET },
      amount: { balance: AdjustmentAmount.absolute(funds) },
    };

    const result = this.engine.applyAccountAdjustment(ACCOUNT_LABEL, [
      adjustment,
    ]);
    if (!result.ok) {
      throw new Error(
        `seed adjustment rejected: ${result.rejects.map((r) => r.reason).join("; ")}`,
      );
    }
  }

  /** Assemble a limit order for the scenario instrument as a plain object. */
  private buildOrder(
    side: "BUY" | "SELL",
    quantity: string,
    price: string,
  ): OrderInit {
    return {
      operation: {
        underlyingAsset: TRADED_ASSET,
        settlementAsset: SETTLE_ASSET,
        accountId: ACCOUNT_LABEL,
        side,
        tradeAmount: TradeAmount.quantity(quantity),
        price,
      },
    };
  }

  /**
   * Assemble a full, final BUY execution report carrying the reservation lock.
   * The Lock is the one wrapper the report keeps; everything else is a plain
   * object with decimal-string prices and quantities.
   */
  private buildFillReport(lock: Lock, pnl: string): ExecutionReportInit {
    return {
      operation: {
        underlyingAsset: TRADED_ASSET,
        settlementAsset: SETTLE_ASSET,
        accountId: ACCOUNT_LABEL,
        side: "BUY",
      },
      // Combined-mode impact: realized P&L plus a zero fee. A loss below the
      // kill-switch floor trips PnlBoundsKillswitch and blocks the account.
      financialImpact: { pnl, fee: "0" },
      fill: {
        lock,
        lastTrade: { price: DEFAULT_PRICE, quantity: "1" },
        leavesQuantity: "0",
        isFinal: true,
      },
    };
  }
}

/** Normalize a wasm `Reject` into a plain row the terminal can render. */
function toRejectRow(reject: Reject): RejectRow {
  return {
    policy: reject.policy,
    code: reject.code,
    reason: reject.reason,
    details: reject.details,
  };
}
