# Copyright The Pit Project Owners. All rights reserved.
# SPDX-License-Identifier: Apache-2.0
#
# Licensed under the Apache License, Version 2.0 (the "License");
# you may not use this file except in compliance with the License.
# You may obtain a copy of the License at
#
#     http://www.apache.org/licenses/LICENSE-2.0
#
# Unless required by applicable law or agreed to in writing, software
# distributed under the License is distributed on an "AS IS" BASIS,
# WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
# See the License for the specific language governing permissions and
# limitations under the License.
#
# Please see https://github.com/openpitkit and the OWNERS file for details.

"""Example spot_table.

Runs a tabular spot-policy scenario against a single sequential ``no_sync``
engine - operation by operation, in the row order written in the table - and
prints a summary report with operation counts, total wall-clock time, and
order/report latency statistics. With ``--min-duration d`` it repeats the
scenario until at least d of wall-clock time has elapsed (a repeat run),
printing a periodic progress block with the engine's running order/report
latency, then a final aggregate summary. The scenario tables live under
examples/tables/spot/.
"""

from __future__ import annotations

import argparse
import os
import sys
import time

import runner
import table
from platform_info import print_platform
from runner import MODE_SYNC, EngineAggregate, Report

# defaultTimeout bounds a single pass of the scenario through the engine.
_DEFAULT_TIMEOUT = "30s"

# How often the repeat run prints a progress block.
_REPEAT_LOG_INTERVAL_S = 10.0


# ---------------------------------------------------------------------------
# CLI
# ---------------------------------------------------------------------------


def parse_duration(s: str) -> float:
    """Parse a duration into seconds.

    Accepts a bare number (seconds) or a value with a ``s``/``m``/``h`` suffix,
    so ``--min-duration 3m`` works. Raises ValueError on an invalid value.
    """
    text = s.strip()
    if not text:
        raise ValueError("duration is empty")
    unit = text[-1]
    scale = {"s": 1.0, "m": 60.0, "h": 3600.0}.get(unit)
    if scale is None:
        return float(text)
    return float(text[:-1]) * scale


def build_parser() -> argparse.ArgumentParser:
    """Build the CLI argument parser."""
    parser = argparse.ArgumentParser(
        prog="spot_table",
        description=(
            "Run a spot-policy scenario table through a single sequential"
            " no_sync engine. Scenario tables live under examples/tables/spot/"
            " (e.g. coverage.md); see examples/tables/spot/README.md for the"
            " table format."
        ),
    )
    parser.add_argument(
        "--table",
        required=True,
        help=(
            "path to the scenario table (Markdown with front-matter); required."
            " See examples/tables/spot/README.md for the table format."
        ),
    )
    parser.add_argument(
        "--timeout",
        default=_DEFAULT_TIMEOUT,
        help="timeout for a single pass of the table (default 30s)",
    )
    parser.add_argument(
        "--min-duration",
        dest="min_duration",
        default="0",
        help=(
            "if > 0, repeat the scenario until this much wall-clock elapses"
            " (repeat run)"
        ),
    )
    return parser


def resolve_table_path(p: str) -> str:
    """Resolve a table path: as-is when it exists, else alongside the script.

    A relative path resolves whether the example is run from the repository
    root or from its own directory.
    """
    if os.path.exists(p):
        return os.path.abspath(p)
    nearby = os.path.join(os.path.dirname(os.path.abspath(__file__)), p)
    if os.path.exists(nearby):
        return nearby
    raise FileNotFoundError(f"table {p!r} not found (cwd={os.getcwd()})")


# ---------------------------------------------------------------------------
# Single pass
# ---------------------------------------------------------------------------


def run_once(table_path: str, parsed: table.Table, timeout_s: float) -> int:
    """Run the scenario once and print the summary report."""
    print_platform()
    deadline_ns = time.perf_counter_ns() + int(timeout_s * 1e9)
    report = runner.run_sync(parsed.fm, parsed.rows, deadline_ns)

    print(
        f"Scenario: {parsed.fm.name} ({os.path.basename(table_path)}),"
        f" slippage {parsed.fm.slippage_bps} bps\n"
    )
    print_legend()
    print_report(report)
    return 1 if report.first_fail is not None else 0


def run_repeat(
    table_path: str, parsed: table.Table, timeout_s: float, min_dur_s: float
) -> int:
    """Re-run the scenario until at least *min_dur_s* of wall-clock has elapsed.

    Fails fast on the first mismatch. Every ~10 s it prints a progress block
    with the engine's running order/report latency; on completion it prints the
    platform and an aggregate summary.
    """
    print(
        f"Repeat: {parsed.fm.name} ({os.path.basename(table_path)}),"
        f" running for at least {_format_seconds(min_dur_s)} ...\n"
    )

    agg = EngineAggregate()
    start = time.monotonic()
    last_log = start
    iterations = 0
    while True:
        deadline_ns = time.perf_counter_ns() + int(timeout_s * 1e9)
        report = runner.run_sync(parsed.fm, parsed.rows, deadline_ns)
        iterations += 1

        if report.first_fail is not None:
            print_report(report)
            elapsed = time.monotonic() - start
            print(
                f"repeat run failed on iteration {iterations}"
                f" after {_format_seconds(elapsed)}"
            )
            return 1
        agg.add(report)

        now = time.monotonic()
        elapsed = now - start
        if now - last_log >= _REPEAT_LOG_INTERVAL_S:
            print_heartbeat(iterations, elapsed, min_dur_s, agg)
            last_log = now
        if elapsed >= min_dur_s:
            # Platform info heads the final report, not the progress blocks.
            print_platform()
            print_repeat_summary(iterations, elapsed, agg)
            return 0


