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

package asyncengine

import (
	"context"
	"errors"
	"runtime"
	"slices"
	"sync/atomic"
	"testing"

	"go.openpit.dev/openpit/accountadjustment"
	"go.openpit.dev/openpit/internal/native"
	"go.openpit.dev/openpit/model"
	"go.openpit.dev/openpit/param"
	"go.openpit.dev/openpit/pretrade"
	"go.openpit.dev/openpit/reject"
)

// A fuzz input describes one chain: a begin mode, a terminal-hook mode, a step
// count, and two bytes per step - its kind and how that step behaves. Every
// field is range checked, so an input that describes no chain this harness can
// build is skipped rather than guessed at.
const (
	chainFuzzBeginOK byte = iota
	chainFuzzBeginError
	chainFuzzBeginPanic
	chainFuzzBeginModes
)

// The terminal hook is the only place where a chain that did everything right
// still ends with an error, so it is the only way to generate an outcome whose
// status and error could disagree.
const (
	chainFuzzFinallyOK byte = iota
	chainFuzzFinallyError
	chainFuzzFinallyPanic
	chainFuzzFinallyModes
)

const (
	chainFuzzStepThen byte = iota
	chainFuzzStepPreTrade
	chainFuzzStepDropCopy
	chainFuzzStepCheckOrder
	chainFuzzStepExecutionReport
	chainFuzzStepAccountAdjustment
	chainFuzzStepKinds
)

const (
	chainFuzzThenOK byte = iota
	chainFuzzThenError
	chainFuzzThenPanic
	chainFuzzThenModes
)

// ExecutePreTrade and ApplyDropCopy share their modes: the first five drive
// the accepted hook of an operation the driver handed out, the last four
// decide what the driver does instead of handing one out.
const (
	chainFuzzOpCommit byte = iota
	chainFuzzOpRollback
	chainFuzzOpHookError
	chainFuzzOpHookPanic
	chainFuzzOpInvalidDecision
	chainFuzzOpReject
	chainFuzzOpRejectHookError
	chainFuzzOpDriverError
	chainFuzzOpDriverPanic
	chainFuzzOpModes
)

const (
	chainFuzzCheckOK byte = iota
	chainFuzzCheckHookError
	chainFuzzCheckDriverError
	chainFuzzCheckModes
)

const (
	chainFuzzReportOK byte = iota
	chainFuzzReportInputError
	chainFuzzReportInputPanic
	chainFuzzReportHookError
	chainFuzzReportDriverError
	chainFuzzReportModes
)

const (
	chainFuzzAdjustEmpty byte = iota
	chainFuzzAdjustEmptyHookError
	chainFuzzAdjustBatch
	chainFuzzAdjustBatchHookError
	chainFuzzAdjustInputError
	chainFuzzAdjustInputPanic
	chainFuzzAdjustDriverError
	chainFuzzAdjustModes
)

const (
	// chainFuzzMaxSteps bounds a generated chain, so one input cannot turn the
	// seed corpus run into a long one.
	chainFuzzMaxSteps = 4
	// chainFuzzHeaderSize is the begin mode, the terminal-hook mode, and the
	// step count.
	chainFuzzHeaderSize = 3
	// chainFuzzStepSize is the kind byte plus the behaviour byte.
	chainFuzzStepSize = 2
	// chainFuzzInvalidDecision is neither supported Decision value.
	chainFuzzInvalidDecision = Decision(255)
)

var (
	errChainFuzzBegin      = errors.New("fuzz chain begin failed")
	errChainFuzzFinally    = errors.New("fuzz chain final hook failed")
	errChainFuzzStep       = errors.New("fuzz chain step failed")
	errChainFuzzHook       = errors.New("fuzz chain hook failed")
	errChainFuzzRejectHook = errors.New("fuzz chain reject hook failed")
	errChainFuzzDriver     = errors.New("fuzz chain driver failed")
	errChainFuzzHarness    = errors.New("fuzz chain harness took a wrong path")
)

// chainFuzzStep is one decoded step: what the chain does there and how that
// step behaves.
type chainFuzzStep struct {
	kind byte
	mode byte
}

// chainFuzzShape is one decoded chain.
type chainFuzzShape struct {
	steps   []chainFuzzStep
	begin   byte
	finally byte
}

// chainFuzzModes reports how many behaviours a step kind has.
func chainFuzzModes(kind byte) byte {
	switch kind {
	case chainFuzzStepThen:
		return chainFuzzThenModes
	case chainFuzzStepPreTrade, chainFuzzStepDropCopy:
		return chainFuzzOpModes
	case chainFuzzStepCheckOrder:
		return chainFuzzCheckModes
	case chainFuzzStepExecutionReport:
		return chainFuzzReportModes
	case chainFuzzStepAccountAdjustment:
		return chainFuzzAdjustModes
	default:
		return 0
	}
}

// decodeChainFuzzShape reads a chain shape out of a fuzz input. It reports
// false for anything it cannot build, so a malformed input is skipped instead
// of failing the run or panicking inside the harness.
func decodeChainFuzzShape(data []byte) (chainFuzzShape, bool) {
	if len(data) < chainFuzzHeaderSize {
		return chainFuzzShape{}, false
	}
	shape := chainFuzzShape{begin: data[0], finally: data[1]}
	if shape.begin >= chainFuzzBeginModes ||
		shape.finally >= chainFuzzFinallyModes {
		return chainFuzzShape{}, false
	}
	count := int(data[2])
	if count > chainFuzzMaxSteps {
		return chainFuzzShape{}, false
	}
	if len(data) < chainFuzzHeaderSize+chainFuzzStepSize*count {
		return chainFuzzShape{}, false
	}
	shape.steps = make([]chainFuzzStep, 0, count)
	for i := range count {
		offset := chainFuzzHeaderSize + chainFuzzStepSize*i
		step := chainFuzzStep{kind: data[offset], mode: data[offset+1]}
		if step.kind >= chainFuzzStepKinds || step.mode >= chainFuzzModes(step.kind) {
			return chainFuzzShape{}, false
		}
		shape.steps = append(shape.steps, step)
	}
	return shape, true
}

