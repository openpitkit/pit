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

package main

import (
	"context"
	"fmt"
	"sort"
	"strings"
	"sync"
	"time"

	openpit "go.openpit.dev/openpit"
	"go.openpit.dev/openpit/asyncengine"
	"go.openpit.dev/openpit/marketdata"
	"go.openpit.dev/openpit/model"
	"go.openpit.dev/openpit/param"
	"go.openpit.dev/openpit/pretrade/policies"
	"go.openpit.dev/openpit/reject"
)

// Mode names the execution strategy of a runner.
type Mode string

const (
	ModeSync  Mode = "sync"
	ModeAsync Mode = "async"
)

// asyncStopTimeout bounds graceful shutdown and reservation draining of the
// async engine.
const asyncStopTimeout = 30 * time.Second

// Report is the per-mode outcome of running a table.
type Report struct {
	Mode      Mode
	Total     int            // executable rows (SEED/GROUP/ORDER/FILL; excludes TICK)
	Accounts  map[string]int // account name -> row count
	WallClock time.Duration
	Order     latencyStats
	Fill      latencyStats
	FirstFail *Failure
}

// Failure describes the first mismatch or runtime error seen.
type Failure struct {
	Row     Row
	Message string
}

// AccountsCount returns the number of distinct accounts touched.
func (r *Report) AccountsCount() int { return len(r.Accounts) }

// earlierFailure returns whichever failure sits on the earlier table row, so a
// mismatch from the verdict loop wins over a later setup-phase failure (and vice
// versa). A nil argument is treated as "no failure".
func earlierFailure(a, b *Failure) *Failure {
	switch {
	case a == nil:
		return b
	case b == nil:
		return a
	case b.Row.Line < a.Row.Line:
		return b
	default:
		return a
	}
}

type latencyStats struct {
	Count int
	Total time.Duration
	Min   time.Duration
	Max   time.Duration
}

func (s *latencyStats) observe(d time.Duration) {
	s.Count++
	s.Total += d
	if s.Count == 1 || d < s.Min {
		s.Min = d
	}
	if d > s.Max {
		s.Max = d
	}
}

// Avg returns the mean latency.
func (s latencyStats) Avg() time.Duration {
	if s.Count == 0 {
		return 0
	}
	return s.Total / time.Duration(s.Count)
}

// merge folds another sample into s, for aggregating across repeat iterations.
func (s *latencyStats) merge(o latencyStats) {
	if o.Count == 0 {
		return
	}
	if s.Count == 0 || o.Min < s.Min {
		s.Min = o.Min
	}
	if o.Max > s.Max {
		s.Max = o.Max
	}
	s.Count += o.Count
	s.Total += o.Total
}

// codeNames is the case-insensitive map from the table's `reject`
// column to the reject Code recognized by the engine.
var codeNames = map[string]reject.Code{
	"missingrequiredfield":        reject.CodeMissingRequiredField,
	"invalidfieldformat":          reject.CodeInvalidFieldFormat,
	"invalidfieldvalue":           reject.CodeInvalidFieldValue,
	"unsupportedordertype":        reject.CodeUnsupportedOrderType,
	"insufficientfunds":           reject.CodeInsufficientFunds,
	"insufficientmargin":          reject.CodeInsufficientMargin,
	"insufficientposition":        reject.CodeInsufficientPosition,
	"markpriceunavailable":        reject.CodeMarkPriceUnavailable,
	"ordervaluecalculationfailed": reject.CodeOrderValueCalculationFailed,
	"accountadjustmentboundsexceeded": reject.
		CodeAccountAdjustmentBoundsExceeded,
}

func resolveCode(name string) (reject.Code, bool) {
	c, ok := codeNames[strings.ToLower(strings.TrimSpace(name))]
	return c, ok
}

// codeNamesByCode is the reverse of codeNames, built once on first use
// so codeName is a map lookup rather than a linear scan.
var codeNamesByCode = sync.OnceValue(func() map[reject.Code]string {
	m := make(map[reject.Code]string, len(codeNames))
	for name, code := range codeNames {
		m[code] = name
	}
	return m
})