# ---------------------------------------------------------------------------
# Printing
# ---------------------------------------------------------------------------


def print_legend() -> None:
    """Explain every field of the report once, so the output stands alone."""
    print("Legend:")
    print(
        "  operations  - table rows applied to the engine"
        " (SEED/GROUP/ORDER/FILL; market-data ticks excluded)"
    )
    print("  accounts    - distinct accounts touched by the scenario")
    print("  total time  - wall-clock to run the whole scenario on this engine")
    print(
        "  order check - time to decide one order"
        " (the pre-trade ACCEPT/REJECT check); n = orders checked"
    )
    print(
        "  reports     - time to apply one fill / execution report;"
        " n = reports applied"
    )
    print()


def print_report(report: Report) -> None:
    """Render the engine's outcome with the legend's field names."""
    print(f"== {engine_title(report.mode)} ==")
    print(f"  operations  : {report.total}")
    print(f"  accounts    : {report.accounts_count()}")
    print(f"  total time  : {_format_ns(report.wall_clock_ns)}")
    print_latency("  order check ", report.order)
    print_latency("  reports     ", report.fill)
    if report.first_fail is not None:
        fail = report.first_fail
        print(
            f"  result      : FAILED at line {fail.row.line}"
            f" ({fail.row.account}, {fail.row.action}): {fail.message}\n"
        )
        return
    print("  result      : ALL PASS")
    print()


def print_latency(label: str, stats: runner.LatencyStats) -> None:
    """Print one latency line, or ``none`` when no samples were taken."""
    if stats.count == 0:
        print(f"{label}: none")
        return
    print(
        f"{label}: n={stats.count}  min={_format_ns(stats.min_ns)}"
        f"  avg={_format_ns(stats.avg_ns())}  max={_format_ns(stats.max_ns)}"
    )


def print_aggregate(agg: EngineAggregate, elapsed_s: float) -> None:
    """Report the engine's aggregate statistics over the repeat run."""
    print(f"== {engine_title(agg.mode)} ==")
    print(f"  operations  : {agg.ops} total across the repeat run")
    print(f"  accounts    : {agg.accounts}")
    print(f"  total time  : {_format_seconds(elapsed_s)} (whole repeat run)")
    print_latency("  order check ", agg.order)
    print_latency("  reports     ", agg.fill)
    print()


def print_repeat_summary(
    iterations: int, elapsed_s: float, agg: EngineAggregate
) -> None:
    """Report aggregate statistics over the whole repeat run."""
    print(
        f"Repeat summary: {iterations} iterations in {_format_seconds(elapsed_s)},"
        f" the engine passed every time\n"
    )
    print_legend()
    print_aggregate(agg, elapsed_s)


def print_heartbeat(
    iterations: int, elapsed_s: float, min_dur_s: float, agg: EngineAggregate
) -> None:
    """Print one progress block during a repeat run."""
    left = min_dur_s - elapsed_s
    if left < 0:
        left = 0.0
    clock = time.strftime("%H:%M:%S")
    print(
        f"── {clock} · {iterations} iter · elapsed {_format_seconds(elapsed_s)}"
        f" · left {_format_seconds(left)} ──"
    )
    o = agg.order
    r = agg.fill
    print(
        f"  sync · ord {_format_ns(o.min_ns)}/{_format_ns(o.avg_ns())}"
        f"/{_format_ns(o.max_ns)} · rpt {_format_ns(r.min_ns)}"
        f"/{_format_ns(r.avg_ns())}/{_format_ns(r.max_ns)}"
    )


def engine_title(mode: str) -> str:
    """Give the mode a self-describing header."""
    if mode == MODE_SYNC:
        return "sequential engine (sync)"
    return mode


def _format_ns(value_ns: int) -> str:
    """Render a nanosecond count as a human-readable duration string."""
    if value_ns >= 1_000_000_000:
        return f"{value_ns / 1_000_000_000:.3f}s"
    if value_ns >= 1_000_000:
        return f"{value_ns / 1_000_000:.3f}ms"
    if value_ns >= 1_000:
        return f"{value_ns / 1_000:.3f}µs"
    return f"{value_ns}ns"


def _format_seconds(value_s: float) -> str:
    """Render a wall-clock span (seconds) sensibly as s or ms."""
    if value_s >= 1.0:
        return f"{value_s:.3f}s"
    return f"{value_s * 1000:.3f}ms"


# ---------------------------------------------------------------------------
# Entry point
# ---------------------------------------------------------------------------


def main() -> int:
    """Parse arguments, resolve the table, and run once or repeatedly."""
    parser = build_parser()
    args = parser.parse_args()

    try:
        timeout_s = parse_duration(args.timeout)
        min_dur_s = parse_duration(args.min_duration)
    except ValueError as exc:
        parser.error(str(exc))

    try:
        resolved = resolve_table_path(args.table)
    except FileNotFoundError as exc:
        print(f"resolve table: {exc}", file=sys.stderr)
        return 1
    try:
        parsed = table.parse_file(resolved)
    except (OSError, ValueError) as exc:
        print(f"parse: {exc}", file=sys.stderr)
        return 1

    if min_dur_s > 0:
        return run_repeat(resolved, parsed, timeout_s, min_dur_s)
    return run_once(resolved, parsed, timeout_s)


if __name__ == "__main__":
    sys.exit(main())
