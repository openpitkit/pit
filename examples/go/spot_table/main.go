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
// Please see https://github.com/openpitkit and the OWNERS file for details.

// Example spot_table runs a tabular spot-policy scenario against the engine in
// two isolated runs — a sequential NoSync engine and a parallel AccountSync
// engine wrapped in asyncengine — and prints a per-engine summary report with
// operation counts, total wall-clock time, and order/report latency statistics.
// With -min-duration d it repeats the scenario until at least d of wall-clock
// time has elapsed (a repeat run), printing a periodic progress block with each
// engine's running order/report latency, then a final per-engine aggregate
// summary. The scenario tables live under examples/tables/spot/.
package main

import (
	"context"
	"flag"
	"fmt"
	"log"
	"os"
	"path/filepath"
	"sync"
	"time"
)

// defaultTimeout bounds a single pass of the scenario through one engine.
const defaultTimeout = 30 * time.Second

// repeatLogInterval is how often the repeat run prints a progress block showing
// each engine's running order/report latency statistics.
const repeatLogInterval = 10 * time.Second

func main() {
	log.SetFlags(0)
	tablePath := flag.String(
		"table", "",
		"path to the scenario table (Markdown with front-matter); required",
	)
	timeout := flag.Duration(
		"timeout", defaultTimeout, "timeout for a single pass of the table",
	)
	minDuration := flag.Duration(
		"min-duration", 0,
		"if > 0, repeat the scenario until this much wall-clock elapses (repeat run)",
	)
	flag.Parse()

	if *tablePath == "" {
		log.Fatal("error: -table is required\n" +
			"usage: go run . -table <path/to/table.md> [-timeout d] [-min-duration d]\n" +
			"  scenario tables live under examples/tables/spot/ (e.g. coverage.md);\n" +
			"  see examples/tables/spot/README.md for the table format")
	}

	resolved, err := resolveTablePath(*tablePath)
	if err != nil {
		log.Fatalf("resolve table: %v", err)
	}
	table, err := ParseFile(resolved)
	if err != nil {
		log.Fatalf("parse: %v", err)
	}

	if *minDuration > 0 {
		err = runRepeat(resolved, table, *timeout, *minDuration)
	} else {
		err = runOnce(resolved, table, *timeout)
	}
	if err != nil {
		log.Fatal(err)
	}
}

// runPair runs the scenario through both engines concurrently and returns each
// engine's report (and any transport error).
func runPair(ctx context.Context, table *Table) (syncReport, asyncReport *Report, syncErr, asyncErr error) {
	var wg sync.WaitGroup
	wg.Add(2)
	go func() {
		defer wg.Done()
		syncReport, syncErr = RunSync(ctx, table.FM, table.Rows)
	}()
	go func() {
		defer wg.Done()
		asyncReport, asyncErr = RunAsync(ctx, table.FM, table.Rows)
	}()
	wg.Wait()
	return syncReport, asyncReport, syncErr, asyncErr
}

// runOnce runs the scenario once on both engines and prints the per-engine
// summary report.
func runOnce(tablePath string, table *Table, timeout time.Duration) error {
	printPlatform()
	ctx, cancel := context.WithTimeout(context.Background(), timeout)
	defer cancel()
	syncReport, asyncReport, syncErr, asyncErr := runPair(ctx, table)

	fmt.Printf("Scenario: %s (%s), slippage %d bps\n\n",
		table.FM.Name, filepath.Base(tablePath), table.FM.SlippageBps)
	printLegend()
	if syncErr != nil {
		fmt.Printf("sequential engine error: %v\n", syncErr)
	}
	if asyncErr != nil {
		fmt.Printf("parallel engine error: %v\n", asyncErr)
	}
	if syncReport != nil {
		printReport(syncReport)
	}
	if asyncReport != nil {
		printReport(asyncReport)
	}
	return verdict(syncReport, asyncReport, syncErr, asyncErr)
}