func codeName(c reject.Code) string {
	if name, ok := codeNamesByCode()[c]; ok {
		return name
	}
	return fmt.Sprintf("Code(%d)", c)
}

// buildSpotEngineSync builds the Mode A engine: single-thread NoSync with the
// spot funds policy reading a Local market-data service. The returned feed owns
// the instrument registry; its instruments are registered up front so live TICK
// pushes resolve. The caller must Close the feed's service after Stop.
func buildSpotEngineSync(
	fm Frontmatter, rows []Row,
) (*openpit.Engine, *MarketFeed, error) {
	eb := openpit.NewEngineBuilder().NoSync()
	service, err := eb.MarketData(marketdata.InfiniteTTL()).NoSync().Build()
	if err != nil {
		return nil, nil, fmt.Errorf("build market data: %w", err)
	}
	feed := NewMarketFeed(service)
	if err := feed.RegisterInstruments(rows); err != nil {
		service.Close()
		return nil, nil, err
	}
	engine, err := eb.
		Builtin(
			policies.BuildSpotFunds().
				WithMarketOrders(service, fm.SlippageBps).
				PricingSource(policies.SpotFundsPricingSourceMark),
		).
		Build()
	if err != nil {
		service.Close()
		return nil, nil, err
	}
	return engine, feed, nil
}

// buildSpotEngineAsync builds the Mode B engine: AccountSync wrapped into an
// asyncengine.AsyncEngine with the Dynamic strategy so each account gets its own
// serial queue. The spot funds policy reads a Full-sync market-data service that
// is safe for the concurrent live feed. The caller must Close the feed's service
// after the async engine has stopped.
func buildSpotEngineAsync(
	fm Frontmatter, rows []Row,
) (*asyncengine.AsyncEngine, *MarketFeed, error) {
	eb := openpit.NewEngineBuilder().AccountSync()
	service, err := eb.MarketData(marketdata.InfiniteTTL()).FullSync().Build()
	if err != nil {
		return nil, nil, fmt.Errorf("build market data: %w", err)
	}
	feed := NewMarketFeed(service)
	if err := feed.RegisterInstruments(rows); err != nil {
		service.Close()
		return nil, nil, err
	}
	builder, err := eb.
		Builtin(
			policies.BuildSpotFunds().
				WithMarketOrders(service, fm.SlippageBps).
				PricingSource(policies.SpotFundsPricingSourceMark),
		).
		BuildAsync()
	if err != nil {
		service.Close()
		return nil, nil, err
	}
	engine, err := builder.Dynamic().Build()
	if err != nil {
		service.Close()
		return nil, nil, err
	}
	return engine, feed, nil
}

// groupMembership aggregates every GROUP row into the set of accounts to
// register per group, preserving row order so registration is deterministic.
// Each GROUP row is retained both for diagnostics and so the report's account
// counts match between the two runners.
type groupMembership struct {
	order   []string                     // group labels in first-seen order
	members map[string][]param.AccountID // group label -> member account IDs
	rows    []Row                        // every GROUP row, in table order
}

func collectGroups(tableRows []Row) (*groupMembership, *Failure) {
	g := &groupMembership{members: map[string][]param.AccountID{}}
	for _, row := range tableRows {
		if row.Action != "GROUP" {
			continue
		}
		acc, err := accountID(row.Account)
		if err != nil {
			return nil, &Failure{Row: row, Message: err.Error()}
		}
		if _, ok := g.members[row.Group]; !ok {
			g.order = append(g.order, row.Group)
		}
		g.members[row.Group] = append(g.members[row.Group], acc)
		g.rows = append(g.rows, row)
	}
	return g, nil
}

// firstRow returns the first GROUP row that named label, used to anchor a
// registration failure to a concrete table line.
func (g *groupMembership) firstRow(label string) Row {
	for _, row := range g.rows {
		if row.Group == label {
			return row
		}
	}
	return Row{}
}

// countInReport records every GROUP row toward the report's totals so both
// runners agree on executable-row and per-account counts.
func (g *groupMembership) countInReport(report *Report) {
	for _, row := range g.rows {
		report.Total++
		report.Accounts[row.Account]++
	}
}

