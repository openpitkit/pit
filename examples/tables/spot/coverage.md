---
name: spot coverage
slippage_bps: 0
---

<!-- markdownlint-disable line-length -->

# Spot coverage scenario

Every feature of the runner in one table: all five actions, global /
account- / group-addressed market data (each resolution path is taken by
a market order), quantity- and volume-denominated orders, market and limit
orders, and a buy **and** a sell of each kind. `slippage_bps = 0`, so a market
order reserves exactly `qty * mark`.

Marks: `alice` reads an account-addressed `100`, `bob` the global `150`, and the
`desk` group (`carol`, `dave`) a group-addressed `120`. Seeds dwarf every
notional, so the only deliberate rejects are the over-budget buy and the
oversold sell.

|  # | account | action | instrument | side | qty    | volume | price | asset | amount | group | expect | reject            | note                                  |
|----|---------|--------|------------|------|--------|--------|-------|-------|--------|-------|--------|-------------------|---------------------------------------|
|  1 | carol   | GROUP  |            |      |        |        |       |       |        | desk  | OK     |                   | carol joins desk                      |
|  2 | dave    | GROUP  |            |      |        |        |       |       |        | desk  | OK     |                   | dave joins desk                       |
|  3 | alice   | SEED   |            |      |        |        |       | USD   | 100000 |       | OK     |                   | cash                                  |
|  4 | alice   | SEED   |            |      |        |        |       | AAPL  | 1000   |       | OK     |                   | inventory                             |
|  5 | bob     | SEED   |            |      |        |        |       | USD   | 100000 |       | OK     |                   | cash                                  |
|  6 | bob     | SEED   |            |      |        |        |       | AAPL  | 1000   |       | OK     |                   | inventory                             |
|  7 | carol   | SEED   |            |      |        |        |       | USD   | 100000 |       | OK     |                   | cash                                  |
|  8 | carol   | SEED   |            |      |        |        |       | AAPL  | 1000   |       | OK     |                   | inventory                             |
|  9 | dave    | SEED   |            |      |        |        |       | USD   | 100000 |       | OK     |                   | cash                                  |
| 10 | dave    | SEED   |            |      |        |        |       | AAPL  | 1000   |       | OK     |                   | inventory                             |
| 11 | erin    | SEED   |            |      |        |        |       | AAPL  | 100    |       | OK     |                   | small inventory                       |
| 12 |         | TICK   | AAPL/USD   |      |        |        | 150   |       |        |       | OK     |                   | global mark (bob reads it)            |
| 13 | alice   | TICK   | AAPL/USD   |      |        |        | 100   |       |        |       | OK     |                   | account-addressed mark                |
| 14 |         | TICK   | AAPL/USD   |      |        |        | 120   |       |        | desk  | OK     |                   | group-addressed mark for desk         |
| 15 | alice   | ORDER  | AAPL/USD   | BUY  | 10     |        |       |       |        |       | ACCEPT |                   | market buy at account mark 100        |
| 16 | alice   | ORDER  | AAPL/USD   | SELL | 10     |        |       |       |        |       | ACCEPT |                   | market sell                           |
| 17 | alice   | ORDER  | AAPL/USD   | BUY  | 10     |        | 150   |       |        |       | ACCEPT |                   | limit buy by quantity                 |
| 18 | alice   | FILL   | AAPL/USD   | BUY  | 10     |        | 150   |       |        |       | OK     |                   | settle the limit buy                  |
| 19 | alice   | ORDER  | AAPL/USD   | SELL | 10     |        | 150   |       |        |       | ACCEPT |                   | limit sell by quantity                |
| 20 | alice   | FILL   | AAPL/USD   | SELL | 10     |        | 150   |       |        |       | OK     |                   | settle the limit sell                 |
| 21 | bob     | ORDER  | AAPL/USD   | BUY  | 10     |        |       |       |        |       | ACCEPT |                   | market buy at global mark 150         |
| 22 | bob     | ORDER  | AAPL/USD   | SELL | 10     |        |       |       |        |       | ACCEPT |                   | market sell                           |
| 23 | bob     | ORDER  | AAPL/USD   | BUY  |        | 3000   |       |       |        |       | ACCEPT |                   | market buy by volume (notional)       |
| 24 | bob     | ORDER  | AAPL/USD   | SELL |        | 3000   |       |       |        |       | ACCEPT |                   | market sell by volume                 |
| 25 | carol   | ORDER  | AAPL/USD   | BUY  | 10     |        |       |       |        |       | ACCEPT |                   | market buy at group mark 120          |
| 26 | carol   | ORDER  | AAPL/USD   | SELL | 10     |        |       |       |        |       | ACCEPT |                   | market sell                           |
| 27 | carol   | ORDER  | AAPL/USD   | BUY  |        | 1500   | 150   |       |        |       | ACCEPT |                   | volume buy at a limit price           |
| 28 | carol   | ORDER  | AAPL/USD   | SELL |        | 1500   | 150   |       |        |       | ACCEPT |                   | volume sell at a limit price          |
| 29 | dave    | ORDER  | AAPL/USD   | BUY  | 10     |        | 150   |       |        |       | ACCEPT |                   | limit buy (group member)              |
| 30 | dave    | ORDER  | AAPL/USD   | SELL | 10     |        | 150   |       |        |       | ACCEPT |                   | limit sell (group member)             |
| 31 | bob     | ORDER  | AAPL/USD   | BUY  | 100000 |        | 150   |       |        |       | REJECT | InsufficientFunds | notional 15,000,000 over cash         |
| 32 | erin    | ORDER  | AAPL/USD   | SELL | 100000 |        | 150   |       |        |       | REJECT | InsufficientFunds | sell 100000 with only 100 inventory   |