// encodeChainFuzzInput builds a seed input from a readable shape. The count
// byte is raised step by step, so the header never has to narrow a length the
// decoder would refuse anyway.
func encodeChainFuzzInput(
	begin byte,
	finally byte,
	steps ...chainFuzzStep,
) []byte {
	data := make(
		[]byte,
		chainFuzzHeaderSize,
		chainFuzzHeaderSize+chainFuzzStepSize*len(steps),
	)
	data[0] = begin
	data[1] = finally
	for _, step := range steps {
		data[2]++
		data = append(data, step.kind, step.mode)
	}
	return data
}

type chainFuzzPhase string

const (
	chainFuzzPhaseBegin       chainFuzzPhase = "begin"
	chainFuzzPhaseThen        chainFuzzPhase = "then"
	chainFuzzPhaseDriver      chainFuzzPhase = "driver"
	chainFuzzPhaseAccepted    chainFuzzPhase = "accepted"
	chainFuzzPhaseRejected    chainFuzzPhase = "rejected"
	chainFuzzPhaseChecked     chainFuzzPhase = "checked"
	chainFuzzPhaseReport      chainFuzzPhase = "report"
	chainFuzzPhaseSettled     chainFuzzPhase = "settled"
	chainFuzzPhaseAdjustments chainFuzzPhase = "adjustments"
	chainFuzzPhaseAdjusted    chainFuzzPhase = "adjusted"
	chainFuzzPhaseFinally     chainFuzzPhase = "finally"
	chainFuzzNoStep                          = -1
)

// chainFuzzEvent identifies one externally observable part of a generated
// chain. The step index distinguishes repeated operations of the same kind.
type chainFuzzEvent struct {
	phase chainFuzzPhase
	index int
}

// chainFuzzExpectation is an independent model of the public chain contract.
// It describes what the generated shape requires, never what the run happened
// to observe, so skipped hooks and wrong finalization cannot make their own
// expectations smaller.
type chainFuzzExpectation struct {
	events              []chainFuzzEvent
	entered             []int
	errors              []error
	beforeFinallyErrors []error
	statefulCalls       int
	finallyCalls        int
	commits             int64
	rollbacks           int64
	status              ChainOutcomeStatus
	beforeFinallyStatus ChainOutcomeStatus
	retryUnsafe         bool
}

func chainFuzzExpected(
	t *testing.T,
	shape chainFuzzShape,
) chainFuzzExpectation {
	t.Helper()
	expected := chainFuzzExpectation{status: ChainOutcomeCompleted}
	expected.record(chainFuzzNoStep, chainFuzzPhaseBegin)
	switch shape.begin {
	case chainFuzzBeginOK:
	case chainFuzzBeginError, chainFuzzBeginPanic:
		expected.fail(errChainFuzzBegin)
		return expected
	default:
		t.Errorf("expected result hit an unknown begin mode %d", shape.begin)
		expected.fail(errChainFuzzHarness)
		return expected
	}

	for index, step := range shape.steps {
		expected.entered = append(expected.entered, index)
		if !expected.applyStep(t, index, step) {
			break
		}
	}

	expected.finallyCalls = 1
	expected.beforeFinallyStatus = expected.status
	expected.beforeFinallyErrors = append(
		expected.beforeFinallyErrors, expected.errors...,
	)
	expected.record(chainFuzzNoStep, chainFuzzPhaseFinally)
	switch shape.finally {
	case chainFuzzFinallyOK:
	case chainFuzzFinallyError, chainFuzzFinallyPanic:
		expected.fail(errChainFuzzFinally)
	default:
		t.Errorf("expected result hit an unknown final mode %d", shape.finally)
		expected.fail(errChainFuzzHarness)
	}
	return expected
}

func (e *chainFuzzExpectation) applyStep(
	t *testing.T,
	index int,
	step chainFuzzStep,
) bool {
	t.Helper()
	switch step.kind {
	case chainFuzzStepThen:
		e.record(index, chainFuzzPhaseThen)
		switch step.mode {
		case chainFuzzThenOK:
			return true
		case chainFuzzThenError, chainFuzzThenPanic:
			e.fail(errChainFuzzStep)
			return false
		}
	case chainFuzzStepPreTrade, chainFuzzStepDropCopy:
		return e.applyOperationStep(t, index, step.mode)
	case chainFuzzStepCheckOrder:
		e.record(index, chainFuzzPhaseDriver)
		switch step.mode {
		case chainFuzzCheckOK:
			e.record(index, chainFuzzPhaseChecked)
			return true
		case chainFuzzCheckHookError:
			e.record(index, chainFuzzPhaseChecked)
			e.fail(errChainFuzzHook)
			return false
		case chainFuzzCheckDriverError:
			e.fail(errChainFuzzDriver)
			return false
		}
	case chainFuzzStepExecutionReport:
		e.record(index, chainFuzzPhaseReport)
		if step.mode == chainFuzzReportInputError ||
			step.mode == chainFuzzReportInputPanic {
			e.fail(errChainFuzzStep)
			return false
		}
		e.recordStatefulDriver(index)
		switch step.mode {
		case chainFuzzReportOK:
			e.record(index, chainFuzzPhaseSettled)
			return true
		case chainFuzzReportHookError:
			e.record(index, chainFuzzPhaseSettled)
			e.fail(errChainFuzzHook)
			return false
		case chainFuzzReportDriverError:
			e.fail(errChainFuzzDriver)
			return false
		}
	case chainFuzzStepAccountAdjustment:
		e.record(index, chainFuzzPhaseAdjustments)
		if step.mode == chainFuzzAdjustInputError ||
			step.mode == chainFuzzAdjustInputPanic {
			e.fail(errChainFuzzStep)
			return false
		}
		e.recordStatefulDriver(index)
		switch step.mode {
		case chainFuzzAdjustEmpty, chainFuzzAdjustBatch:
			e.record(index, chainFuzzPhaseAdjusted)
			return true
		case chainFuzzAdjustEmptyHookError,
			chainFuzzAdjustBatchHookError:
			e.record(index, chainFuzzPhaseAdjusted)
			e.fail(errChainFuzzHook)
			return false
		case chainFuzzAdjustDriverError:
			e.fail(errChainFuzzDriver)
			return false
		}
	}

	t.Errorf(
		"expected result hit step kind %d with mode %d",
		step.kind,
		step.mode,
	)
	e.fail(errChainFuzzHarness)
	return false
}