// RunSync executes the table in Mode A: NoSync engine, strictly
// operation-by-operation. TICK rows are replayed live at their row position.
// Stops at the first verdict mismatch and returns a partial report.
func RunSync(
	ctx context.Context, fm Frontmatter, rows []Row,
) (*Report, error) {
	engine, feed, err := buildSpotEngineSync(fm, rows)
	if err != nil {
		return nil, fmt.Errorf("build sync engine: %w", err)
	}
	defer engine.Stop()
	defer feed.service.Close()

	report := &Report{Mode: ModeSync, Accounts: map[string]int{}}

	groups, fail := collectGroups(rows)
	if fail != nil {
		report.FirstFail = fail
		return report, nil
	}
	if fail := registerGroupsSync(engine, groups, report); fail != nil {
		report.FirstFail = fail
		return report, nil
	}

	start := time.Now()
	for _, row := range rows {
		if ctx.Err() != nil {
			break
		}
		switch row.Action {
		case "GROUP":
			// Registered up front in registerGroupsSync.
			continue
		case "TICK":
			if fail := runSyncTick(feed, row); fail != nil {
				report.FirstFail = fail
				goto done
			}
			continue
		}
		acc, err := accountID(row.Account)
		if err != nil {
			report.FirstFail = &Failure{Row: row, Message: err.Error()}
			break
		}
		report.Total++
		report.Accounts[row.Account]++

		switch row.Action {
		case "SEED":
			if fail := runSyncSeed(engine, acc, row); fail != nil {
				report.FirstFail = fail
				goto done
			}
		case "ORDER":
			fail, d := runSyncOrder(engine, acc, row)
			report.Order.observe(d)
			if fail != nil {
				report.FirstFail = fail
				goto done
			}
		case "FILL":
			fail, d := runSyncFill(engine, acc, row, feed)
			report.Fill.observe(d)
			if fail != nil {
				report.FirstFail = fail
				goto done
			}
		}
	}
done:
	report.WallClock = time.Since(start)
	return report, nil
}

// registerGroupsSync registers every aggregated GROUP membership on the sync
// engine before any dependent row runs, counting each GROUP row toward the
// report. A registration failure is reported against the group's first row.
func registerGroupsSync(
	engine *openpit.Engine, groups *groupMembership, report *Report,
) *Failure {
	groups.countInReport(report)
	accountsView := engine.Accounts()
	for _, label := range groups.order {
		groupID, err := accountGroupID(label)
		if err != nil {
			return &Failure{Row: groups.firstRow(label), Message: err.Error()}
		}
		if err := accountsView.RegisterGroup(groups.members[label], groupID); err != nil {
			return &Failure{
				Row:     groups.firstRow(label),
				Message: "register group: " + err.Error(),
			}
		}
	}
	return nil
}

func runSyncTick(feed *MarketFeed, row Row) *Failure {
	if err := pushTick(feed, row); err != nil {
		return &Failure{Row: row, Message: err.Error()}
	}
	return nil
}

// pushTick replays one TICK row: a global push when neither account nor group
// is set, otherwise an addressed push to the named account and/or group.
func pushTick(feed *MarketFeed, row Row) error {
	if row.Account == "" && row.Group == "" {
		return feed.Push(row.Instrument, row.Price)
	}
	var accounts []param.AccountID
	if row.Account != "" {
		acc, err := accountID(row.Account)
		if err != nil {
			return err
		}
		accounts = append(accounts, acc)
	}
	var groups []param.AccountGroupID
	if row.Group != "" {
		groupID, err := accountGroupID(row.Group)
		if err != nil {
			return err
		}
		groups = append(groups, groupID)
	}
	return feed.PushFor(row.Instrument, row.Price, accounts, groups)
}

func runSyncSeed(
	engine *openpit.Engine, acc param.AccountID, row Row,
) *Failure {
	adj, err := buildSeedAdjustment(row)
	if err != nil {
		return &Failure{Row: row, Message: err.Error()}
	}
	r, _, err := engine.ApplyAccountAdjustment(
		acc, []model.AccountAdjustment{adj},
	)
	if err != nil {
		return &Failure{Row: row, Message: "engine: " + err.Error()}
	}
	return checkSeedVerdict(row, r.IsSet())
}