// runRepeat re-runs the scenario on both engines until at least minDuration of
// wall-clock has elapsed, failing fast on the first mismatch. Every
// repeatLogInterval it prints a progress block with each engine's running
// order/report latency (min/avg/max); on completion it prints the platform and a
// per-engine aggregate summary.
func runRepeat(tablePath string, table *Table, timeout, minDuration time.Duration) error {
	fmt.Printf("Repeat: %s (%s), running for at least %s ...\n\n",
		table.FM.Name, filepath.Base(tablePath), minDuration)

	var syncAgg, asyncAgg engineAggregate
	start := time.Now()
	lastLog := start
	iterations := 0
	for {
		ctx, cancel := context.WithTimeout(context.Background(), timeout)
		syncReport, asyncReport, syncErr, asyncErr := runPair(ctx, table)
		cancel()
		iterations++

		if err := verdict(syncReport, asyncReport, syncErr, asyncErr); err != nil {
			if syncReport != nil {
				printReport(syncReport)
			}
			if asyncReport != nil {
				printReport(asyncReport)
			}
			return fmt.Errorf("repeat run failed on iteration %d after %s: %w",
				iterations, time.Since(start).Round(time.Millisecond), err)
		}
		syncAgg.add(syncReport)
		asyncAgg.add(asyncReport)

		now := time.Now()
		if now.Sub(lastLog) >= repeatLogInterval {
			printHeartbeat(now, iterations, now.Sub(start), minDuration, syncAgg, asyncAgg)
			lastLog = now
		}
		if now.Sub(start) >= minDuration {
			// Platform info heads the final report, not the stream of progress
			// blocks above it.
			printPlatform()
			printRepeatSummary(iterations, now.Sub(start), syncAgg, asyncAgg)
			return nil
		}
	}
}

// printHeartbeat prints one progress block during a repeat run: the clock time,
// iteration count, elapsed and remaining wall-clock, then each engine's running
// order/report latency as min/avg/max.
func printHeartbeat(
	now time.Time, iterations int, elapsed, minDuration time.Duration,
	syncAgg, asyncAgg engineAggregate,
) {
	left := minDuration - elapsed
	if left < 0 {
		left = 0
	}
	fmt.Printf("── %s · %d iter · elapsed %s · left %s ──\n",
		now.Format("15:04:05"), iterations,
		elapsed.Round(time.Second), left.Round(time.Second))
	printHeartbeatEngine("sync ", syncAgg)
	printHeartbeatEngine("async", asyncAgg)
}

// printHeartbeatEngine prints one engine's running order/report min/avg/max.
func printHeartbeatEngine(label string, a engineAggregate) {
	fmt.Printf("  %s · ord %s/%s/%s · rpt %s/%s/%s\n",
		label,
		a.order.Min, a.order.Avg(), a.order.Max,
		a.fill.Min, a.fill.Avg(), a.fill.Max)
}

// engineAggregate accumulates one engine's statistics across repeat iterations.
type engineAggregate struct {
	mode     Mode
	accounts int
	ops      int
	order    latencyStats
	fill     latencyStats
}

func (a *engineAggregate) add(r *Report) {
	if r == nil {
		return
	}
	a.mode = r.Mode
	a.accounts = r.AccountsCount()
	a.ops += r.Total
	a.order.merge(r.Order)
	a.fill.merge(r.Fill)
}

// verdict turns the two engines' outcomes into a single error (nil on success).
func verdict(syncReport, asyncReport *Report, syncErr, asyncErr error) error {
	if syncErr != nil || asyncErr != nil {
		return fmt.Errorf("one or more engines errored")
	}
	if (syncReport != nil && syncReport.FirstFail != nil) ||
		(asyncReport != nil && asyncReport.FirstFail != nil) {
		return fmt.Errorf("scenario failed")
	}
	return nil
}