func (e *chainFuzzExpectation) applyOperationStep(
	t *testing.T,
	index int,
	mode byte,
) bool {
	t.Helper()
	e.recordStatefulDriver(index)
	switch mode {
	case chainFuzzOpCommit:
		e.record(index, chainFuzzPhaseAccepted)
		e.commits++
		return true
	case chainFuzzOpRollback:
		e.record(index, chainFuzzPhaseAccepted)
		e.rollbacks++
		e.status = ChainOutcomeRejected
		return false
	case chainFuzzOpHookError, chainFuzzOpHookPanic:
		e.record(index, chainFuzzPhaseAccepted)
		e.rollbacks++
		e.fail(errChainFuzzHook)
		return false
	case chainFuzzOpInvalidDecision:
		e.record(index, chainFuzzPhaseAccepted)
		e.rollbacks++
		e.fail(ErrChainInvalidDecision)
		return false
	case chainFuzzOpReject:
		e.record(index, chainFuzzPhaseRejected)
		e.status = ChainOutcomeRejected
		return false
	case chainFuzzOpRejectHookError:
		e.record(index, chainFuzzPhaseRejected)
		e.fail(errChainFuzzRejectHook)
		return false
	case chainFuzzOpDriverError, chainFuzzOpDriverPanic:
		e.fail(errChainFuzzDriver)
		return false
	default:
		t.Errorf("expected result hit an unknown operation mode %d", mode)
		e.fail(errChainFuzzHarness)
		return false
	}
}

func (e *chainFuzzExpectation) recordStatefulDriver(index int) {
	e.record(index, chainFuzzPhaseDriver)
	e.statefulCalls++
	e.retryUnsafe = true
}

func (e *chainFuzzExpectation) record(index int, phase chainFuzzPhase) {
	e.events = append(e.events, chainFuzzEvent{phase: phase, index: index})
}

func (e *chainFuzzExpectation) fail(err error) {
	e.status = ChainOutcomeFailed
	e.errors = append(e.errors, err)
}

// chainFuzzReachesDriver reports whether a step the chain entered must also
// reach the driver. Execution-report and account-adjustment input hooks can end
// their own step before the stateful call; every other kind starts at the
// driver or prepares input without a failure mode.
func chainFuzzReachesDriver(step chainFuzzStep) bool {
	switch step.kind {
	case chainFuzzStepExecutionReport:
		return step.mode != chainFuzzReportInputError &&
			step.mode != chainFuzzReportInputPanic
	case chainFuzzStepAccountAdjustment:
		return step.mode != chainFuzzAdjustInputError &&
			step.mode != chainFuzzAdjustInputPanic
	default:
		return true
	}
}

// chainFuzzScheduledStep is the driver side of one step: how that step behaves
// and where it sits in the shape, so a driver call names the step it serves.
type chainFuzzScheduledStep struct {
	index int
	mode  byte
}

// chainFuzzTrace records every observable chain event in order. entered keeps
// the coarser step prefix used to check which per-kind driver queues drained.
//
// It is written from the chain's lane goroutine only and read after the chain
// future resolved.
type chainFuzzTrace struct {
	events  []chainFuzzEvent
	entered []int
}

func (tr *chainFuzzTrace) enter(index int, phase chainFuzzPhase) {
	tr.entered = append(tr.entered, index)
	tr.record(index, phase)
}

// hook checks that a result hook belongs to the step most recently entered,
// then records the hook itself so omitting it is observable.
func (tr *chainFuzzTrace) hook(
	t *testing.T,
	index int,
	phase chainFuzzPhase,
) {
	t.Helper()
	if len(tr.entered) == 0 || tr.entered[len(tr.entered)-1] != index {
		t.Errorf(
			"%s hook of step %d ran after entered steps %v",
			phase,
			index,
			tr.entered,
		)
	}
	tr.record(index, phase)
}

func (tr *chainFuzzTrace) record(index int, phase chainFuzzPhase) {
	tr.events = append(tr.events, chainFuzzEvent{phase: phase, index: index})
}

// chainFuzzDriver serves one generated chain. Every call takes the mode its
// step scheduled, and the accepting modes hand out operations of a real engine
// carrying the mutation probe, so the probe reports how the chain finalized
// each one. Dry runs come from a second engine without the probe: they are
// pre-trade checks too, and counting their mutations next to the operations the
// chain owns would say nothing about the chain.
//
// It is called from the chain's lane goroutine only, and its counters are read
// after the chain future resolved.
type chainFuzzDriver struct {
	*acceptingDriver
	t             *testing.T
	trace         *chainFuzzTrace
	engine        native.Engine
	dryRunEngine  native.Engine
	preTradeSteps []chainFuzzScheduledStep
	dropCopySteps []chainFuzzScheduledStep
	checkSteps    []chainFuzzScheduledStep
	reportSteps   []chainFuzzScheduledStep
	adjustSteps   []chainFuzzScheduledStep
	statefulCalls int
}

// schedule queues the driver side of one step under its index in the shape. A
// caller step never reaches the driver.
func (d *chainFuzzDriver) schedule(index int, step chainFuzzStep) {
	scheduled := chainFuzzScheduledStep{index: index, mode: step.mode}
	switch step.kind {
	case chainFuzzStepPreTrade:
		d.preTradeSteps = append(d.preTradeSteps, scheduled)
	case chainFuzzStepDropCopy:
		d.dropCopySteps = append(d.dropCopySteps, scheduled)
	case chainFuzzStepCheckOrder:
		d.checkSteps = append(d.checkSteps, scheduled)
	case chainFuzzStepExecutionReport:
		d.reportSteps = append(d.reportSteps, scheduled)
	case chainFuzzStepAccountAdjustment:
		d.adjustSteps = append(d.adjustSteps, scheduled)
	}
}