// checkSeedVerdict compares a SEED outcome against the row's expected
// verdict. rejected reports whether the engine refused the adjustment.
func checkSeedVerdict(row Row, rejected bool) *Failure {
	switch row.Expect {
	case "OK":
		if rejected {
			return &Failure{Row: row, Message: "expected OK, SEED rejected"}
		}
	case "REJECT":
		if !rejected {
			return &Failure{Row: row, Message: "expected REJECT, SEED accepted"}
		}
	default:
		return seedFillVerdictError(row)
	}
	return nil
}

// checkFillVerdict compares a FILL outcome against the row's expected
// verdict. blocked reports whether the report produced an account block.
func checkFillVerdict(row Row, blocked bool) *Failure {
	switch row.Expect {
	case "OK":
		if blocked {
			return &Failure{Row: row, Message: "expected OK, got account block"}
		}
	case "REJECT":
		if !blocked {
			return &Failure{
				Row: row, Message: "expected REJECT, FILL produced no block",
			}
		}
	default:
		return seedFillVerdictError(row)
	}
	return nil
}

// seedFillVerdictError reports an expectation that SEED/FILL cannot
// honor. ACCEPT in particular is an ORDER-only verdict.
func seedFillVerdictError(row Row) *Failure {
	if row.Expect == "ACCEPT" {
		return &Failure{
			Row: row,
			Message: fmt.Sprintf(
				"%s row cannot use ACCEPT (ORDER-only); use OK or REJECT",
				row.Action,
			),
		}
	}
	return &Failure{
		Row: row,
		Message: fmt.Sprintf(
			"%s row must use OK/REJECT, got %s", row.Action, row.Expect,
		),
	}
}

func runSyncOrder(
	engine *openpit.Engine, acc param.AccountID, row Row,
) (*Failure, time.Duration) {
	order, err := buildOrder(row, acc)
	if err != nil {
		return &Failure{Row: row, Message: err.Error()}, 0
	}
	start := time.Now()
	reservation, rejects, err := engine.ExecutePreTrade(order)
	dur := time.Since(start)
	if err != nil {
		return &Failure{Row: row, Message: "engine: " + err.Error()}, dur
	}
	fail := checkOrderVerdict(row, rejects)
	if fail == nil && reservation != nil {
		reservation.CommitAndClose()
	} else if reservation != nil {
		reservation.RollbackAndClose()
	}
	return fail, dur
}

func runSyncFill(
	engine *openpit.Engine, acc param.AccountID, row Row, feed *MarketFeed,
) (*Failure, time.Duration) {
	report, err := buildFillReport(row, acc, feed)
	if err != nil {
		return &Failure{Row: row, Message: err.Error()}, 0
	}
	start := time.Now()
	result, err := engine.ApplyExecutionReport(report)
	dur := time.Since(start)
	if err != nil {
		return &Failure{Row: row, Message: "engine: " + err.Error()}, dur
	}
	return checkFillVerdict(row, len(result.AccountBlocks) > 0), dur
}

// checkOrderVerdict compares the engine's rejects against the row's
// expectation. Returns nil on a match.
func checkOrderVerdict(row Row, rejects []reject.Reject) *Failure {
	switch row.Expect {
	case "ACCEPT":
		if rejects != nil {
			return &Failure{
				Row: row,
				Message: fmt.Sprintf(
					"expected ACCEPT, got REJECT(%s)",
					describeRejects(rejects),
				),
			}
		}
	case "REJECT":
		if rejects == nil {
			return &Failure{
				Row:     row,
				Message: "expected REJECT, got ACCEPT",
			}
		}
		if row.Reject != "" {
			wantCode, ok := resolveCode(row.Reject)
			if !ok {
				return &Failure{
					Row: row,
					Message: fmt.Sprintf(
						"unknown reject code %q in table",
						row.Reject,
					),
				}
			}
			if !containsCode(rejects, wantCode) {
				return &Failure{
					Row: row,
					Message: fmt.Sprintf(
						"expected REJECT(%s), got REJECT(%s)",
						row.Reject, describeRejects(rejects),
					),
				}
			}
		}
	default:
		return &Failure{
			Row: row,
			Message: fmt.Sprintf(
				"ORDER row must use ACCEPT/REJECT, got %s",
				row.Expect,
			),
		}
	}
	return nil
}