// resolveTablePath looks for the requested file first as-is, then alongside the
// running binary, so a relative table path resolves whether the example is run
// via `go run .` or from an installed binary.
func resolveTablePath(p string) (string, error) {
	if _, err := os.Stat(p); err == nil {
		abs, err := filepath.Abs(p)
		if err != nil {
			return "", err
		}
		return abs, nil
	}
	exe, err := os.Executable()
	if err == nil {
		next := filepath.Join(filepath.Dir(exe), p)
		if _, statErr := os.Stat(next); statErr == nil {
			return next, nil
		}
	}
	return "", fmt.Errorf("table %q not found (cwd=%s)", p, currentDir())
}

// currentDir reports the working directory for diagnostics, falling back to "?"
// when it cannot be determined.
func currentDir() string {
	d, err := os.Getwd()
	if err != nil {
		return "?"
	}
	return d
}

// printLegend explains every field of the per-engine report once, so the output
// is readable without knowing the tool's internals.
func printLegend() {
	fmt.Println("Legend:")
	fmt.Println("  operations  - table rows applied to the engine (SEED/GROUP/ORDER/FILL; market-data ticks excluded)")
	fmt.Println("  accounts    - distinct accounts touched by the scenario")
	fmt.Println("  total time  - wall-clock to run the whole scenario on this engine")
	fmt.Println("  order check - time to decide one order (the pre-trade ACCEPT/REJECT check); n = orders checked")
	fmt.Println("  reports     - time to apply one fill / execution report; n = reports applied")
	fmt.Println("  parallel-engine times are the full submit-to-result round-trip: they include")
	fmt.Println("  async dispatch (the per-account worker handoff) and any queue wait, while the")
	fmt.Println("  sequential engine times only the direct call - so the two are not comparable.")
	fmt.Println()
}

// printReport renders one engine's outcome with the legend's field names.
func printReport(r *Report) {
	fmt.Printf("== %s ==\n", engineTitle(r.Mode))
	fmt.Printf("  operations  : %d\n", r.Total)
	fmt.Printf("  accounts    : %d\n", r.AccountsCount())
	fmt.Printf("  total time  : %s\n", r.WallClock)
	printLatency("  order check ", r.Order)
	printLatency("  reports     ", r.Fill)
	if r.FirstFail != nil {
		fmt.Printf("  result      : FAILED at line %d (%s, %s): %s\n\n",
			r.FirstFail.Row.Line, r.FirstFail.Row.Account,
			r.FirstFail.Row.Action, r.FirstFail.Message)
		return
	}
	fmt.Println("  result      : ALL PASS")
	fmt.Println()
}

func printLatency(label string, s latencyStats) {
	if s.Count == 0 {
		fmt.Printf("%s: none\n", label)
		return
	}
	fmt.Printf("%s: n=%d  min=%s  avg=%s  max=%s\n",
		label, s.Count, s.Min, s.Avg(), s.Max)
}

// printAggregate reports one engine's aggregate statistics over the repeat run.
func printAggregate(a engineAggregate, elapsed time.Duration) {
	fmt.Printf("== %s ==\n", engineTitle(a.mode))
	fmt.Printf("  operations  : %d total across the repeat run\n", a.ops)
	fmt.Printf("  accounts    : %d\n", a.accounts)
	fmt.Printf("  total time  : %s (whole repeat run)\n", elapsed.Round(time.Millisecond))
	printLatency("  order check ", a.order)
	printLatency("  reports     ", a.fill)
	fmt.Println()
}

// printRepeatSummary reports aggregate statistics over the whole repeat run.
func printRepeatSummary(iterations int, elapsed time.Duration, syncAgg, asyncAgg engineAggregate) {
	fmt.Printf("Repeat summary: %d iterations in %s, both engines agreed every time\n\n",
		iterations, elapsed.Round(time.Second))
	printLegend()
	printAggregate(syncAgg, elapsed)
	printAggregate(asyncAgg, elapsed)
}

// engineTitle gives each mode a self-describing header.
func engineTitle(m Mode) string {
	switch m {
	case ModeSync:
		return "sequential engine (sync)"
	case ModeAsync:
		return "parallel engine (async, one queue per account)"
	default:
		return string(m)
	}
}
