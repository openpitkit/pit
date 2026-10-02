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

import { useCallback, useEffect, useMemo, useRef, useState } from "react";

import type { ReactNode } from "react";

import { trackPlaygroundEvent } from "./analytics.ts";
import brandMarkDark from "./assets/pit-logo-dark.svg";
import brandMarkLight from "./assets/pit-logo-light.svg";
import { Badge, Button, Card } from "./design-system.tsx";
import {
  DEFAULT_LIMITS,
  type CheckName,
  type DemoLimits,
  type DemoOrder,
  type GateDecision,
  type GateReject,
  RiskSession,
  type SessionSnapshot,
} from "./risk-session.ts";
import {
  applyTheme,
  readTheme,
  resolveTheme,
  saveTheme,
  type ResolvedTheme,
  type ThemeMode,
} from "./theme.ts";

interface InputLimits {
  readonly lossLimit: string;
  readonly maxRate: string;
  readonly maxShares: string;
  readonly maxValue: string;
  readonly spotFundsLimit: string;
}

interface InputOrder {
  readonly price: string;
  readonly quantity: string;
  readonly side: "BUY" | "SELL";
}

interface CheckView {
  readonly detail: string;
  readonly name: string;
  readonly state: "fail" | "pass" | "skip";
}

interface LimitMeterConfig {
  readonly current: bigint | null;
  readonly format: (value: bigint) => string;
  readonly limit: bigint;
  readonly unit?: string;
}

const DEFAULT_INPUT_LIMITS: InputLimits = {
  lossLimit: DEFAULT_LIMITS.lossLimit.toString(),
  maxRate: DEFAULT_LIMITS.maxRate.toString(),
  maxShares: DEFAULT_LIMITS.maxShares.toString(),
  maxValue: DEFAULT_LIMITS.maxValue.toString(),
  spotFundsLimit: DEFAULT_LIMITS.spotFundsLimit.toString(),
};

const DEFAULT_INPUT_ORDER: InputOrder = {
  price: "162.45",
  quantity: "30",
  side: "BUY",
};

const MAX_DEMO_RATE = 60n;

const CHECK_ORDER: readonly CheckName[] = [
  "pnl",
  "funds",
  "size",
  "value",
  "rate",
];

const CHECK_LABELS: Record<CheckName, string> = {
  funds: "Funds",
  pnl: "P&L",
  rate: "Order rate",
  size: "Order size",
  value: "Order value",
};

const YOUR_LIMITS_ID = "your-limits";