func containsCode(rejects []reject.Reject, want reject.Code) bool {
	for _, r := range rejects {
		if r.Code == want {
			return true
		}
	}
	return false
}

func describeRejects(rejects []reject.Reject) string {
	if len(rejects) == 0 {
		return ""
	}
	names := make([]string, 0, len(rejects))
	for _, r := range rejects {
		names = append(names, codeName(r.Code))
	}
	sort.Strings(names)
	return strings.Join(names, ",")
}

// RunAsync executes the table in Mode B: AccountSync engine wrapped in an
// asyncengine. GROUP rows are registered (and awaited) first. Non-TICK rows are
// then submitted in row order; per-account dispatchers run them serially while
// different accounts progress in parallel. An addressed TICK is replayed only
// after the outstanding operations of its target account(s) have executed, so
// the new quote is visible to those accounts' later rows but never rewrites
// their earlier ones. Verdict checks then await futures in row order and stop on
// the first mismatch.
func RunAsync(
	ctx context.Context, fm Frontmatter, rows []Row,
) (*Report, error) {
	engine, feed, err := buildSpotEngineAsync(fm, rows)
	if err != nil {
		return nil, fmt.Errorf("build async engine: %w", err)
	}
	defer func() {
		stopCtx, cancel := context.WithTimeout(
			context.Background(), asyncStopTimeout,
		)
		defer cancel()
		_ = engine.StopGraceful(stopCtx)
		feed.service.Close()
	}()

	report := &Report{Mode: ModeAsync, Accounts: map[string]int{}}

	groups, fail := collectGroups(rows)
	if fail != nil {
		report.FirstFail = fail
		return report, nil
	}
	if fail := registerGroupsAsync(ctx, engine, groups, report); fail != nil {
		report.FirstFail = fail
		return report, nil
	}

	s, err := submitAsyncSteps(ctx, engine, feed, groups, rows, report)
	if err != nil {
		return nil, err
	}
	pending := s.steps
	// A TICK replay failure during submission is recorded in report.FirstFail;
	// the verdict loop below still finalizes the steps that were submitted
	// before it. Keep whichever failure sits on the earlier table row.
	submitFail := report.FirstFail
	start := time.Now()
	awaited := 0
	for i, p := range pending {
		if ctx.Err() != nil {
			break
		}
		awaited = i + 1
		fail := p.await(ctx)
		if fail != nil {
			report.FirstFail = fail
			break
		}
	}
	report.WallClock = time.Since(start)
	report.FirstFail = earlierFailure(report.FirstFail, submitFail)
	// Every step was submitted up front, so steps the verdict loop never
	// reached (after an early stop, or after a cancelled context) may have
	// already resolved a reservation on a worker. Drain them so each
	// reservation is finalized exactly once and no native handle leaks.
	drainCtx, cancel := context.WithTimeout(
		context.Background(), asyncStopTimeout,
	)
	defer cancel()
	for _, p := range pending[awaited:] {
		if p.release != nil {
			p.release(drainCtx)
		}
	}
	// Every operation has now resolved (awaited above or drained here); wait for
	// the per-operation latency timers to finish recording before the caller
	// reads the report.
	s.timingWG.Wait()
	return report, nil
}

// registerGroupsAsync registers every aggregated GROUP membership on the async
// engine and awaits the registrations before any dependent row is submitted.
func registerGroupsAsync(
	ctx context.Context,
	engine *asyncengine.AsyncEngine,
	groups *groupMembership,
	report *Report,
) *Failure {
	groups.countInReport(report)
	accountsView := engine.Accounts()
	for _, label := range groups.order {
		groupID, err := accountGroupID(label)
		if err != nil {
			return &Failure{Row: groups.firstRow(label), Message: err.Error()}
		}
		if _, err := accountsView.RegisterGroup(
			ctx, groups.members[label], groupID,
		).Await(ctx); err != nil {
			return &Failure{
				Row:     groups.firstRow(label),
				Message: "register group: " + err.Error(),
			}
		}
	}
	return nil
}