// nextStep takes the step this call belongs to. A call with no step left to
// serve means the chain called the driver more often than the shape has steps,
// which is a defect of its own.
func (d *chainFuzzDriver) nextStep(
	queue *[]chainFuzzScheduledStep,
	call string,
) (chainFuzzScheduledStep, bool) {
	if len(*queue) == 0 {
		d.t.Errorf("%s called more often than the chain has steps for it", call)
		return chainFuzzScheduledStep{}, false
	}
	step := (*queue)[0]
	*queue = (*queue)[1:]
	return step, true
}

func (d *chainFuzzDriver) ExecutePreTrade(
	order model.Order,
) (*pretrade.Reservation, []reject.Reject, error) {
	d.statefulCalls++
	step, ok := d.nextStep(&d.preTradeSteps, "ExecutePreTrade")
	if !ok {
		return nil, nil, errChainFuzzHarness
	}
	// The driver call is the first thing this step does, so an error or a panic
	// below still counts as a step the chain entered.
	d.trace.enter(step.index, chainFuzzPhaseDriver)
	switch step.mode {
	case chainFuzzOpReject, chainFuzzOpRejectHookError:
		return nil, []reject.Reject{{}}, nil
	case chainFuzzOpDriverError:
		return nil, nil, errChainFuzzDriver
	case chainFuzzOpDriverPanic:
		panic(errChainFuzzDriver)
	}
	handle, rejects, err := native.EngineExecutePreTrade(d.engine, order.Handle())
	runtime.KeepAlive(order)
	if err != nil {
		d.t.Errorf("EngineExecutePreTrade() error = %v", err)
		return nil, nil, err
	}
	if rejects != nil {
		native.DestroyPretradeRejectList(rejects)
		d.t.Error("EngineExecutePreTrade() rejected a valid order")
		return nil, nil, errChainFuzzHarness
	}
	return pretrade.NewReservationFromHandle(handle), nil, nil
}

func (d *chainFuzzDriver) ApplyDropCopy(
	order model.Order,
) (*pretrade.DropCopyOperation, []reject.Reject, error) {
	d.statefulCalls++
	step, ok := d.nextStep(&d.dropCopySteps, "ApplyDropCopy")
	if !ok {
		return nil, nil, errChainFuzzHarness
	}
	d.trace.enter(step.index, chainFuzzPhaseDriver)
	switch step.mode {
	case chainFuzzOpReject, chainFuzzOpRejectHookError:
		return nil, []reject.Reject{{}}, nil
	case chainFuzzOpDriverError:
		return nil, nil, errChainFuzzDriver
	case chainFuzzOpDriverPanic:
		panic(errChainFuzzDriver)
	}
	handle, rejects, err := native.EngineApplyDropCopy(d.engine, order.Handle())
	runtime.KeepAlive(order)
	if err != nil {
		d.t.Errorf("EngineApplyDropCopy() error = %v", err)
		return nil, nil, err
	}
	if rejects != nil {
		native.DestroyPretradeRejectList(rejects)
		d.t.Error("EngineApplyDropCopy() rejected a valid order")
		return nil, nil, errChainFuzzHarness
	}
	return pretrade.NewDropCopyOperationFromHandle(handle), nil, nil
}

func (d *chainFuzzDriver) ExecutePreTradeDryRun(
	order model.Order,
) (*pretrade.DryRunReport, error) {
	step, ok := d.nextStep(&d.checkSteps, "ExecutePreTradeDryRun")
	if !ok {
		return nil, errChainFuzzHarness
	}
	d.trace.enter(step.index, chainFuzzPhaseDriver)
	if step.mode == chainFuzzCheckDriverError {
		return nil, errChainFuzzDriver
	}
	handle, err := native.EngineExecutePreTradeDryRun(
		d.dryRunEngine, order.Handle(),
	)
	runtime.KeepAlive(order)
	if err != nil {
		d.t.Errorf("EngineExecutePreTradeDryRun() error = %v", err)
		return nil, err
	}
	return pretrade.NewDryRunReportFromHandle(handle), nil
}

func (d *chainFuzzDriver) ApplyExecutionReport(
	report model.ExecutionReport,
) (pretrade.PostTradeResult, error) {
	d.statefulCalls++
	step, ok := d.nextStep(&d.reportSteps, "ApplyExecutionReport")
	if !ok {
		return pretrade.PostTradeResult{}, errChainFuzzHarness
	}
	d.trace.record(step.index, chainFuzzPhaseDriver)
	if step.mode == chainFuzzReportDriverError {
		return pretrade.PostTradeResult{}, errChainFuzzDriver
	}
	return d.acceptingDriver.ApplyExecutionReport(report)
}

func (d *chainFuzzDriver) ApplyAccountAdjustment(
	accountID param.AccountID,
	adjustments []model.AccountAdjustment,
) (accountadjustment.BatchResult, error) {
	// The chain marks itself retry unsafe on entry, whatever the batch holds;
	// count what the chain counts.
	d.statefulCalls++
	step, ok := d.nextStep(&d.adjustSteps, "ApplyAccountAdjustment")
	if !ok {
		return accountadjustment.BatchResult{}, errChainFuzzHarness
	}
	d.trace.record(step.index, chainFuzzPhaseDriver)
	if step.mode == chainFuzzAdjustDriverError {
		return accountadjustment.BatchResult{}, errChainFuzzDriver
	}
	return d.acceptingDriver.ApplyAccountAdjustment(accountID, adjustments)
}

// chainFuzzRun builds the generated chain and records what it actually did, so
// the invariants are checked against observations instead of against a second
// model of the chain runner.
type chainFuzzRun struct {
	t              *testing.T
	trace          *chainFuzzTrace
	report         model.ExecutionReport
	finallyOutcome ChainOutcome
	finallyCalls   int
}

func (r *chainFuzzRun) then(
	index int,
	mode byte,
) func(context.Context, struct{}) error {
	return func(context.Context, struct{}) error {
		r.trace.enter(index, chainFuzzPhaseThen)
		switch mode {
		case chainFuzzThenOK:
			return nil
		case chainFuzzThenError:
			return errChainFuzzStep
		case chainFuzzThenPanic:
			panic(errChainFuzzStep)
		default:
			r.t.Errorf("caller step ran for mode %d", mode)
			return errChainFuzzHarness
		}
	}
}