export function App() {
  const session = useMemo(() => new RiskSession(), []);
  const [limits, setLimits] = useState(DEFAULT_INPUT_LIMITS);
  const [order, setOrder] = useState(DEFAULT_INPUT_ORDER);
  const [snapshot, setSnapshot] = useState<SessionSnapshot>(session.snapshot());
  const [verdict, setVerdict] = useState<GateDecision | null>(null);
  const [inputError, setInputError] = useState<string | null>(null);
  const [rapidFireRunning, setRapidFireRunning] = useState(false);
  const rapidFireRun = useRef(0);
  const [theme, setTheme] = useState<ThemeMode>(readTheme);
  const [resolvedTheme, setResolvedTheme] = useState<ResolvedTheme>(() =>
    resolveTheme(readTheme()),
  );

  const refresh = useCallback(() => setSnapshot(session.snapshot()), [session]);

  useEffect(() => {
    const media = window.matchMedia("(prefers-color-scheme: dark)");
    const update = (): void => setResolvedTheme(applyTheme(theme));
    update();
    saveTheme(theme);
    media.addEventListener("change", update);
    return () => media.removeEventListener("change", update);
  }, [theme]);

  const parsedLimits = useMemo(() => parseLimits(limits), [limits]);
  const parsedOrder = useMemo(() => parseOrder(order), [order]);
  const orderValueAtoms = snapshot.lastOrderValueAtoms;
  const fundsUsedAtoms =
    parsedLimits === null
      ? null
      : parsedLimits.spotFundsLimit * 10_000n - snapshot.availableFundsAtoms;

  const applyLimits = useCallback(
    (next: InputLimits): DemoLimits | null => {
      setLimits(next);
      const parsed = parseLimits(next);
      if (parsed === null) {
        setInputError(
          "Limits must be positive whole numbers; rate must be 60/min or less.",
        );
        return null;
      }
      try {
        session.updateLimits(parsed);
        setVerdict(null);
        setInputError(null);
        refresh();
        return parsed;
      } catch (error) {
        // The engine kept its previous limits; show those, not the rejected
        // input.
        setLimits(inputLimitsFrom(session.appliedLimits()));
        setInputError(errorMessage(error));
        return null;
      }
    },
    [refresh, session],
  );

  const updateLimit = (key: keyof InputLimits, value: string): void => {
    setLimits({ ...limits, [key]: value });
    setVerdict(null);
    setInputError(null);
  };

  // A verdict belongs to the order that was sent; editing the next one
  // retires it.
  const editOrder = (next: InputOrder): void => {
    setOrder(next);
    setVerdict(null);
  };

  const commitLimit = (event: string): void => {
    if (applyLimits(limits) !== null) {
      trackLimitChange(event);
    }
  };

  const selectTheme = (nextTheme: ThemeMode): void => {
    trackPlaygroundEvent("risk_playground_theme_selected", {
      theme: nextTheme,
    });
    setTheme(nextTheme);
  };

  const submit = useCallback(
    (
      nextOrder: InputOrder,
      limitsOverride?: DemoLimits,
    ): GateDecision | null => {
      const parsed = parseOrder(nextOrder);
      if (
        parsed === null ||
        (limitsOverride === undefined && parsedLimits === null)
      ) {
        setInputError(
          "Quantity and limits must be positive whole numbers; price must be a positive decimal; rate must be 60/min or less.",
        );
        return null;
      }
      try {
        trackPlaygroundEvent("risk_playground_order_submitted", {
          side: nextOrder.side,
        });
        const decision = session.send(parsed);
        trackDecision(decision, nextOrder.side);
        setVerdict(decision);
        setInputError(null);
        refresh();
        return decision;
      } catch (error) {
        setInputError(errorMessage(error));
        return null;
      }
    },
    [parsedLimits, refresh, session],
  );

  const reset = (): void => {
    trackPlaygroundEvent("risk_playground_reset");
    session.reset();
    setLimits(DEFAULT_INPUT_LIMITS);
    setOrder(DEFAULT_INPUT_ORDER);
    setVerdict(null);
    setInputError(null);
    refresh();
  };

  const prepareScenario = (
    scenarioLimits: DemoLimits = DEFAULT_LIMITS,
  ): DemoLimits => {
    session.reset(scenarioLimits);
    setLimits(inputLimitsFrom(scenarioLimits));
    setOrder(DEFAULT_INPUT_ORDER);
    setVerdict(null);
    setInputError(null);
    refresh();
    return scenarioLimits;
  };

  const runNormal = (): void => {
    trackScenario("normal_buy");
    prepareScenario();
    setOrder(DEFAULT_INPUT_ORDER);
    submit(DEFAULT_INPUT_ORDER, DEFAULT_LIMITS);
  };

  const runFatFinger = (): void => {
    trackScenario("fat_finger");
    prepareScenario();
    const next = { ...DEFAULT_INPUT_ORDER, quantity: "5000" };
    setOrder(next);
    submit(next, DEFAULT_LIMITS);
  };

  const runOversizedValue = (): void => {
    trackScenario("oversized_value");
    prepareScenario();
    const next = { ...DEFAULT_INPUT_ORDER, quantity: "500" };
    setOrder(next);
    submit(next, DEFAULT_LIMITS);
  };

  const runSpotFundsLimit = (): void => {
    trackScenario("funds_limit");
    const scenarioLimits = { ...DEFAULT_LIMITS, spotFundsLimit: 1n };
    prepareScenario(scenarioLimits);
    setOrder(DEFAULT_INPUT_ORDER);
    submit(DEFAULT_INPUT_ORDER, scenarioLimits);
  };

  const runLossEvent = (): void => {
    trackScenario("realize_loss");
    prepareScenario();
    const buy = DEFAULT_INPUT_ORDER;
    const sell = { ...DEFAULT_INPUT_ORDER, price: "1", side: "SELL" as const };
    const buyDecision = submit(buy, DEFAULT_LIMITS);
    if (buyDecision?.accepted !== true) {
      setInputError("The loss scenario could not open its SPCX position.");
      return;
    }
    setOrder(sell);
    submit(sell, DEFAULT_LIMITS);
  };

  const runRapidFire = async (): Promise<void> => {
    if (rapidFireRunning) {
      return;
    }
    const run = ++rapidFireRun.current;
    const scenarioLimits = prepareScenario();
    trackScenario("rapid_fire");
    setRapidFireRunning(true);
    const rapidOrder = { ...DEFAULT_INPUT_ORDER, price: "1", quantity: "1" };
    setOrder(rapidOrder);

    try {
      for (let index = 0; index <= scenarioLimits.maxRate; index += 1) {
        if (rapidFireRun.current !== run) {
          break;
        }
        trackPlaygroundEvent("risk_playground_order_submitted", {
          side: "BUY",
        });
        const decision = session.send(parseOrderOrThrow(rapidOrder));
        trackDecision(decision, "BUY");
        setVerdict(decision);
        refresh();
        if (index < scenarioLimits.maxRate) {
          await wait(480);
        }
      }
    } catch (error) {
      setInputError(errorMessage(error));
    } finally {
      if (rapidFireRun.current === run) {
        setRapidFireRunning(false);
      }
    }
  };

  const stopRapidFire = (): void => {
    rapidFireRun.current += 1;
    setRapidFireRunning(false);
  };

  const checks =
    verdict === null || parsedLimits === null || parsedOrder === null
      ? []
      : checkViews(verdict, parsedLimits, parsedOrder, snapshot);
  const postTradeBlocked = verdict !== null && executionHalted(verdict);
  const accepted = verdict?.accepted === true && !postTradeBlocked;
  const statusWord = snapshot.halted ? "Halted" : "Live";

  return (
    <main className="page" data-density="comfortable">
      <div className="shell">
        <header className="masthead">
          <div className="brand-toolbar">
            <div className="brand-row">
              <a aria-label="OpenPit home" href="https://openpit.dev">
                <img
                  alt=""
                  className="brand-mark"
                  src={
                    resolvedTheme === "dark" ? brandMarkDark : brandMarkLight
                  }
                />
              </a>
              <a className="brand-home" href="https://openpit.dev">
                OpenPit
              </a>
              <span className="eyebrow">in-browser demo</span>
            </div>
            <ThemeSwitcher onSelect={selectTheme} theme={theme} />
          </div>
          <h1>The pre-trade gate, live</h1>
          <p className="lead">
            Set your limits, send an order, and watch the gate reserve funds,
            settle a fill, and update session risk - all in your browser. A
            trader, script, or AI agent <strong>can&apos;t</strong> get past
            your limits. Not &quot;shouldn&apos;t.&quot; Can&apos;t.
          </p>
          <p className="browser-note">
            Orders, balances, and P&amp;L run locally in your browser on{" "}
            <a href="https://openpit.dev">OpenPit</a> (WebAssembly).
          </p>
          <div className="engine-parity">
            <strong>One WASM binary. Browser and Node.js.</strong>
            <span>
              Run the same OpenPit engine on both sides. With matching built-in
              policies, state, market data, and event timing, identical orders
              produce identical verdicts - enabling local pre-checks before the
              server call.
            </span>
          </div>
          <p className="browser-note source-note">
            You can view the source code for this example{" "}
            <a
              href="https://github.com/openpitkit/pit/tree/main/examples/js/risk_playground"
              rel="noopener"
              target="_blank"
            >
              here
            </a>
            .
          </p>
          <div aria-label="How the playground works" className="steps">
            <span>
              <b>1</b> Reserve funds
            </span>
            <span>
              <b>2</b> Settle the fill
            </span>
            <span>
              <b>3</b> Read the report
            </span>
          </div>
        </header>

        <div className="grid">
          <div className="stack">
            <Card className="panel">
              <PanelTitle number="1" title="Send an order">
                Build an SPCX order, then send it through pre-trade, reserve,
                commit, and its immediate execution report.
              </PanelTitle>

              <div className="instrument-row">
                <span className="symbol">SPCX</span>
                <span className="instrument-name">
                  Space Exploration Technologies Corp.
                </span>
                <span className="mark">simulated mark $162.45</span>
              </div>

              <label className="field-label">Side</label>
              <div className="side-row" role="group" aria-label="Order side">
                <button
                  aria-pressed={order.side === "BUY"}
                  className={`side buy ${order.side === "BUY" ? "selected" : ""}`}
                  onClick={() => {
                    trackPlaygroundEvent(
                      "risk_playground_order_side_selected",
                      {
                        side: "BUY",
                      },
                    );
                    editOrder({ ...order, side: "BUY" });
                  }}
                  type="button"
                >
                  Buy
                </button>
                <button
                  aria-pressed={order.side === "SELL"}
                  className={`side sell ${order.side === "SELL" ? "selected" : ""}`}
                  onClick={() => {
                    trackPlaygroundEvent(
                      "risk_playground_order_side_selected",
                      {
                        side: "SELL",
                      },
                    );
                    editOrder({ ...order, side: "SELL" });
                  }}
                  type="button"
                >
                  Sell
                </button>
              </div>

              <div className="field-row">
                <NumberField
                  label="Quantity - shares"
                  onChange={(value) => editOrder({ ...order, quantity: value })}
                  value={order.quantity}
                />
                <NumberField
                  label="Price - USD"
                  inputMode="decimal"
                  min="0.0001"
                  onChange={(value) => editOrder({ ...order, price: value })}
                  step="0.01"
                  value={order.price}
                />
              </div>

              <div className="order-value">
                <span>Engine order value</span>
                <strong>
                  {orderValueAtoms === null
                    ? "reported after fill"
                    : formatMoneyAtoms(orderValueAtoms)}
                </strong>
              </div>
              <p className="fee-note">Fixed venue fee - $0.25 USD</p>

              <div className="actions">
                <Button
                  disabled={
                    parsedLimits === null ||
                    parsedOrder === null ||
                    rapidFireRunning
                  }
                  onClick={() => submit(order)}
                  style={{ height: 44, width: "100%" }}
                >
                  Send and settle
                </Button>
              </div>

              <div className="rule" />
              <span className="field-label">Or try a scenario</span>
              <div className="scenario-row">
                <ScenarioButton
                  color="var(--ok)"
                  disabled={rapidFireRunning}
                  onClick={runNormal}
                >
                  Normal buy
                </ScenarioButton>
                <ScenarioButton
                  color="var(--danger)"
                  disabled={rapidFireRunning}
                  onClick={runFatFinger}
                >
                  Fat finger
                </ScenarioButton>
                <ScenarioButton
                  color="var(--danger)"
                  disabled={rapidFireRunning}
                  onClick={runOversizedValue}
                >
                  Oversized value
                </ScenarioButton>
                <ScenarioButton
                  color="var(--danger)"
                  disabled={rapidFireRunning}
                  onClick={runSpotFundsLimit}
                >
                  Funds limit
                </ScenarioButton>
                <ScenarioButton
                  color="var(--danger)"
                  onClick={() => {
                    if (rapidFireRunning) {
                      stopRapidFire();
                    } else {
                      void runRapidFire();
                    }
                  }}
                >
                  {rapidFireRunning ? "Stop rapid fire" : "Rapid fire"}
                </ScenarioButton>
                <ScenarioButton
                  color="var(--danger)"
                  disabled={rapidFireRunning}
                  onClick={runLossEvent}
                >
                  Realize loss
                </ScenarioButton>
              </div>
            </Card>

            <Card className="panel" id={YOUR_LIMITS_ID}>
              <PanelTitle number="2" title="Your limits">
                Funds owns the balance and P&amp;L state. The other limits still
                run before an order reaches the simulated venue.
              </PanelTitle>

              <LimitControl
                description="Sets the opening USD balance in Funds"
                label="Funds cash allocation"
                meter={
                  parsedLimits === null
                    ? undefined
                    : {
                        current:
                          fundsUsedAtoms === null
                            ? null
                            : fundsUsedAtoms < 0n
                              ? 0n
                              : fundsUsedAtoms,
                        format: formatMoneyAtoms,
                        limit: parsedLimits.spotFundsLimit * 10_000n,
                      }
                }
                onChange={(value) => updateLimit("spotFundsLimit", value)}
                onCommit={() => commitLimit("spot_funds_cash_allocation")}
                value={limits.spotFundsLimit}
              />
              <LimitControl
                description="Biggest single order the gate will pass"
                label="Max order size"
                meter={
                  parsedLimits === null
                    ? undefined
                    : {
                        current: parsedOrder?.quantity ?? null,
                        format: formatIntegerBigInt,
                        limit: parsedLimits.maxShares,
                        unit: "shares",
                      }
                }
                onChange={(value) => updateLimit("maxShares", value)}
                onCommit={() => commitLimit("max_order_size")}
                value={limits.maxShares}
              />
              <LimitControl
                description="OpenPit reports the accepted order value after pre-trade"
                label="Max order value"
                meter={
                  parsedLimits === null
                    ? undefined
                    : {
                        current: orderValueAtoms,
                        format: formatMoneyAtoms,
                        limit: parsedLimits.maxValue * 10_000n,
                      }
                }
                onChange={(value) => updateLimit("maxValue", value)}
                onCommit={() => commitLimit("max_order_value")}
                value={limits.maxValue}
              />
              <LimitControl
                description="How fast orders may be sent"
                label="Max orders per minute"
                max={MAX_DEMO_RATE}
                meter={undefined}
                onChange={(value) => updateLimit("maxRate", value)}
                onCommit={() => commitLimit("max_order_rate")}
                value={limits.maxRate}
              />
              <LimitControl
                description="Halts the account once P&L falls below minus this, in USD"
                label="Max session loss"
                meter={
                  parsedLimits === null
                    ? undefined
                    : {
                        // Only a loss uses this one-sided limit; profit uses none.
                        current:
                          snapshot.pnlAtoms < 0n ? -snapshot.pnlAtoms : 0n,
                        format: formatMoneyAtoms,
                        limit: parsedLimits.lossLimit * 10_000n,
                      }
                }
                onChange={(value) => updateLimit("lossLimit", value)}
                onCommit={() => commitLimit("session_loss_limit")}
                value={limits.lossLimit}
              />
            </Card>
          </div>

          <div className="stack">
            <Card className="panel gate-panel">
              <PanelTitle number="3" title="The gate decides">
                Every order is checked against <LimitsLink /> before it reaches
                the simulated venue, then an execution report returns
                immediately with the fill.
              </PanelTitle>

              <div className="ribbon">
                <div>
                  <span>Account</span>
                  <Badge variant={snapshot.halted ? "danger" : "ok"}>
                    {statusWord}
                  </Badge>
                </div>
                <div>
                  <span>Funds available</span>
                  <strong
                    title={formatMoneyAtoms(snapshot.availableFundsAtoms)}
                  >
                    {formatMoneyAtoms(snapshot.availableFundsAtoms)}
                  </strong>
                </div>
                <div>
                  <span>SPCX available</span>
                  <strong
                    title={formatQuantityAtoms(snapshot.availableSharesAtoms)}
                  >
                    {formatQuantityAtoms(snapshot.availableSharesAtoms)}
                  </strong>
                </div>
                <div>
                  <span>P&amp;L</span>
                  <strong
                    className={snapshot.pnlAtoms < 0n ? "negative" : "positive"}
                    title={formatPnlAtoms(snapshot.pnlAtoms)}
                  >
                    {formatPnlAtoms(snapshot.pnlAtoms)}
                  </strong>
                </div>
                <div>
                  <span>Rate limit</span>
                  <strong>{parsedLimits?.maxRate ?? "-"} / min</strong>
                </div>
              </div>

              <Button
                className="gate-reset"
                disabled={rapidFireRunning}
                onClick={reset}
                style={{ height: 44, width: "100%" }}
                variant="outline"
              >
                Reset
              </Button>

              {inputError !== null && (
                <p className="input-error">{inputError}</p>
              )}

              <div aria-live="polite">
                {verdict === null ? (
                  <div className="empty-state">
                    <strong>Send an order to run the lifecycle</strong>
                    <p>
                      The gate checks the order against <LimitsLink />,
                      reserves Funds, commits the accepted order, then
                      immediately applies its execution report.
                    </p>
                  </div>
                ) : (
                  <section
                    className={`verdict ${accepted ? "accepted" : "blocked"}`}
                  >
                    <div className="verdict-head">
                      <h2>{verdictTitle(verdict)}</h2>
                      <span>
                        pre-trade + fill report
                        <br />
                        OpenPit WebAssembly engine
                      </span>
                    </div>
                    <p className="reason">{reasonFor(verdict)}</p>
                    {verdict.execution !== null && (
                      <section className="execution-report">
                        <span className="check-heading">
                          Execution report applied
                        </span>
                        <div>
                          <span>Fee</span>
                          <strong>
                            ${verdict.execution.feeAmount}{" "}
                            {verdict.execution.feeCurrency}
                          </strong>
                        </div>
                        <div>
                          <span>Settlement</span>
                          <strong>
                            {verdict.execution.accountBlockCode === null
                              ? "completed"
                              : "completed, account halted"}
                          </strong>
                        </div>
                      </section>
                    )}
                    <div className="check-list">
                      <span className="check-heading">
                        Checked against <LimitsLink />
                      </span>
                      {checks.map((check) => (
                        <CheckRow check={check} key={check.name} />
                      ))}
                    </div>
                  </section>
                )}
              </div>
            </Card>
          </div>
        </div>

        <footer>
          Copyright The Pit Project Owners. All rights reserved.{" "}
          <a href="https://openpit.dev/">https://openpit.dev</a>
        </footer>
      </div>
    </main>
  );
}

