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

// The terminal half of the demo: an xterm.js line editor that parses typed
// commands and drives the in-browser risk engine in `session.ts`. There is no
// server and no network - the engine is the wasm module bundled into this page.

import { Terminal } from "@xterm/xterm";
import "@xterm/xterm/css/xterm.css";

import { Session } from "./session.ts";
import type { PlaceOutcome, RejectRow } from "./session.ts";

// ANSI colour helpers. Keeping them tiny avoids an extra dependency and makes
// the engine output readable against the dark page.
const RESET = "\x1b[0m";
const BOLD = "\x1b[1m";
const DIM = "\x1b[2m";
const GREEN = "\x1b[32m";
const RED = "\x1b[31m";
const YELLOW = "\x1b[33m";
const CYAN = "\x1b[36m";
const BLUE = "\x1b[34m";

const PROMPT = `${CYAN}openpit${RESET} ${DIM}$${RESET} `;

/** A parsed command plus its positional arguments. */
interface ParsedCommand {
  readonly name: string;
  readonly args: string[];
}

/**
 * The interactive controller: owns the terminal, the engine session, and the
 * line-editing state. Each typed line is dispatched to a command handler that
 * makes real engine calls and prints the result.
 */
class TerminalApp {
  private readonly term: Terminal;
  private readonly session = new Session();
  private line = "";

  constructor(container: HTMLElement) {
    this.term = new Terminal({
      convertEol: true,
      cursorBlink: true,
      fontFamily:
        '"SF Mono", "JetBrains Mono", "Fira Code", Menlo, Consolas, monospace',
      fontSize: 13,
      theme: {
        background: "#11151f",
        foreground: "#c5c8c6",
        cursor: "#7aa2f7",
      },
    });
    this.term.open(container);
    this.term.onData((data) => this.onData(data));

    this.banner();
    this.prompt();
  }

  // ---------------------------------------------------------------------------
  // Line editing. xterm.js is a raw terminal: it delivers keystrokes, and we
  // maintain the current line, echo printable characters, and handle Enter and
  // Backspace ourselves.
  // ---------------------------------------------------------------------------

  private onData(data: string): void {
    for (const ch of data) {
      const code = ch.codePointAt(0) ?? 0;
      if (ch === "\r") {
        this.term.write("\r\n");
        this.runLine(this.line.trim());
        this.line = "";
        this.prompt();
      } else if (ch === "\x7f") {
        // Backspace: erase one character from the buffer and the screen.
        if (this.line.length > 0) {
          this.line = this.line.slice(0, -1);
          this.term.write("\b \b");
        }
      } else if (ch === "\x03") {
        // Ctrl-C: abandon the current line.
        this.term.write("^C\r\n");
        this.line = "";
        this.prompt();
      } else if (code >= 0x20) {
        // Printable character: buffer and echo it.
        this.line += ch;
        this.term.write(ch);
      }
    }
  }

  private prompt(): void {
    this.term.write(PROMPT);
  }

  private writeln(text = ""): void {
    this.term.write(`${text}\r\n`);
  }

  // ---------------------------------------------------------------------------
  // Command dispatch.
  // ---------------------------------------------------------------------------

  private runLine(input: string): void {
    if (input === "") {
      return;
    }
    const parsed = parse(input);
    try {
      this.dispatch(parsed);
    } catch (err) {
      // Engine input-validation and misuse surface as typed JS errors; show the
      // name and message rather than crashing the terminal.
      const name = err instanceof Error ? err.name : "Error";
      const message = err instanceof Error ? err.message : String(err);
      this.writeln(`${RED}${name}${RESET}: ${message}`);
    }
  }

  private dispatch(cmd: ParsedCommand): void {
    switch (cmd.name) {
      case "help":
        this.cmdHelp();
        return;
      case "config":
        this.cmdConfig();
        return;
      case "balance":
        this.cmdBalance();
        return;
      case "quote":
        this.cmdQuote();
        return;
      case "place":
        this.cmdPlace(cmd.args);
        return;
      case "fill":
        this.cmdFill(cmd.args);
        return;
      case "clear":
        this.term.clear();
        return;
      default:
        this.writeln(
          `${RED}unknown command${RESET} ${BOLD}${cmd.name}${RESET}` +
            ` - type ${CYAN}help${RESET}`,
        );
    }
  }

  // ---------------------------------------------------------------------------
  // Commands. Each one calls the engine session and renders the outcome.
  // ---------------------------------------------------------------------------