// decide answers an accepted operation hook. Only the modes that reach one may
// arrive here.
func (r *chainFuzzRun) decide(index int, mode byte) (Decision, error) {
	r.trace.hook(r.t, index, chainFuzzPhaseAccepted)
	switch mode {
	case chainFuzzOpCommit:
		return DecisionCommit, nil
	case chainFuzzOpRollback:
		return DecisionRollback, nil
	case chainFuzzOpHookError:
		return decisionInvalid, errChainFuzzHook
	case chainFuzzOpHookPanic:
		panic(errChainFuzzHook)
	case chainFuzzOpInvalidDecision:
		return chainFuzzInvalidDecision, nil
	default:
		r.t.Errorf("accepted hook ran for operation mode %d", mode)
		return decisionInvalid, errChainFuzzHarness
	}
}

// rejected answers a rejection hook, which only a rejecting driver reaches. It
// is a second entry point into the same step, so it carries the same check.
func (r *chainFuzzRun) rejected(index int, mode byte) error {
	r.trace.hook(r.t, index, chainFuzzPhaseRejected)
	switch mode {
	case chainFuzzOpReject:
		return nil
	case chainFuzzOpRejectHookError:
		return errChainFuzzRejectHook
	default:
		r.t.Errorf("reject hook ran for operation mode %d", mode)
		return errChainFuzzHarness
	}
}

func (r *chainFuzzRun) checked(
	index int,
	mode byte,
) func(context.Context, struct{}, OrderCheckResult) error {
	return func(context.Context, struct{}, OrderCheckResult) error {
		r.trace.hook(r.t, index, chainFuzzPhaseChecked)
		switch mode {
		case chainFuzzCheckOK:
			return nil
		case chainFuzzCheckHookError:
			return errChainFuzzHook
		default:
			r.t.Errorf("checked hook ran for check mode %d", mode)
			return errChainFuzzHarness
		}
	}
}

func (r *chainFuzzRun) reportHooks(
	index int,
	mode byte,
) ExecutionReportHooks[struct{}] {
	return ExecutionReportHooks[struct{}]{
		Report: func(context.Context, struct{}) (model.ExecutionReport, error) {
			// The chain asks for the report before it calls the driver, so this
			// hook is where the step first becomes observable.
			r.trace.enter(index, chainFuzzPhaseReport)
			if mode == chainFuzzReportInputError {
				return model.ExecutionReport{}, errChainFuzzStep
			}
			if mode == chainFuzzReportInputPanic {
				panic(errChainFuzzStep)
			}
			return r.report, nil
		},
		OnSettled: func(
			context.Context,
			struct{},
			pretrade.PostTradeResult,
		) error {
			r.trace.hook(r.t, index, chainFuzzPhaseSettled)
			switch mode {
			case chainFuzzReportOK:
				return nil
			case chainFuzzReportHookError:
				return errChainFuzzHook
			default:
				r.t.Errorf("settled hook ran for report mode %d", mode)
				return errChainFuzzHarness
			}
		},
	}
}

func (r *chainFuzzRun) adjustmentHooks(
	index int,
	mode byte,
) AccountAdjustmentHooks[struct{}] {
	return AccountAdjustmentHooks[struct{}]{
		Adjustments: func(
			context.Context,
			struct{},
		) ([]model.AccountAdjustment, error) {
			// The chain asks for the batch before it calls the driver, so this
			// hook is where the step first becomes observable.
			r.trace.enter(index, chainFuzzPhaseAdjustments)
			switch mode {
			case chainFuzzAdjustEmpty, chainFuzzAdjustEmptyHookError:
				return nil, nil
			case chainFuzzAdjustBatch,
				chainFuzzAdjustBatchHookError,
				chainFuzzAdjustDriverError:
				return []model.AccountAdjustment{model.NewAccountAdjustment()}, nil
			case chainFuzzAdjustInputError:
				return nil, errChainFuzzStep
			case chainFuzzAdjustInputPanic:
				panic(errChainFuzzStep)
			default:
				r.t.Errorf("adjustment input ran for mode %d", mode)
				return nil, errChainFuzzHarness
			}
		},
		OnAdjusted: func(
			context.Context,
			struct{},
			accountadjustment.BatchResult,
		) error {
			r.trace.hook(r.t, index, chainFuzzPhaseAdjusted)
			switch mode {
			case chainFuzzAdjustEmpty, chainFuzzAdjustBatch:
				return nil
			case chainFuzzAdjustEmptyHookError, chainFuzzAdjustBatchHookError:
				return errChainFuzzHook
			default:
				r.t.Errorf("adjusted hook ran for adjustment mode %d", mode)
				return errChainFuzzHarness
			}
		},
	}
}