function ThemeSwitcher({
  onSelect,
  theme,
}: {
  readonly onSelect: (theme: ThemeMode) => void;
  readonly theme: ThemeMode;
}) {
  const options: readonly {
    readonly label: string;
    readonly value: ThemeMode;
  }[] = [
    { label: "System", value: "system" },
    { label: "Light", value: "light" },
    { label: "Dark", value: "dark" },
  ];
  return (
    <div aria-label="Color theme" className="theme-switcher" role="group">
      {options.map((option) => (
        <button
          aria-pressed={theme === option.value}
          className={theme === option.value ? "selected" : ""}
          key={option.value}
          onClick={() => onSelect(option.value)}
          type="button"
        >
          {option.label}
        </button>
      ))}
    </div>
  );
}

function PanelTitle({
  children,
  number,
  title,
}: {
  readonly children: ReactNode;
  readonly number: string;
  readonly title: string;
}) {
  return (
    <header className="panel-title">
      <h2>
        <b>{number}</b>
        {title}
      </h2>
      <p>{children}</p>
    </header>
  );
}

/** Points a verdict back at the panel that configures the checks. */
function LimitsLink() {
  return (
    <a className="limits-link" href={`#${YOUR_LIMITS_ID}`}>
      your limits
    </a>
  );
}