// asyncStep is a row submitted to the async engine, paired with the
// future-await logic specific to its action kind. wait blocks until the step's
// engine call has executed without finalizing it or scoring its verdict; a TICK
// barrier uses it to fence target accounts. release, when set, awaits the step's
// future and finalizes any reservation the verdict loop did not reach; it is
// safe to call instead of or after await.
type asyncStep struct {
	row     Row
	await   func(context.Context) *Failure
	wait    func(context.Context)
	release func(context.Context)
}

// asyncSubmission threads the per-account barrier bookkeeping through the submit
// loop so an addressed TICK can fence exactly the accounts it targets. It also
// owns the per-operation latency timers: each records submit-to-resolve the
// moment its future resolves (independent of the verdict loop), so a stat is the
// operation's true round-trip and not an artifact of await ordering.
type asyncSubmission struct {
	feed     *MarketFeed
	groups   *groupMembership
	report   *Report
	steps    []asyncStep
	waiters  map[string][]func(context.Context)
	statsMu  sync.Mutex
	timingWG sync.WaitGroup
}

// observeOnResolve records one operation's submit-to-resolve latency as soon as
// its future resolves, on its own goroutine. RunAsync waits for all such
// goroutines (timingWG) before reading the report; statsMu guards the shared
// latencyStats against the concurrent timers.
func (s *asyncSubmission) observeOnResolve(
	ctx context.Context, done <-chan struct{}, start time.Time, stat *latencyStats,
) {
	s.timingWG.Add(1)
	go func() {
		defer s.timingWG.Done()
		select {
		case <-done:
			s.statsMu.Lock()
			stat.observe(time.Since(start))
			s.statsMu.Unlock()
		case <-ctx.Done():
		}
	}()
}

func submitAsyncSteps(
	ctx context.Context,
	engine *asyncengine.AsyncEngine,
	feed *MarketFeed,
	groups *groupMembership,
	rows []Row,
	report *Report,
) (*asyncSubmission, error) {
	s := &asyncSubmission{
		feed:    feed,
		groups:  groups,
		report:  report,
		waiters: map[string][]func(context.Context){},
	}
	for _, row := range rows {
		if ctx.Err() != nil {
			break
		}
		switch row.Action {
		case "GROUP":
			// Registered up front in registerGroupsAsync.
			continue
		case "TICK":
			if err := s.replayTick(ctx, row); err != nil {
				report.FirstFail = &Failure{Row: row, Message: err.Error()}
				return s, nil //nolint:nilerr // row/ctx failures are surfaced via the report, not the error return
			}
			continue
		}
		acc, err := accountID(row.Account)
		if err != nil {
			report.FirstFail = &Failure{Row: row, Message: err.Error()}
			break
		}
		report.Total++
		report.Accounts[row.Account]++

		var step asyncStep
		switch row.Action {
		case "SEED":
			step, err = submitAsyncSeed(ctx, engine, acc, row)
		case "ORDER":
			step, err = s.submitOrder(ctx, engine, acc, row)
		case "FILL":
			step, err = s.submitFill(ctx, engine, acc, row)
		}
		if err != nil {
			return nil, err
		}
		s.steps = append(s.steps, step)
		s.waiters[acc.String()] = append(s.waiters[acc.String()], step.wait)
	}
	return s, nil //nolint:nilerr // row/ctx failures are surfaced via the report, not the error return
}

// replayTick fences the TICK's target accounts (so their already-submitted
// operations have executed), then publishes the quote. A global push has no
// fence: the determinism contract restricts it to the pre-order setup block.
func (s *asyncSubmission) replayTick(ctx context.Context, row Row) error {
	if row.Account == "" && row.Group == "" {
		return s.feed.Push(row.Instrument, row.Price)
	}
	for _, acc := range s.barrierAccounts(row) {
		for _, wait := range s.waiters[acc.String()] {
			wait(ctx)
		}
	}
	return pushTick(s.feed, row)
}

// barrierAccounts returns the accounts whose outstanding operations a TICK must
// fence: the explicitly addressed account plus every member of the addressed
// group.
func (s *asyncSubmission) barrierAccounts(row Row) []param.AccountID {
	var accounts []param.AccountID
	if row.Account != "" {
		if acc, err := accountID(row.Account); err == nil {
			accounts = append(accounts, acc)
		}
	}
	if row.Group != "" {
		accounts = append(accounts, s.groups.members[row.Group]...)
	}
	return accounts
}