  private cmdHelp(): void {
    const rows: [string, string][] = [
      ["help", "show this help"],
      ["config", "print the engine configuration"],
      ["balance", "show available and held settlement cash"],
      ["quote", "read the live top-of-book quote from market data"],
      ["place <BUY|SELL> <qty> [price]", "run the full pre-trade flow and commit"],
      ["fill [pnl]", "settle the last BUY, booking realized P&L"],
      ["clear", "clear the screen"],
    ];
    this.writeln(`${BOLD}Commands${RESET}`);
    for (const [name, desc] of rows) {
      this.writeln(`  ${CYAN}${name.padEnd(34)}${RESET}${DIM}${desc}${RESET}`);
    }
    this.writeln();
    this.writeln(
      `${DIM}Try: ${RESET}place BUY 3${DIM} then ${RESET}place BUY 3` +
        `${DIM} (the held cash makes the second one fail), then ${RESET}fill -60000` +
        `${DIM} to trip the kill switch.${RESET}`,
    );
  }

  private cmdConfig(): void {
    this.writeln(`${BOLD}Engine configuration${RESET}`);
    for (const line of this.session.config()) {
      this.writeln(`  ${BLUE}${line.label.padEnd(12)}${RESET}${line.value}`);
    }
  }

  private cmdBalance(): void {
    const balance = this.session.balance();
    this.writeln(
      `available ${GREEN}${balance.available}${RESET} USD` +
        `   held ${YELLOW}${balance.held}${RESET} USD`,
    );
  }

  private cmdQuote(): void {
    const quote = this.session.quote();
    this.writeln(
      `BTC/USD   bid ${GREEN}${quote.bid}${RESET}` +
        `   ask ${RED}${quote.ask}${RESET}` +
        `   mark ${CYAN}${quote.mark}${RESET}`,
    );
  }

  private cmdPlace(args: string[]): void {
    const side = (args[0] ?? "").toUpperCase();
    if (side !== "BUY" && side !== "SELL") {
      this.writeln(`${RED}usage${RESET}: place <BUY|SELL> <qty> [price]`);
      return;
    }
    const quantity = args[1] ?? "1";
    const price = args[2] ?? Session.defaultPrice;

    const outcome = this.session.place(side, quantity, price);
    this.renderPlace(outcome);
  }

  private renderPlace(outcome: PlaceOutcome): void {
    const head = `${outcome.side} ${outcome.quantity} BTC @ ${outcome.price}` +
      ` (${outcome.notional} USD)`;
    if (outcome.accepted) {
      this.writeln(`${GREEN}ACCEPTED${RESET} ${head}`);
      this.writeln(
        `  ${DIM}committed reservation;${RESET}` +
          ` available ${GREEN}${outcome.availableAfter}${RESET}` +
          ` held ${YELLOW}${outcome.heldAfter}${RESET}`,
      );
      return;
    }
    this.writeln(`${RED}REJECTED${RESET} ${head}`);
    this.renderRejects(outcome.rejects);
  }

  private renderRejects(rejects: RejectRow[]): void {
    if (rejects.length === 0) {
      this.writeln(`  ${DIM}(no rejects reported)${RESET}`);
      return;
    }
    for (const reject of rejects) {
      const highlight = reject.code === Session.insufficientFunds ? YELLOW : RED;
      this.writeln(
        `  ${highlight}${reject.policy} [${reject.code}]${RESET}: ` +
          `${reject.reason}${reject.details ? ` ${DIM}(${reject.details})${RESET}` : ""}`,
      );
    }
  }

  private cmdFill(args: string[]): void {
    const pnl = args[0] ?? "0";
    const outcome = this.session.fill(pnl);
    if (outcome.blocked) {
      this.writeln(
        `${RED}${BOLD}KILL SWITCH${RESET} fill settled ${outcome.settled} USD,` +
          ` then the account was blocked`,
      );
      this.writeln(`  ${RED}${outcome.blockReason}${RESET}`);
      this.writeln(`  ${DIM}further orders for this account would be rejected.${RESET}`);
      return;
    }
    this.writeln(
      `${GREEN}FILLED${RESET} settled ${outcome.settled} USD reservation,` +
        ` realized P&L ${pnl}, no account block`,
    );
  }

  private banner(): void {
    this.writeln(`${BOLD}${CYAN}OpenPit${RESET} ${DIM}pre-trade risk engine${RESET}`);
    this.writeln(
      `${DIM}Running entirely in your browser as WebAssembly - no backend.${RESET}`,
    );
    this.writeln(`${DIM}Type ${RESET}help${DIM} to begin.${RESET}`);
    this.writeln();
  }
}

/** Split a line into a command name and whitespace-separated arguments. */
function parse(input: string): ParsedCommand {
  const parts = input.split(/\s+/).filter((part) => part.length > 0);
  const name = (parts[0] ?? "").toLowerCase();
  return { name, args: parts.slice(1) };
}

const container = document.getElementById("terminal");
if (container === null) {
  throw new Error("missing #terminal mount point");
}
// Instantiating the app builds the engine synchronously (the wasm is inlined in
// the browser bundle and initialized at import) - no await, no loading state.
new TerminalApp(container);