function NumberField({
  inputMode = "numeric",
  label,
  min = "1",
  onChange,
  step = "1",
  value,
}: {
  readonly inputMode?: "decimal" | "numeric";
  readonly label: string;
  readonly min?: string;
  readonly onChange: (value: string) => void;
  readonly step?: string;
  readonly value: string;
}) {
  const id = `field-${label.toLowerCase().replaceAll(/[^a-z]+/g, "-")}`;
  return (
    <div>
      <label className="field-label" htmlFor={id}>
        {label}
      </label>
      <input
        id={id}
        className="ph-no-capture"
        inputMode={inputMode}
        min={min}
        onChange={(event) => onChange(event.target.value)}
        step={step}
        type="number"
        value={value}
      />
    </div>
  );
}

function ScenarioButton({
  children,
  color,
  disabled = false,
  onClick,
}: {
  readonly children: string;
  readonly color: string;
  readonly disabled?: boolean;
  readonly onClick: () => void;
}) {
  return (
    <button
      className="scenario"
      disabled={disabled}
      onClick={onClick}
      type="button"
    >
      <i style={{ background: color }} />
      {children}
    </button>
  );
}

function LimitControl({
  description,
  label,
  max,
  meter,
  onChange,
  onCommit,
  value,
}: {
  readonly description: string;
  readonly label: string;
  readonly max?: bigint;
  readonly meter: LimitMeterConfig | undefined;
  readonly onChange: (value: string) => void;
  readonly onCommit: () => void;
  readonly value: string;
}) {
  const limit = parsePositiveWhole(value);
  const valid = limit !== null && (max === undefined || limit <= max);
  return (
    <div className="limit-control">
      <div className="limit-copy">
        <div>
          <strong>{label}</strong>
          <span>{description}</span>
        </div>
        <input
          aria-label={label}
          aria-invalid={!valid}
          className="ph-no-capture"
          inputMode="numeric"
          max={max?.toString()}
          min="1"
          onBlur={onCommit}
          onChange={(event) => onChange(event.target.value)}
          step="1"
          type="number"
          value={value}
        />
      </div>
      {!valid ? (
        <p className="limit-invalid">
          {max === undefined
            ? "Enter a positive whole number to apply this limit."
            : `Enter a positive whole number no greater than ${max}.`}
        </p>
      ) : (
        <>
          {meter !== undefined && <LimitMeter {...meter} />}
          <p className="limit-engine-note">
            OpenPit applies this limit during pre-trade.
          </p>
        </>
      )}
    </div>
  );
}