// build assembles the generated chain. Every shape gets a terminal hook, so
// each run reports whether it ran and what it saw.
func (r *chainFuzzRun) build(
	order model.Order,
	shape chainFuzzShape,
) *ChainRunner[struct{}] {
	chain := Chain(order, func(context.Context) (struct{}, error) {
		r.trace.record(chainFuzzNoStep, chainFuzzPhaseBegin)
		switch shape.begin {
		case chainFuzzBeginOK:
			return struct{}{}, nil
		case chainFuzzBeginError:
			return struct{}{}, errChainFuzzBegin
		case chainFuzzBeginPanic:
			panic(errChainFuzzBegin)
		default:
			r.t.Errorf("begin ran for mode %d", shape.begin)
			return struct{}{}, errChainFuzzHarness
		}
	})
	for index, step := range shape.steps {
		switch step.kind {
		case chainFuzzStepThen:
			chain.Then(r.then(index, step.mode))
		case chainFuzzStepPreTrade:
			chain.ExecutePreTrade(PreTradeHooks[struct{}]{
				OnRejected: func(
					context.Context,
					struct{},
					[]reject.Reject,
				) error {
					return r.rejected(index, step.mode)
				},
				OnReserved: func(
					context.Context,
					struct{},
					OperationResult,
				) (Decision, error) {
					return r.decide(index, step.mode)
				},
			})
		case chainFuzzStepDropCopy:
			chain.ApplyDropCopy(DropCopyHooks[struct{}]{
				OnRejected: func(
					context.Context,
					struct{},
					[]reject.Reject,
				) error {
					return r.rejected(index, step.mode)
				},
				OnApplied: func(
					context.Context,
					struct{},
					DropCopyResult,
				) (Decision, error) {
					return r.decide(index, step.mode)
				},
			})
		case chainFuzzStepCheckOrder:
			chain.CheckOrder(r.checked(index, step.mode))
		case chainFuzzStepExecutionReport:
			chain.ApplyExecutionReport(r.reportHooks(index, step.mode))
		case chainFuzzStepAccountAdjustment:
			chain.ApplyAccountAdjustment(r.adjustmentHooks(index, step.mode))
		default:
			r.t.Errorf("chain built a step of unknown kind %d", step.kind)
		}
	}
	return chain.Finally(func(
		_ context.Context,
		_ struct{},
		outcome ChainOutcome,
	) error {
		// Record before failing: a terminal hook that panics still ran, and the
		// count is what proves it ran exactly once.
		r.trace.record(chainFuzzNoStep, chainFuzzPhaseFinally)
		r.finallyCalls++
		r.finallyOutcome = outcome
		switch shape.finally {
		case chainFuzzFinallyOK:
			return nil
		case chainFuzzFinallyError:
			return errChainFuzzFinally
		case chainFuzzFinallyPanic:
			panic(errChainFuzzFinally)
		default:
			r.t.Errorf("final hook ran for mode %d", shape.finally)
			return errChainFuzzHarness
		}
	})
}

// FuzzChainStateMachine runs generated chain shapes through the real runner and
// holds every one of them to the invariants that hold for any sequence of
// steps, decisions, and failures - the properties example-based tests can only
// pin one path at a time.
//
// The gate runs the seed corpus, which carries the shapes that already bit in
// review; -fuzz explores from there.
func FuzzChainStateMachine(f *testing.F) {
	for _, seed := range [][]byte{
		// A driver failure inside a stateful call.
		encodeChainFuzzInput(
			chainFuzzBeginOK,
			chainFuzzFinallyOK,
			chainFuzzStep{chainFuzzStepPreTrade, chainFuzzOpDriverError},
		),
		// An engine reject, and a reject hook that fails on top of it.
		encodeChainFuzzInput(
			chainFuzzBeginOK,
			chainFuzzFinallyOK,
			chainFuzzStep{chainFuzzStepPreTrade, chainFuzzOpReject},
		),
		encodeChainFuzzInput(
			chainFuzzBeginOK,
			chainFuzzFinallyOK,
			chainFuzzStep{chainFuzzStepDropCopy, chainFuzzOpRejectHookError},
		),
		// A commit followed by a later failure.
		encodeChainFuzzInput(
			chainFuzzBeginOK,
			chainFuzzFinallyOK,
			chainFuzzStep{chainFuzzStepPreTrade, chainFuzzOpCommit},
			chainFuzzStep{chainFuzzStepThen, chainFuzzThenError},
		),
		// A panicking hook and a panicking driver.
		encodeChainFuzzInput(
			chainFuzzBeginOK,
			chainFuzzFinallyOK,
			chainFuzzStep{chainFuzzStepDropCopy, chainFuzzOpHookPanic},
		),
		encodeChainFuzzInput(
			chainFuzzBeginOK,
			chainFuzzFinallyOK,
			chainFuzzStep{chainFuzzStepPreTrade, chainFuzzOpDriverPanic},
		),
		// An invalid decision, and a rollback that ends the chain.
		encodeChainFuzzInput(
			chainFuzzBeginOK,
			chainFuzzFinallyOK,
			chainFuzzStep{chainFuzzStepPreTrade, chainFuzzOpInvalidDecision},
		),
		encodeChainFuzzInput(
			chainFuzzBeginOK,
			chainFuzzFinallyOK,
			chainFuzzStep{chainFuzzStepDropCopy, chainFuzzOpRollback},
			chainFuzzStep{chainFuzzStepThen, chainFuzzThenOK},
		),
		// An empty adjustment batch and the same batch with content: entering
		// the call marks the chain retry unsafe either way.
		encodeChainFuzzInput(
			chainFuzzBeginOK,
			chainFuzzFinallyOK,
			chainFuzzStep{
				chainFuzzStepAccountAdjustment, chainFuzzAdjustEmptyHookError,
			},
		),
		encodeChainFuzzInput(
			chainFuzzBeginOK,
			chainFuzzFinallyOK,
			chainFuzzStep{
				chainFuzzStepAccountAdjustment, chainFuzzAdjustBatchHookError,
			},
		),
		// A non-mutating check next to the calls that do mutate.
		encodeChainFuzzInput(
			chainFuzzBeginOK,
			chainFuzzFinallyOK,
			chainFuzzStep{chainFuzzStepCheckOrder, chainFuzzCheckOK},
			chainFuzzStep{chainFuzzStepPreTrade, chainFuzzOpCommit},
			chainFuzzStep{chainFuzzStepExecutionReport, chainFuzzReportOK},
			chainFuzzStep{chainFuzzStepAccountAdjustment, chainFuzzAdjustBatch},
		),
		encodeChainFuzzInput(
			chainFuzzBeginOK,
			chainFuzzFinallyOK,
			chainFuzzStep{chainFuzzStepCheckOrder, chainFuzzCheckDriverError},
		),
		// The report the chain never got to apply, and the settled hook that
		// failed after it did.
		encodeChainFuzzInput(
			chainFuzzBeginOK,
			chainFuzzFinallyOK,
			chainFuzzStep{
				chainFuzzStepExecutionReport, chainFuzzReportInputError,
			},
		),
		encodeChainFuzzInput(
			chainFuzzBeginOK,
			chainFuzzFinallyOK,
			chainFuzzStep{
				chainFuzzStepExecutionReport, chainFuzzReportInputPanic,
			},
		),
		encodeChainFuzzInput(
			chainFuzzBeginOK,
			chainFuzzFinallyOK,
			chainFuzzStep{
				chainFuzzStepAccountAdjustment, chainFuzzAdjustInputError,
			},
		),
		encodeChainFuzzInput(
			chainFuzzBeginOK,
			chainFuzzFinallyOK,
			chainFuzzStep{
				chainFuzzStepAccountAdjustment, chainFuzzAdjustInputPanic,
			},
		),
		// Input-hook failures after a commit stop before their own driver call,
		// but retain the retry-unsafe state accumulated by the earlier call.
		encodeChainFuzzInput(
			chainFuzzBeginOK,
			chainFuzzFinallyOK,
			chainFuzzStep{chainFuzzStepPreTrade, chainFuzzOpCommit},
			chainFuzzStep{
				chainFuzzStepExecutionReport, chainFuzzReportInputError,
			},
		),
		encodeChainFuzzInput(
			chainFuzzBeginOK,
			chainFuzzFinallyOK,
			chainFuzzStep{chainFuzzStepPreTrade, chainFuzzOpCommit},
			chainFuzzStep{
				chainFuzzStepExecutionReport, chainFuzzReportInputPanic,
			},
		),
		encodeChainFuzzInput(
			chainFuzzBeginOK,
			chainFuzzFinallyOK,
			chainFuzzStep{chainFuzzStepPreTrade, chainFuzzOpCommit},
			chainFuzzStep{
				chainFuzzStepAccountAdjustment, chainFuzzAdjustInputError,
			},
		),
		encodeChainFuzzInput(
			chainFuzzBeginOK,
			chainFuzzFinallyOK,
			chainFuzzStep{chainFuzzStepPreTrade, chainFuzzOpCommit},
			chainFuzzStep{
				chainFuzzStepAccountAdjustment, chainFuzzAdjustInputPanic,
			},
		),
		encodeChainFuzzInput(
			chainFuzzBeginOK,
			chainFuzzFinallyOK,
			chainFuzzStep{chainFuzzStepExecutionReport, chainFuzzReportHookError},
		),
		// A begin that never produces State, so no terminal hook may run - with
		// a terminal hook that would fail if it did, so a chain that runs it
		// anyway is caught by more than the count.
		encodeChainFuzzInput(
			chainFuzzBeginError,
			chainFuzzFinallyOK,
			chainFuzzStep{chainFuzzStepPreTrade, chainFuzzOpCommit},
		),
		encodeChainFuzzInput(
			chainFuzzBeginPanic,
			chainFuzzFinallyOK,
			chainFuzzStep{chainFuzzStepDropCopy, chainFuzzOpCommit},
		),
		encodeChainFuzzInput(
			chainFuzzBeginError,
			chainFuzzFinallyError,
			chainFuzzStep{chainFuzzStepThen, chainFuzzThenOK},
		),
		// A chain that did everything right and still ends with an error,
		// because its terminal hook failed. This is the only shape where the
		// status and the error can disagree, so it is the only one that can
		// prove they do not.
		encodeChainFuzzInput(
			chainFuzzBeginOK,
			chainFuzzFinallyError,
			chainFuzzStep{chainFuzzStepThen, chainFuzzThenOK},
		),
		encodeChainFuzzInput(
			chainFuzzBeginOK,
			chainFuzzFinallyPanic,
			chainFuzzStep{chainFuzzStepPreTrade, chainFuzzOpCommit},
		),
		// The same, after a rejection: the chain ends rejected, and only the
		// terminal hook turns that into a failure.
		encodeChainFuzzInput(
			chainFuzzBeginOK,
			chainFuzzFinallyError,
			chainFuzzStep{chainFuzzStepDropCopy, chainFuzzOpReject},
		),
		encodeChainFuzzInput(
			chainFuzzBeginOK,
			chainFuzzFinallyError,
			chainFuzzStep{chainFuzzStepPreTrade, chainFuzzOpRollback},
		),
		// A terminal hook that fails on top of a chain that already failed.
		encodeChainFuzzInput(
			chainFuzzBeginOK,
			chainFuzzFinallyPanic,
			chainFuzzStep{chainFuzzStepPreTrade, chainFuzzOpHookError},
		),
	} {
		f.Add(seed)
	}
	f.Fuzz(func(t *testing.T, data []byte) {
		shape, ok := decodeChainFuzzShape(data)
		if !ok {
			t.Skip("input describes no chain this harness can build")
		}
		runChainFuzzShape(t, shape)
	})
}

