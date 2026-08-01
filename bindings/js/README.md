# @openpit/engine - OpenPit (Pre-trade Integrity Toolkit) for JavaScript

<!-- markdownlint-disable MD013 -->

[![Verify](https://github.com/openpitkit/pit/actions/workflows/verify.yml/badge.svg)](https://github.com/openpitkit/pit/actions/workflows/verify.yml) [![Release](https://img.shields.io/github/v/release/openpitkit/pit)](https://github.com/openpitkit/pit/releases) [![npm](https://img.shields.io/npm/v/@openpit/engine)](https://www.npmjs.com/package/@openpit/engine) [![node](https://img.shields.io/node/v/@openpit/engine)](https://www.npmjs.com/package/@openpit/engine) [![License](https://img.shields.io/badge/license-Apache%202.0-blue)](https://github.com/openpitkit/pit/blob/main/LICENSE)

<!-- markdownlint-enable MD013 -->

`@openpit/engine` is an embeddable pre-trade risk SDK for integrating
policy-driven risk checks into trading systems from JavaScript and TypeScript.
It is a WebAssembly build of the OpenPit engine that runs the same way in Node,
browsers, Deno, Bun, and edge runtimes (Cloudflare Workers) behind a single
import, with no native add-on to compile and no `await` in the common path.

For an overview and links to all resources, see
the project website [openpit.dev](https://openpit.dev/).
For the generated API reference, see
[the JS API docs](https://docs.openpit.dev/js-api/).
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

<!-- Test mirror: bindings/js/tests/examples.readme.test.ts -->

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
[`justfile`](https://github.com/openpitkit/pit/blob/main/justfile) and install
only the parts you need for this package: the `wasm32-unknown-unknown` target,
a matching `wasm-bindgen-cli`, and the local npm dependencies. Then:

```sh
cd bindings/js
npm install
npm run build
```

## Quick Start

Money and quantities cross the boundary as lossless decimal strings (or
`bigint` for 64-bit ids), never as floating-point `number`; assets are
validated strings. The full input contract is on
[Domain Types](https://wiki.openpit.dev/Domain-Types/).

<!-- Test mirror: bindings/js/tests/examples.readme.test.ts -->

```ts
import {
  Engine,
  OrderSizeBrokerBarrier,
  TradeAmount,
  buildOrderSizeLimit,
} from "@openpit/engine";

// Build the engine once, at start-up: one broker-wide fat-finger cap.
const engine = Engine.builder()
  .builtin(
    buildOrderSizeLimit().brokerBarrier(
      new OrderSizeBrokerBarrier({
        maxQuantity: "500",
        maxNotional: "1000000",
      }),
    ),
  )
  .build();

const result = engine.executePreTrade({
  operation: {
    underlyingAsset: "AAPL",
    settlementAsset: "USD",
    accountId: 99224416,
    side: "BUY",
    tradeAmount: TradeAmount.quantity("1000"),
    price: "185",
  },
});

if (result.ok) {
  // Send the order to the venue, then commit or roll the reservation back.
  result.reservation?.commit();
} else {
  for (const reject of result.rejects) {
    // OrderSizeLimitPolicy [OrderQtyExceedsLimit]: order quantity exceeded
    console.error(`${reject.policy} [${reject.code}]: ${reject.reason}`);
  }
}
```

The explicit two-stage flow, drop copy, and post-trade reports are described
on the [Pre-trade Pipeline](https://wiki.openpit.dev/Pre-trade-Pipeline/)
page.

## What Is Inside

- [Spot Funds](https://wiki.openpit.dev/Spot-Funds/) - per-account
  solvency gate over spendable funds.
- [Order Validation](https://wiki.openpit.dev/Policies/#ordervalidationpolicy)
  \- structural integrity checks on every order.
- [Rate Limit](https://wiki.openpit.dev/Policies/#ratelimitpolicy)
  \- throttle order flow per broker, asset, or account.
- [Order Size Limit](https://wiki.openpit.dev/Policies/#ordersizelimitpolicy)
  \- fat-finger caps on quantity and notional.
- [P&L Kill Switch](https://wiki.openpit.dev/Policies/#pnlboundskillswitchpolicy)
  \- halt an account when realized P&L breaches bounds.
- [Custom policies](https://wiki.openpit.dev/Policy-API/) - the primary
  integration model.
- [Account Blocking](https://wiki.openpit.dev/Account-Blocking/),
  [Account Groups](https://wiki.openpit.dev/Account-Groups/),
  [Account Adjustments](https://wiki.openpit.dev/Account-Adjustments/), and
  [Balance Reconciliation](https://wiki.openpit.dev/Balance-Reconciliation/).
- [Drop Copy](https://wiki.openpit.dev/Pre-trade-Pipeline/#drop-copy) -
  record already executed orders without pre-trade enforcement.
- [Market Data](https://wiki.openpit.dev/Market-Data/).
- [Dynamic Reconfiguration](https://wiki.openpit.dev/Dynamic-Policy-Reconfiguration/)
  of a live policy.
- [Domain Types](https://wiki.openpit.dev/Domain-Types/) - decimals, handles,
  and which objects are consumed on use.
- [Rejects and errors](https://wiki.openpit.dev/Errors/) with
  [stable reject codes](https://wiki.openpit.dev/Reject-Codes/).

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

Apache-2.0. See
[`LICENSE`](https://github.com/openpitkit/pit/blob/main/LICENSE) and
[`OWNERS`](https://github.com/openpitkit/pit/blob/main/OWNERS).