function LimitMeter({ current, format, limit, unit = "" }: LimitMeterConfig) {
  const percentage =
    current === null || limit <= 0n
      ? null
      : Math.min(100, Number((current * 100n) / limit));
  return (
    <div className="limit-meter">
      <div className="limit-meter-meta">
        <strong>
          {current === null ? "Engine report pending" : format(current)}
        </strong>
        <span>
          / {format(limit)}
          {unit === "" ? "" : ` ${unit}`}
        </span>
        {percentage !== null && <b>{percentage}%</b>}
      </div>
      <div
        aria-label={`${format(limit)} limit usage`}
        aria-valuemax={100}
        aria-valuemin={0}
        aria-valuenow={percentage ?? 0}
        className="limit-meter-track"
        role="progressbar"
      >
        <span style={{ width: `${percentage ?? 0}%` }} />
      </div>
    </div>
  );
}

function CheckRow({ check }: { readonly check: CheckView }) {
  const icon =
    check.state === "skip" ? (
      "-"
    ) : (
      <svg
        fill="none"
        height="0.75em"
        stroke="currentColor"
        strokeLinecap="round"
        strokeLinejoin="round"
        strokeWidth="2"
        viewBox="0 0 12 12"
        width="0.75em"
      >
        <path
          d={
            check.state === "pass" ? "M2 6.5 5 9.5 10 3" : "M3 3 9 9M9 3 3 9"
          }
        />
      </svg>
    );
  return (
    <div className={`check ${check.state}`}>
      <span aria-hidden="true">{icon}</span>
      <div>
        <strong>{check.name}</strong>
        <p>{check.detail}</p>
      </div>
    </div>
  );
}

