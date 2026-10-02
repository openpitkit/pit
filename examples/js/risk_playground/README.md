# Risk Playground

An interactive [OpenPit](https://openpit.dev) pre-trade playground: the browser
entry of [`@openpit/engine`](https://www.npmjs.com/package/@openpit/engine)
runs the real engine as WebAssembly, with no backend. Published at
[openpit.dev/playground](https://openpit.dev/playground/).

- Spot Funds seeds the account balance, reserves the settlement asset, commits
  accepted orders, and immediately receives a matching execution report with a
  fixed 0.25 USD fee; its P&L axis owns the session loss limit.
- Order-size, order-value, and rate policies run as separate pre-trade checks;
  the visible chain stops at the first engine rejection.
- Preset scenarios and editable limits; every decision comes from the engine.
- On openpit.dev only, PostHog analytics with session replay; other hosts,
  including the dev server, send nothing.

## Run

Build the JS binding first, as the [examples README](../README.md#prerequisites)
describes. Then, from this directory:

```sh
just install   # once: locked dependencies, linked to ../../../bindings/js
just run       # rebuilds the binding and starts the Vite dev server
```

## Build

```sh
just check     # tests, type-checks, and bundles into dist/
```

CI runs the same `just check` against the local binding. Every push to `main`
builds the binding from that commit and deploys the page with it.