// runChainFuzzShape runs one generated chain and asserts the invariants that
// must hold whatever it did.
func runChainFuzzShape(t *testing.T, shape chainFuzzShape) {
	t.Helper()
	var commits, rollbacks atomic.Int64
	probe := &chainMutationProbe{
		onCommit:   func() { commits.Add(1) },
		onRollback: func() { rollbacks.Add(1) },
	}
	trace := &chainFuzzTrace{}
	// The native engines must outlive the async engine: cleanups run last in
	// first out, so they are created first and destroyed after the chain lane
	// has stopped.
	driver := &chainFuzzDriver{
		acceptingDriver: newAcceptingDriver(),
		t:               t,
		trace:           trace,
		engine:          newChainProbeEngine(t, probe),
		dryRunEngine:    newChainNativeEngine(t),
	}
	for index, step := range shape.steps {
		driver.schedule(index, step)
	}
	engine := newChainEngine(t, driver)
	run := &chainFuzzRun{
		t:      t,
		trace:  trace,
		report: buildTestReport(t, chainOrderAccountID),
	}
	runner := run.build(buildChainCheckOrder(t), shape)

	outcome, err := runner.Run(context.Background(), engine).Await(
		context.Background(),
	)

	// The independent model names every required driver and hook event. A runner
	// cannot make a skipped callback disappear from both the run and its oracle.
	expected := chainFuzzExpected(t, shape)
	if !slices.Equal(trace.events, expected.events) {
		t.Errorf("chain events = %v, want %v", trace.events, expected.events)
	}
	if !slices.Equal(trace.entered, expected.entered) {
		t.Errorf("entered steps = %v, want %v", trace.entered, expected.entered)
	}
	assertChainFuzzQueuesDrained(t, driver, shape, expected.entered)

	// The terminal hook runs exactly once for a chain that started, and never
	// for one whose begin produced no State.
	if run.finallyCalls != expected.finallyCalls {
		t.Errorf(
			"Finally calls = %d, want %d",
			run.finallyCalls,
			expected.finallyCalls,
		)
	}
	if expected.finallyCalls > 0 {
		assertChainFuzzOutcome(
			t,
			"Finally",
			run.finallyOutcome,
			expected.beforeFinallyStatus,
			expected.beforeFinallyErrors,
			expected.retryUnsafe,
		)
	}
	assertChainFuzzOutcome(
		t,
		"resolved",
		outcome,
		expected.status,
		expected.errors,
		expected.retryUnsafe,
	)
	assertChainFuzzErrors(
		t,
		"Await",
		err,
		chainFuzzExpectedErrors(
			expected.status,
			expected.errors,
			expected.retryUnsafe,
		),
	)
	if err != nil && !errors.Is(outcome.Err, err) {
		t.Errorf("outcome error = %v, want the awaited %v", outcome.Err, err)
	}

	if driver.statefulCalls != expected.statefulCalls {
		t.Errorf(
			"stateful driver calls = %d, want %d",
			driver.statefulCalls,
			expected.statefulCalls,
		)
	}

	// Finalization comes from the generated shape, not from which decision hooks
	// happened to run, so skipping a hook cannot turn its required commit or
	// rollback into the expected result.
	if got := commits.Load(); got != expected.commits {
		t.Errorf("mutation commit calls = %d, want %d", got, expected.commits)
	}
	if got := rollbacks.Load(); got != expected.rollbacks {
		t.Errorf("mutation rollback calls = %d, want %d", got, expected.rollbacks)
	}
}