function checkViews(
  verdict: GateDecision,
  limits: DemoLimits,
  order: DemoOrder,
  snapshot: SessionSnapshot,
): CheckView[] {
  return CHECK_ORDER.map((name) => {
    const reject = rejectForCheck(verdict.rejects, name);
    const state =
      reject !== undefined || (name === "pnl" && executionHalted(verdict))
        ? "fail"
        : verdict.accepted
          ? "pass"
          : "skip";
    return {
      name: CHECK_LABELS[name],
      state,
      detail: checkDetail(
        name,
        limits,
        order,
        snapshot,
        verdict,
        reject,
        state,
      ),
    };
  });
}

function checkDetail(
  name: CheckName,
  limits: DemoLimits,
  order: DemoOrder,
  snapshot: SessionSnapshot,
  verdict: GateDecision,
  reject: GateReject | undefined,
  state: CheckView["state"],
): string {
  if (reject !== undefined) {
    return reject.details === ""
      ? reject.reason
      : `${reject.reason} - ${reject.details}`;
  }
  if (state === "skip") {
    return "The engine response contains no separate verdict for this check";
  }
  if (name === "pnl") {
    if (executionHalted(verdict)) {
      return "pre-trade passed; the execution report then tripped the P&L axis";
    }
    return `OpenPit reported P&L ${formatPnlAtoms(snapshot.pnlAtoms)} against the -${formatUsd(
      limits.lossLimit,
    )} loss limit`;
  }
  if (name === "funds") {
    if (order.side === "SELL") {
      return `${formatIntegerBigInt(order.quantity)} SPCX shares are available before settlement`;
    }
    return "Funds accepted this order against its available balance";
  }
  if (name === "size") {
    return `${formatIntegerBigInt(order.quantity)} <= ${formatIntegerBigInt(limits.maxShares)} share limit`;
  }
  if (name === "value") {
    return `OpenPit accepted the order against the ${formatUsd(limits.maxValue)} value limit`;
  }
  return `OpenPit accepted this order under the ${limits.maxRate}/min rate limit`;
}

