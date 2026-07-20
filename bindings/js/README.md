# @openpit/engine - OpenPit (Pre-trade Integrity Toolkit) for JavaScript

<!-- markdownlint-disable MD013 -->

[![Verify](https://github.com/openpitkit/pit/actions/workflows/verify.yml/badge.svg)](https://github.com/openpitkit/pit/actions/workflows/verify.yml) [![Release](https://img.shields.io/github/v/release/openpitkit/pit)](https://github.com/openpitkit/pit/releases) [![npm](https://img.shields.io/npm/v/@openpit/engine)](https://www.npmjs.com/package/@openpit/engine) [![node](https://img.shields.io/node/v/@openpit/engine)](https://www.npmjs.com/package/@openpit/engine) [![License](https://img.shields.io/badge/license-Apache%202.0-blue)](../../LICENSE)

<!-- markdownlint-enable MD013 -->

`@openpit/engine` is an embeddable pre-trade risk SDK for integrating
policy-driven risk checks into trading systems from JavaScript and TypeScript.
It is a WebAssembly build of the OpenPit engine that runs the same way in Node,
browsers, Deno, Bun, and edge runtimes (Cloudflare Workers) behind a single
import, with no native add-on to compile and no `await` in the common path.

For an overview and links to all resources, see
the project website [openpit.dev](https://openpit.dev/).
For the generated API reference, see
[the JS API docs](https://openpit.dev/js-api/).
For full project documentation, see
[the repository README](https://github.com/openpitkit/pit/blob/main/README.md).
For conceptual and architectural pages, see
[the project wiki](https://github.com/openpitkit/pit/wiki).

## Versioning Policy (Pre-1.0)

Before the `1.0` release OpenPit follows a relaxed Semantic Versioning:

- `PATCH` releases carry bug fixes and small internal corrections.
- `MINOR` releases may introduce new features **and may also change the
  public interface**.

Breaking API changes can appear in minor releases before `1.0`. Pick
version constraints that tolerate API evolution during the pre-stable
phase.

## Install

```sh
npm install @openpit/engine
```

The package ships two platform builds behind one import. Both instantiate the
wasm synchronously at load, so there is no `await` in the common path:

- **Node** reads the sibling `.wasm` from disk (smallest footprint, fastest
  cold start).
- **Browser / edge** uses base64-inlined wasm - no `fetch`, no `fs`, no extra
  asset to host. It works on any CDN with zero configuration.

CDN / no build step (browser, Deno) via [esm.sh](https://esm.sh):

```html
<script type="module">
  import { Engine } from "https://esm.sh/@openpit/engine";
  // or: https://cdn.jsdelivr.net/npm/@openpit/engine/+esm
</script>
```

Deno, via the npm specifier:

<!-- Test mirror: pit/bindings/js/tests/examples.readme.test.ts -->

```ts
import { Engine as DenoEngine } from "npm:@openpit/engine";
```

### Building from source / Toolchain

End users should install the published
[npm package](https://www.npmjs.com/package/@openpit/engine); a source build is
needed only for local development on the binding itself. The full toolchain
(Rust with the `wasm32-unknown-unknown` target, `wasm-bindgen`, `wasm-opt`, and
the Node dev dependencies) is provisioned by:

```sh
just install
```

Note that `just install` provisions the complete build toolchain for the whole
repository. If you do not need the full build, read the recipe in
[`justfile`](../../justfile) and install only the parts you need for this
package: the `wasm32-unknown-unknown` target, a matching `wasm-bindgen-cli`, and
the local npm dependencies. Then:

```sh
cd bindings/js
npm install
npm run build
```

## Decimals

Prices, quantities, and money cross the boundary as decimal **strings** - the
only lossless form for full- and variable-scale instruments. This mirrors what
major exchange APIs do on the wire, and it sidesteps the IEEE-754 rounding that
silently corrupts trailing digits (`0.1 + 0.2 === 0.30000000000000004`).

The `DecimalInput` type accepted on input is:

<!-- Test mirror: pit/bindings/js/tests/examples.readme.test.ts -->

```ts
type DecimalInput = string | number | bigint;
```

- `string` is the recommended, lossless form (`"100.50"`, `"0.00847000"`).
- `bigint` is safe for exact integer values.
- `number` is an IEEE-754 double and is accepted only as a convenience for
  small exact integers. **Never use `number` for fractional money.**

Value types are constructed from `DecimalInput` and serialize back to a decimal
string; they never return a raw `number` for money.

<!-- Test mirror: pit/bindings/js/tests/examples.readme.test.ts -->

```ts
import { Price } from "@openpit/engine/param";

const price = Price.fromString("100.50");
price.toString(); // "100.50"
price.toJSON(); // "100.50" (so JSON.stringify is lossless)

// Quantize to an instrument tick with an explicit rounding strategy.
Price.fromStringRounded("1.005", 2, "default").toString(); // "1.00"
```

The canonical rounding strategies are `"midpointNearestEven"`,
`"midpointAwayFromZero"`, `"up"`, and `"down"`. Four ergonomic aliases are
also available: `"default"`/`"banker"` map to midpoint nearest-even and
`"conservativeProfit"`/`"conservativeLoss"` map to round-down. The plain
strings are accepted directly; the package also exports a
`RoundingStrategies` constant (and matching `RoundingStrategy` type) for
autocomplete, alongside `RejectCode`, `RejectScope`, and
`SpotFundsPricingSource` value sets for the stable strings accepted by this
package.
The `param` subpath also exports the five `FillType` wire values and the nine
`ParamKind` names used by cross-language error reporting.

### Plain values and object literals

You do not have to construct wrapper classes to feed the engine. Scalar inputs
accept a plain value (`accountId: 99224416`, `side: "BUY"`, `price: "185.00"`),
and every group accepts a plain object literal - so an order or an execution
report can be written inline, as in the [Usage](#usage) example. The wrapper
classes (`Price`, `AccountId`, `OrderOperation`, ...) remain available as a typed
alternative and are interchangeable with the plain forms.

Ordinary setters and input coercions borrow or clone wrapper values, so passing
a `Price`, `AccountId`, model group, or similar value does not invalidate the
caller's handle. Their `.clone()` methods are available when an independent copy
is useful, but are not required just to assign a value. Only lifecycle handles
and staged builders (`Request`, `Reservation`, `Mutation`, and consumed engine
builders) intentionally become unusable after their terminal operation.

## Engine

The engine evaluates an order through a deterministic pre-trade pipeline:

- `engine.startPreTrade(order)` runs lightweight start-stage checks and returns
  a single-use `Request`.
- `request.execute()` runs main-stage check policies and returns a single-use
  `Reservation`.
- `reservation.commit()` applies the reserved state; `reservation.rollback()`
  reverts it. Exactly one of the two must be called, exactly once.
- `engine.executePreTrade(order)` is a shortcut that collapses start and main
  into one step, still returning a `Reservation` to commit or roll back.
- `engine.startPreTradeDryRun(order)` / `engine.executePreTradeDryRun(order)`
  run the same checks on a read-only path and report what would have happened
  without spending rate-limit budget, creating reservations, or recording
  account blocks.
- `engine.applyExecutionReport(report)` updates post-trade policy state.
- `engine.configure()` retunes registered built-ins at runtime, including
  spot-funds limit modes and the generic / spot-funds P&L accumulators.

Start-stage checks aggregate rejects from all registered policies. Main-stage
checks aggregate rejects and run rollback mutations in reverse order when any
reject is produced.

The five built-in policies are:

- **SpotFunds** (`buildSpotFunds`) -
  [per-account solvency gate over spendable funds](https://github.com/openpitkit/pit/wiki/Spot-Funds).
- **OrderValidation** (`buildOrderValidation`) -
  [structural integrity checks on every order](https://github.com/openpitkit/pit/wiki/Policies#ordervalidationpolicy).
- **RateLimit** (`buildRateLimit`) -
  [throttle order flow per broker, asset, or account](https://github.com/openpitkit/pit/wiki/Policies#ratelimitpolicy).
- **OrderSizeLimit** (`buildOrderSizeLimit`) -
  [fat-finger caps on quantity and notional](https://github.com/openpitkit/pit/wiki/Policies#ordersizelimitpolicy).
- **PnlBoundsKillswitch** (`buildPnlBoundsKillswitch`) -
  [halt an account when realized P&L breaches bounds](https://github.com/openpitkit/pit/wiki/Policies#pnlboundskillswitchpolicy).

You can also write project-specific policies against the public policy API:
[Custom policies](https://github.com/openpitkit/pit/wiki/Policy-API).

## Threading

The WebAssembly engine is **single-threaded** and always uses no-op locking. The
builder exposes no sync-mode selection, and the engine and its handles are
`!Send`. There is no async engine and no off-thread execution: every policy
callback runs synchronously on the calling thread. Use one engine instance per
worker/isolate when you need parallelism.

## Usage

<!-- Test mirror: pit/bindings/js/tests/examples.readme.test.ts -->

```ts
import { Engine } from "@openpit/engine";
import { TradeAmount } from "@openpit/engine/param";
import {
  type OrderInit,
  type ExecutionReportInit,
} from "@openpit/engine/model";
import { buildOrderValidation } from "@openpit/engine/pretrade/policies";

// 1. Build the engine once, at platform initialization.
const engine = Engine.builder().builtin(buildOrderValidation()).build();

// 2. Assemble an order as a plain object. Scalars accept plain values (the
//    account id as a number, the price as a decimal string); the order itself
//    is an object literal - no wrapper classes to construct. The OrderInit
//    annotation is optional; it just lets the literal sit in its own variable.
const order: OrderInit = {
  operation: {
    underlyingAsset: "AAPL",
    settlementAsset: "USD",
    accountId: 99224416,
    side: "BUY",
    tradeAmount: TradeAmount.quantity("100"),
    price: "185.00",
  },
};

// 3. Start stage: lightweight checks, no state change yet.
const start = engine.startPreTrade(order);
if (!start.ok) {
  const reasons = start.rejects
    .map((r) => `${r.policy} [${r.code}]: ${r.reason}`)
    .join(", ");
  throw new Error(reasons);
}

// 4. Main stage: full pre-trade and risk control.
const request = start.request;
if (request === undefined) {
  throw new Error("accepted start result is missing its request");
}
const execute = request.execute();
if (!execute.ok) {
  const reasons = execute.rejects
    .map((r) => `${r.policy} [${r.code}]: ${r.reason}`)
    .join(", ");
  throw new Error(reasons);
}

// 5. Commit once the venue accepts the order; roll back otherwise.
const reservation = execute.reservation;
if (reservation === undefined) {
  throw new Error("accepted execute result is missing its reservation");
}
try {
  // sendOrderToVenue(order);
  reservation.commit();
} catch (err) {
  reservation.rollback();
  throw err;
}

// 6. Feed the venue's execution report back into post-trade policy state, again
//    as a plain object literal. P&L and fee cross as decimal strings.
const report: ExecutionReportInit = {
  operation: {
    underlyingAsset: "AAPL",
    settlementAsset: "USD",
    accountId: 99224416,
    side: "BUY",
  },
  financialImpact: { pnl: "-50", fee: "3.4" },
};

const result = engine.applyExecutionReport(report);
// A non-empty `accountBlocks` means a kill switch has fired for the account.
if (result.accountBlocks.length > 0) {
  // Halt routing for the blocked account.
}
```

## Errors

Policy rejects from `engine.startPreTrade()` and `request.execute()` are not
exceptions: they are returned on the `StartResult` and `ExecuteResult` (`ok`
plus a `rejects` array).

Malformed JS shapes throw native `TypeError`; invalid values and numeric ranges
throw `RangeError` subclasses such as `ParamError`, `AssetError`, and
`AccountIdError`, all rooted at `OpenpitValueError`. Lifecycle and engine-state
failures use named `Error` subclasses. Every error OpenPit constructs at the
boundary is also branded, so `instanceof OpenpitError` remains the catch-all in
**both** the Node and browser builds; branch on the native category, concrete
class, or stable `err.name`:

<!-- Test mirror: pit/bindings/js/tests/examples.readme.test.ts -->

```ts
import { ParamError, OpenpitError } from "@openpit/engine";
import { Price } from "@openpit/engine/param";

try {
  Price.fromString("not a number");
} catch (err) {
  if (err instanceof ParamError) {
    console.error(err.code); // e.g. "InvalidFormat"
  } else if (err instanceof OpenpitError) {
    console.error(err.name, err.message);
  }
}
```

The base class and every subclass are exported from the root
`@openpit/engine`; `AccountBlockError` is also re-exported from
`@openpit/engine/reject`. The subclasses are:

- `ParamError` - invalid numeric input, arithmetic overflow, or a malformed
  value.
- `AssetError` / `AccountIdError` - empty or invalid asset / account
  identifiers.
- `LifecycleError` - single-use misuse: executing the same request twice,
  finalizing the same reservation twice, using a stale account-control handle,
  or reusing an engine builder that was already consumed.
- `EngineBuildError` - building an engine with no policy registered, or with a
  duplicate policy name / group id or an invalid built-in configuration.
- `PolicyConfigureError` - unknown policy, settings-type mismatch, rejected
  update, or a non-reentrant nested configuration call.
- `PolicyCallbackError` - a custom JS callback threw. Its `cause` is the
  original thrown value; `result` carries the completed post-trade or account-
  adjustment reconciliation result when that operation produces one.
- `MarketDataError` (and `UnknownInstrument` / `QuoteUnavailable`),
  `RegistrationError` / `AlreadyRegistered` / `UnknownInstrumentId`,
  `AccountGroupRegistrationError`, and `AccountBlockError` for the remaining
  market-data, registration, account-group, and admin-block failures.

Two stable code vocabularies appear, and they do not overlap:

- `err.code` (an `ErrorCode`, exported from the root) is set only on
  `ParamError`, `AssetError`, and `AccountIdError`. It classifies a value
  failure - for example `"InvalidFormat"`, `"Overflow"`, `"Negative"`, or
  `"Other"` when no finer code applies. `ParamError.param` identifies the
  affected value type and `ParamError.input` preserves malformed text where the
  core reports it. The other subclasses leave `code` undefined.
- `AccountBlockError.kind` (an `AccountBlockErrorKind`) is `"ReservedGroup"`,
  `"AccountNotBlocked"`, or `"GroupNotBlocked"`, telling the three admin-block
  failures apart.
- Registration, account-group, engine-build, and policy-configuration failures
  expose their own stable `kind` plus structured fields such as `instrumentId`,
  `accountId`, `policyName`, `policyGroupId`, `expected`, and `found`; callers
  do not need to parse human-readable messages.
- `RejectCode` (exported from `@openpit/engine/reject`) is a **separate**
  vocabulary carried on a business `Reject`, never on a thrown error. Business
  rejects use stable codes such as `"OrderValueCalculationFailed"` when a policy
  cannot evaluate order value without a `price`.

## Targets

The package ships ESM and CommonJS builds for Node `>=18` and browser/edge
bundlers. Every environment resolves the root `.` or any subpath to the matching
`node/` or `browser/` entry and module format:

<!-- markdownlint-disable MD013 -->

| Environment                                | Resolves to                  | Wasm           |
| ------------------------------------------ | ---------------------------- | -------------- |
| Node ESM (`>=18`)                          | `node/<entry>.js`            | from disk      |
| Node CommonJS (`>=18`)                     | `node/<entry>.cjs`           | from disk      |
| Bundlers (Vite, webpack, Rollup, esbuild)  | `browser/<entry>.{js,cjs}`   | inlined        |
| Browsers via CDN (esm.sh, jsDelivr, unpkg) | `browser/<entry>.js`         | inlined        |
| Deno (`npm:` specifier or esm.sh)          | `node` / `browser` ESM entry | disk / inlined |
| Bun                                        | `node/<entry>.{js,cjs}`      | from disk      |
| Cloudflare Workers / edge                  | `browser/<entry>.{js,cjs}`   | inlined        |

<!-- markdownlint-enable MD013 -->

`<entry>` is `index` for the root import or the subpath name (for example
`param`, `marketdata`, `pretrade/policies`). The inlined browser build needs no
asset-loader configuration and makes no network request at import, so CDN and
Workers usage is zero-config.

## License

Apache-2.0. See [`LICENSE`](../../LICENSE) and [`OWNERS`](../../OWNERS).