func assertChainFuzzOutcome(
	t *testing.T,
	label string,
	outcome ChainOutcome,
	wantStatus ChainOutcomeStatus,
	wantErrors []error,
	wantRetryUnsafe bool,
) {
	t.Helper()
	if outcome.Status != wantStatus {
		t.Errorf("%s outcome status = %v, want %v", label, outcome.Status, wantStatus)
	}
	if outcome.RetryUnsafe != wantRetryUnsafe {
		t.Errorf(
			"%s outcome RetryUnsafe = %v, want %v",
			label,
			outcome.RetryUnsafe,
			wantRetryUnsafe,
		)
	}
	assertChainFuzzErrors(
		t,
		label+" outcome",
		outcome.Err,
		chainFuzzExpectedErrors(wantStatus, wantErrors, wantRetryUnsafe),
	)
}

func chainFuzzExpectedErrors(
	status ChainOutcomeStatus,
	errs []error,
	retryUnsafe bool,
) []error {
	expected := append([]error(nil), errs...)
	if status == ChainOutcomeFailed && retryUnsafe {
		expected = append(expected, ErrChainRetryUnsafe)
	}
	return expected
}

func assertChainFuzzErrors(
	t *testing.T,
	label string,
	got error,
	want []error,
) {
	t.Helper()
	gotLeaves := chainFuzzErrorLeaves(got)
	if len(gotLeaves) != len(want) {
		t.Errorf("%s error leaves = %v, want %v", label, gotLeaves, want)
	}
	matched := make([]bool, len(gotLeaves))
	for _, wantErr := range want {
		found := false
		for index, gotErr := range gotLeaves {
			if !matched[index] && errors.Is(gotErr, wantErr) {
				matched[index] = true
				found = true
				break
			}
		}
		if !found {
			t.Errorf("%s error = %v, want it to wrap %v", label, got, wantErr)
		}
	}
}

func chainFuzzErrorLeaves(err error) []error {
	if err == nil {
		return nil
	}
	if joined, ok := err.(interface{ Unwrap() []error }); ok {
		var leaves []error
		for _, child := range joined.Unwrap() {
			leaves = append(leaves, chainFuzzErrorLeaves(child)...)
		}
		return leaves
	}
	if wrapped, ok := err.(interface{ Unwrap() error }); ok {
		return chainFuzzErrorLeaves(wrapped.Unwrap())
	}
	return []error{err}
}

// assertChainFuzzQueuesDrained checks that every step the chain entered also
// reached the engine. Entering is not performing: the kinds whose input the
// chain prepares first are traced by their own hook, so a driver call that went
// missing would leave the trace, the counters, and the outcome all intact. What
// proves the call happened is that the step left the driver's queue - and that
// the steps the chain never reached are still in it.
func assertChainFuzzQueuesDrained(
	t *testing.T,
	driver *chainFuzzDriver,
	shape chainFuzzShape,
	entered []int,
) {
	t.Helper()
	for _, queue := range []struct {
		remaining []chainFuzzScheduledStep
		call      string
		kind      byte
	}{
		{driver.preTradeSteps, "ExecutePreTrade", chainFuzzStepPreTrade},
		{driver.dropCopySteps, "ApplyDropCopy", chainFuzzStepDropCopy},
		{driver.checkSteps, "ExecutePreTradeDryRun", chainFuzzStepCheckOrder},
		{
			driver.reportSteps,
			"ApplyExecutionReport",
			chainFuzzStepExecutionReport,
		},
		{
			driver.adjustSteps,
			"ApplyAccountAdjustment",
			chainFuzzStepAccountAdjustment,
		},
	} {
		unserved := make([]int, 0, len(queue.remaining))
		for _, step := range queue.remaining {
			unserved = append(unserved, step.index)
		}
		want := chainFuzzUnservedSteps(shape, entered, queue.kind)
		if !slices.Equal(unserved, want) {
			t.Errorf(
				"%s left steps %v unserved, want %v",
				queue.call,
				unserved,
				want,
			)
		}
	}
}

// chainFuzzUnservedSteps lists the steps of one kind whose driver call the
// chain must not have made: every step past the point where it stops, plus the
// entered step whose entry point ends it before the driver.
func chainFuzzUnservedSteps(
	shape chainFuzzShape,
	entered []int,
	kind byte,
) []int {
	unserved := make([]int, 0, len(shape.steps))
	for index, step := range shape.steps {
		if step.kind != kind {
			continue
		}
		if slices.Contains(entered, index) && chainFuzzReachesDriver(step) {
			continue
		}
		unserved = append(unserved, index)
	}
	return unserved
}