function rejectForCheck(
  rejects: readonly GateReject[],
  check: CheckName,
): GateReject | undefined {
  return rejects.find((reject) => reject.checks.includes(check));
}

function verdictTitle(verdict: GateDecision): string {
  if (!verdict.accepted) {
    return "BLOCKED";
  }
  return executionHalted(verdict) ? "FILLED - HALTED" : "FILLED";
}

function reasonFor(verdict: GateDecision): string {
  if (verdict.accepted) {
    if (executionHalted(verdict)) {
      return "Pre-trade passed and settled the fill. Its execution report then pushed P&L past the loss limit and halted the account.";
    }
    return "Pre-trade passed. Funds reserved the balance, committed the order, and settled the immediate execution report.";
  }
  const reasons = [...new Set(verdict.rejects.map((reject) => reject.reason))];
  return reasons.length === 0
    ? "Blocked by OpenPit."
    : `Blocked - ${reasons.join("; ")}.`;
}

function executionHalted(verdict: GateDecision): boolean {
  return (
    verdict.execution !== null && verdict.execution.accountBlockCode !== null
  );
}

function parseLimits(input: InputLimits): DemoLimits | null {
  const lossLimit = parsePositiveWhole(input.lossLimit);
  const maxRate = parseRate(input.maxRate);
  const maxShares = parsePositiveWhole(input.maxShares);
  const maxValue = parsePositiveWhole(input.maxValue);
  const spotFundsLimit = parsePositiveWhole(input.spotFundsLimit);
  if (
    lossLimit === null ||
    maxRate === null ||
    maxShares === null ||
    maxValue === null ||
    spotFundsLimit === null
  ) {
    return null;
  }
  return { lossLimit, maxRate, maxShares, maxValue, spotFundsLimit };
}