func submitAsyncSeed(
	ctx context.Context, engine *asyncengine.AsyncEngine,
	acc param.AccountID, row Row,
) (asyncStep, error) {
	adj, err := buildSeedAdjustment(row)
	if err != nil {
		return asyncStep{}, err
	}
	fut := engine.ApplyAccountAdjustment(
		ctx, acc, []model.AccountAdjustment{adj},
	)
	return asyncStep{
		row: row,
		await: func(ctx context.Context) *Failure {
			result, _, err := fut.Await(ctx)
			if err != nil {
				return &Failure{
					Row: row, Message: "engine: " + err.Error(),
				}
			}
			return checkSeedVerdict(row, result.IsSet())
		},
		wait: func(ctx context.Context) { _, _, _ = fut.Await(ctx) },
	}, nil
}

// submitOrder submits a pre-trade order and starts a latency timer that records
// the operation's true submit-to-resolve round-trip the moment its future
// resolves. The first order on an empty per-account queue therefore measures
// only async dispatch (worker handoff) plus the engine call, with no queue wait;
// the verdict is scored later by the caller's await loop.
func (s *asyncSubmission) submitOrder(
	ctx context.Context, engine *asyncengine.AsyncEngine,
	acc param.AccountID, row Row,
) (asyncStep, error) {
	order, err := buildOrder(row, acc)
	if err != nil {
		return asyncStep{}, err
	}
	start := time.Now()
	fut := engine.ExecutePreTrade(ctx, order)
	s.observeOnResolve(ctx, fut.Wait(), start, &s.report.Order)
	var once sync.Once
	step := asyncStep{row: row}
	step.await = func(ctx context.Context) *Failure {
		var fail *Failure
		once.Do(func() {
			result, rejects, err := fut.Await(ctx)
			if err != nil {
				fail = &Failure{Row: row, Message: "engine: " + err.Error()}
				return
			}
			fail = checkOrderVerdict(row, rejects)
			finalizeReservation(ctx, result, fail == nil)
		})
		return fail
	}
	step.wait = func(ctx context.Context) { _, _, _ = fut.Await(ctx) }
	// release covers steps the verdict loop never awaited: roll back any
	// reservation the worker already resolved so it is closed exactly once.
	step.release = func(ctx context.Context) {
		once.Do(func() {
			result, _, err := fut.Await(ctx)
			if err != nil {
				return
			}
			finalizeReservation(ctx, result, false)
		})
	}
	return step, nil
}

// finalizeReservation commits a passing reservation and rolls back any
// other, then closes it. A nil reservation is a no-op. Each reservation
// must pass through exactly once.
func finalizeReservation(
	ctx context.Context, res *asyncengine.AsyncReservation, commit bool,
) {
	if res == nil {
		return
	}
	if commit {
		_, _ = res.CommitAndClose(ctx).Await(ctx)
		return
	}
	_, _ = res.RollbackAndClose(ctx).Await(ctx)
}

// submitFill submits a final execution report, timing its submit-to-resolve
// round-trip the same way submitOrder does.
func (s *asyncSubmission) submitFill(
	ctx context.Context, engine *asyncengine.AsyncEngine,
	acc param.AccountID, row Row,
) (asyncStep, error) {
	r, err := buildFillReport(row, acc, s.feed)
	if err != nil {
		return asyncStep{}, err
	}
	start := time.Now()
	fut := engine.ApplyExecutionReport(ctx, r)
	s.observeOnResolve(ctx, fut.Wait(), start, &s.report.Fill)
	return asyncStep{
		row: row,
		await: func(ctx context.Context) *Failure {
			result, err := fut.Await(ctx)
			if err != nil {
				return &Failure{
					Row: row, Message: "engine: " + err.Error(),
				}
			}
			return checkFillVerdict(row, len(result.AccountBlocks) > 0)
		},
		wait: func(ctx context.Context) { _, _ = fut.Await(ctx) },
	}, nil
}
