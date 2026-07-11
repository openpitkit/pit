# browser_terminal

A web terminal that runs the OpenPit pre-trade risk engine **entirely in the
browser**. The engine is compiled to WebAssembly and bundled into the page;
there is no backend, no API call, and no network round-trip. Every command you
type drives the real risk engine running client-side.

This is the flagship in-browser demo. It focuses on the browser environment:
the same core engine used in native deployments runs unmodified as WASM in a
browser tab.

## What it demonstrates

- **The full pre-trade pipeline, client-side.** `OrderValidation`, `SpotFunds`,
  and `PnlBoundsKillswitch` are wired into one engine in the browser and run the
  two-stage `startPreTrade` -> `execute` -> `commit` flow on every order.
- **Zero-await default import.** The browser entry of `@openpit/engine` has the
  wasm base64-inlined and initializes synchronously at import, so the engine is
  ready with no loading spinner and no `await` in the page code.
- **No backend.** A production `vite build` produces a static bundle with the
  wasm inlined into the JavaScript - no sidecar `.wasm` to host and no server to
  talk to. The built `dist/` can be dropped on any static host or CDN.
- **Real risk decisions.** Held funds make a second identical buy fail with
  `InsufficientFunds`; a large realized loss on a fill trips the P&L kill switch
  and blocks the account. Nothing is mocked.

## Commands

| Command | Effect |
| --- | --- |
| `help` | list the commands |
| `config` | print the engine configuration |
| `balance` | show available and held settlement cash |
| `quote` | read the live top-of-book quote from market data |
| `place <BUY\|SELL> <qty> [price]` | run pre-trade and commit the order |
| `fill [pnl]` | settle the last BUY, booking realized P&L |
| `clear` | clear the screen |

### A guided tour

```text
config                 # see the three policies and the seeded 250000 USD
place BUY 3            # 3 BTC @ 50000 = 150000 USD: accepted, funds held
balance               # 100000 available, 150000 held
place BUY 3            # only 100000 available now: REJECTED, InsufficientFunds
fill -60000           # settle the held buy, book a 60000 USD loss
                      #   -> trips PnlBoundsKillswitch, the account is blocked
```

## Run it

`@openpit/engine` is a self-contained WASM package: no native add-on to compile.
This demo depends on the locally built package via a `file:` reference, so build
the package once first. From the repository root:

```sh
just install                         # provisions the toolchain (once)
cd bindings/js && npm run build      # builds bindings/js/dist
```

Then, from this directory:

```sh
npm install
npm run dev      # start the Vite dev server, open the printed URL
```

For a production build (the acceptance bar - a static, inlined-wasm bundle):

```sh
npm run build    # type-checks, then vite build -> ./dist
npm run preview  # serve the built ./dist locally
```

The repository-wide `just test-js` / `just test-js-debug` recipes run this
production build after the Node binding and example tests, so browser package
resolution is part of the normal JS CI signal.

The resulting `dist/` is fully static: open it from any static file host. Because
the wasm is inlined into the bundle, there is no separate `.wasm` asset to serve.

## How it is wired

- [`src/session.ts`](src/session.ts) is pure `@openpit/engine` SDK usage - the
  same calls a real integration makes - with no DOM concerns. It builds the
  engine, seeds the account, and exposes `config`/`place`/`fill`/`quote`.
- [`src/main.ts`](src/main.ts) wires [xterm.js](https://xtermjs.org/) into the
  page, parses typed lines, and dispatches them to the session.

## Decimals are strings

Prices, quantities, and money cross the engine boundary as decimal **strings**,
the only lossless form (`Price.fromString("50000")`, never `50000.0`). See the
package [`README`](../../../bindings/js/README.md) for the full decimal contract.