function inputLimitsFrom(limits: DemoLimits): InputLimits {
  return {
    lossLimit: limits.lossLimit.toString(),
    maxRate: limits.maxRate.toString(),
    maxShares: limits.maxShares.toString(),
    maxValue: limits.maxValue.toString(),
    spotFundsLimit: limits.spotFundsLimit.toString(),
  };
}

function parseOrder(input: InputOrder): DemoOrder | null {
  const quantity = parsePositiveWhole(input.quantity);
  const price = parsePositiveDecimal(input.price);
  if (quantity === null || price === null) {
    return null;
  }
  return { price, quantity, side: input.side };
}

function parseOrderOrThrow(input: InputOrder): DemoOrder {
  const order = parseOrder(input);
  if (order === null) {
    throw new RangeError("the rapid-fire scenario requires a valid order");
  }
  return order;
}

function parsePositiveWhole(value: string): bigint | null {
  if (!/^\d+$/.test(value)) {
    return null;
  }
  const parsed = BigInt(value);
  return parsed > 0n ? parsed : null;
}

function parsePositiveDecimal(value: string): string | null {
  return /^\d+(?:\.\d{1,4})?$/.test(value) && /[1-9]/.test(value)
    ? value
    : null;
}

function parseRate(value: string): number | null {
  const parsed = parsePositiveWhole(value);
  if (parsed === null || parsed > MAX_DEMO_RATE) {
    return null;
  }
  return Number(parsed);
}

function formatIntegerBigInt(value: bigint): string {
  return value.toLocaleString("en-US");
}

function formatUsd(value: bigint): string {
  return `$${formatIntegerBigInt(value)}`;
}

function formatMoneyAtoms(value: bigint): string {
  const sign = value < 0n ? "-" : "";
  const absoluteValue = value < 0n ? -value : value;
  const whole = absoluteValue / 10_000n;
  const fraction = (absoluteValue % 10_000n)
    .toString()
    .padStart(4, "0")
    .replace(/0+$/, "");
  return `${sign}$${formatIntegerBigInt(whole)}${fraction === "" ? "" : `.${fraction}`}`;
}

function formatPnlAtoms(value: bigint): string {
  return value === 0n
    ? "$0"
    : `${value < 0n ? "" : "+"}${formatMoneyAtoms(value)}`;
}

function formatQuantityAtoms(value: bigint): string {
  const sign = value < 0n ? "-" : "";
  const absoluteValue = value < 0n ? -value : value;
  const whole = absoluteValue / 10_000n;
  const fraction = (absoluteValue % 10_000n)
    .toString()
    .padStart(4, "0")
    .replace(/0+$/, "");
  return `${sign}${formatIntegerBigInt(whole)}${fraction === "" ? "" : `.${fraction}`}`;
}

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

function wait(milliseconds: number): Promise<void> {
  return new Promise((resolve) => window.setTimeout(resolve, milliseconds));
}

function trackDecision(decision: GateDecision, side: DemoOrder["side"]): void {
  trackPlaygroundEvent("risk_playground_gate_decided", {
    outcome: decision.accepted ? "accepted" : "blocked",
    side,
  });
  if (decision.execution !== null) {
    trackPlaygroundEvent("risk_playground_execution_report_applied", {
      account_halted: decision.execution.accountBlockCode !== null,
      side,
    });
  }
}

function trackLimitChange(limit: string): void {
  trackPlaygroundEvent("risk_playground_limit_updated", { limit });
}

function trackScenario(scenario: string): void {
  trackPlaygroundEvent("risk_playground_scenario_selected", { scenario });
}
