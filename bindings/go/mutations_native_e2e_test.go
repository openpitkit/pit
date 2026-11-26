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

package openpit

import (
	"errors"
	"slices"
	"strings"
	"testing"

	"go.openpit.dev/openpit/accountadjustment"
	"go.openpit.dev/openpit/model"
	"go.openpit.dev/openpit/param"
	"go.openpit.dev/openpit/pretrade"
	"go.openpit.dev/openpit/reject"
	"go.openpit.dev/openpit/tx"
)

func requireAppliedDropCopy(
	t *testing.T,
	engine *Engine,
	order model.Order,
) *pretrade.DropCopyOperation {
	t.Helper()
	operation, rejects, err := engine.ApplyDropCopy(order)
	if err != nil {
		t.Fatalf("ApplyDropCopy() error = %v", err)
	}
	if rejects != nil {
		t.Fatalf("ApplyDropCopy() rejects = %v, want nil", rejects)
	}
	if operation == nil {
		t.Fatal("ApplyDropCopy() operation = nil, want non-nil")
	}
	return operation
}

func TestCustomPolicyReviewModes(t *testing.T) {
	for _, tc := range []struct {
		name string
		mode reject.RuleMode
	}{
		{name: "accept"},
		{name: "reject", mode: reject.RuleModeReject},
		{name: "review-with-hold", mode: reject.RuleModeReviewWithHold},
		{name: "review-without-hold", mode: reject.RuleModeReviewWithoutHold},
	} {
		t.Run(tc.name, func(t *testing.T) {
			policy := &reviewTestPolicy{
				lockPrice: mustOrderNativePrice(t, "99"),
				asset:     mustOrderNativeAsset(t, "USD"),
			}
			var wantRejects, wantReviews []reject.Reject
			if tc.name != "accept" {
				policy.main = reject.NewSingleItemList(
					reject.CodeRiskLimitExceeded,
					policy.Name(),
					"risk limit exceeded",
					"from the main stage",
					reject.ScopeOrder,
				)
				policy.main[0].Mode = tc.mode
				if tc.mode == reject.RuleModeReject {
					wantRejects = policy.main
				} else {
					wantReviews = policy.main
				}
			}
			engine := newEngineWithPreTradePolicyForNativeE2E(t, policy)
			defer engine.Stop()
			order := newValidOrderForNativeE2E(t)

			request, rejects, err := engine.StartPreTrade(order)
			if err != nil || request == nil || len(rejects) != 0 {
				t.Fatalf("StartPreTrade() = %v, %v, %v, want an accepted request", request, rejects, err)
			}
			defer request.Close()
			assertReviewMarks(t, request.Reviews, nil)
			reservation, rejects, err := request.Execute()
			if err != nil || !slices.Equal(rejects, wantRejects) {
				t.Fatalf("Execute() rejects = %v, error = %v, want %v", rejects, err, wantRejects)
			}
			if (reservation == nil) != (len(wantRejects) != 0) {
				t.Fatalf("Execute() reservation = %v, rejects = %v", reservation, rejects)
			}
			if reservation != nil {
				defer reservation.Close()
				assertReviewMarks(t, reservation.Reviews, wantReviews)
				assertReviewResult(t, reservation, tc.mode)
				reservation.RollbackAndClose()
				if _, err := reservation.Reviews(); !errors.Is(err, pretrade.ErrReservationClosed) {
					t.Fatalf("closed Reservation.Reviews() error = %v", err)
				}
			}
			if _, err := request.Reviews(); err == nil || errors.Is(err, pretrade.ErrRequestClosed) {
				t.Fatalf("executed Request.Reviews() error = %v, want native error", err)
			}
			request.Close()
			if _, err := request.Reviews(); !errors.Is(err, pretrade.ErrRequestClosed) {
				t.Fatalf("closed Request.Reviews() error = %v", err)
			}

			report, err := engine.ExecutePreTradeDryRun(order)
			if err != nil {
				t.Fatalf("ExecutePreTradeDryRun() error = %v", err)
			}
			defer report.Close()
			passed, err := report.IsPass()
			if err != nil || passed != (len(wantRejects) == 0) {
				t.Fatalf("IsPass() = %v, %v, want pass = %v", passed, err, len(wantRejects) == 0)
			}
			assertReviewMarks(t, report.Rejects, wantRejects)
			assertReviewMarks(t, report.Reviews, wantReviews)
			assertReviewResult(t, report, tc.mode)
			report.Close()
			if _, err := report.Reviews(); !errors.Is(err, pretrade.ErrDryRunReportClosed) {
				t.Fatalf("closed DryRunReport.Reviews() error = %v", err)
			}
		})
	}
}

func TestCustomPolicyReceivesConfirmedOrder(t *testing.T) {
	policy := &reviewTestPolicy{}
	policy.start = reject.NewSingleItemList(
		reject.CodeRiskLimitExceeded, policy.Name(), "requires review", "from the start stage",
		reject.ScopeOrder,
	)
	policy.start[0].Mode = reject.RuleModeReviewWithHold
	engine := newEngineWithPreTradePolicyForNativeE2E(t, policy)
	defer engine.Stop()

	for _, confirmed := range []bool{false, true} {
		order := newValidOrderForNativeE2E(t)
		want := policy.start
		if confirmed {
			review := order.EnsureReviewView()
			review.SetConfirmed(true)
			want = nil
		}
		request, rejects, err := engine.StartPreTrade(order)
		if err != nil || request == nil || len(rejects) != 0 {
			t.Fatalf("StartPreTrade(confirmed = %v) = %v, %v, %v", confirmed, request, rejects, err)
		}
		defer request.Close()
		assertReviewMarks(t, request.Reviews, want)
		reservation, rejects, err := request.Execute()
		if err != nil || reservation == nil || len(rejects) != 0 {
			t.Fatalf("Execute(confirmed = %v) = %v, %v, %v", confirmed, reservation, rejects, err)
		}
		defer reservation.Close()
		assertReviewMarks(t, reservation.Reviews, want)
		reservation.RollbackAndClose()
	}
}

func assertReviewMarks(t *testing.T, read func() ([]reject.Reject, error), want []reject.Reject) {
	t.Helper()
	got, err := read()
	if err != nil || !slices.Equal(got, want) {
		t.Fatalf("review or reject marks = %v, %v, want %v", got, err, want)
	}
}

func assertReviewResult(t *testing.T, result interface {
	Lock() (pretrade.Lock, error)
	AccountAdjustments() ([]accountadjustment.Outcome, error)
}, mode reject.RuleMode) {
	t.Helper()
	if mode != reject.RuleModeReviewWithHold && mode != reject.RuleModeReviewWithoutHold {
		return
	}
	lock, lockErr := result.Lock()
	var lockCount int
	var lockCountErr error
	if len(lock.Bytes()) != 0 {
		lockCount, lockCountErr = lock.Len()
	}
	adjustments, adjustmentsErr := result.AccountAdjustments()
	wantCount := 0
	if mode == reject.RuleModeReviewWithHold {
		wantCount = 1
	}
	if lockErr != nil || lockCountErr != nil || adjustmentsErr != nil ||
		lockCount != wantCount || len(adjustments) != wantCount {
		t.Fatalf("mode %v: lock has %d prices, adjustments = %v, errors = %v, %v, %v, want %d of each",
			mode, lockCount, adjustments, lockErr, lockCountErr, adjustmentsErr, wantCount)
	}
}

type reviewTestPolicy struct {
	orderNativeE2ENoopStartPolicy
	start     []reject.Reject
	main      []reject.Reject
	lockPrice param.Price
	asset     param.Asset
}

func (*reviewTestPolicy) Name() string { return "review-test" }

func (p *reviewTestPolicy) CheckPreTradeStart(_ pretrade.Context, order model.Order) []reject.Reject {
	if review, ok := order.Review().Get(); ok && review.Confirmed().Or(false) {
		return nil
	}
	return p.start
}

func (p *reviewTestPolicy) PerformPreTradeCheck(
	_ pretrade.Context,
	_ model.Order,
	_ tx.Mutations,
	result pretrade.Result,
) []reject.Reject {
	if len(p.main) != 0 {
		if err := result.PushLockPrice(p.lockPrice); err != nil {
			panic(err)
		}
		if err := result.PushAccountAdjustment(accountadjustment.AccountOutcomeEntry{Asset: p.asset}); err != nil {
			panic(err)
		}
	}
	return p.main
}

func (p *reviewTestPolicy) CheckPreTradeStartDryRun(ctx pretrade.Context, order model.Order) []reject.Reject {
	return p.CheckPreTradeStart(ctx, order)
}

func (p *reviewTestPolicy) PerformPreTradeCheckDryRun(
	ctx pretrade.Context,
	order model.Order,
	mutations tx.Mutations,
	result pretrade.Result,
) []reject.Reject {
	return p.PerformPreTradeCheck(ctx, order, mutations, result)
}

func requireRejectedDropCopy(
	t *testing.T,
	engine *Engine,
	order model.Order,
) []reject.Reject {
	t.Helper()
	operation, rejects, err := engine.ApplyDropCopy(order)
	if err != nil {
		t.Fatalf("ApplyDropCopy() error = %v", err)
	}
	if operation != nil {
		operation.Close()
		t.Fatal("ApplyDropCopy() operation != nil, want nil")
	}
	return rejects
}

// mutationOrderAccountID is the account newValidOrderForNativeE2E trades on.
const mutationOrderAccountID uint64 = 1001

// mutationFinalizerBlockReason is the cause the engine records for the kill
// switch a failed mutation finalizer arms.
const mutationFinalizerBlockReason = "mutation finalizer failed"

// commitFailingReservation drives one accepted pre-trade for the policy under
// test and commits it, which runs its failing commit callback.
func commitFailingReservation(t *testing.T, engine *Engine) {
	t.Helper()
	reservation, rejects, err := engine.ExecutePreTrade(newValidOrderForNativeE2E(t))
	if err != nil {
		t.Fatalf("ExecutePreTrade() error = %v", err)
	}
	if reservation == nil {
		t.Fatalf("ExecutePreTrade() rejects = %v, want reservation", rejects)
	}
	reservation.CommitAndClose()
}

// assertMutationFinalizerBlock drives one StartPreTrade for accountID and fails
// unless it is rejected with the engine-wide mutation-finalizer cause.
func assertMutationFinalizerBlock(t *testing.T, engine *Engine, accountID uint64) {
	t.Helper()
	request, rejects, err := engine.StartPreTrade(rateLimitTestOrder(t, accountID))
	if err != nil {
		t.Fatalf("StartPreTrade(%d) error = %v", accountID, err)
	}
	if request != nil {
		request.Close()
		t.Fatalf("StartPreTrade(%d): request != nil, want kill-switch block", accountID)
	}
	if len(rejects) != 1 {
		t.Fatalf("StartPreTrade(%d): reject len = %d, want 1", accountID, len(rejects))
	}
	if rejects[0].Code != reject.CodeSystemUnavailable {
		t.Fatalf(
			"StartPreTrade(%d): reject code = %v, want %v",
			accountID, rejects[0].Code, reject.CodeSystemUnavailable,
		)
	}
	if rejects[0].Policy != "Engine" {
		t.Fatalf("StartPreTrade(%d): reject policy = %q, want Engine", accountID, rejects[0].Policy)
	}
	if rejects[0].Reason != mutationFinalizerBlockReason {
		t.Fatalf(
			"StartPreTrade(%d): reject reason = %q, want %q",
			accountID, rejects[0].Reason, mutationFinalizerBlockReason,
		)
	}
}

// assertPreTradePassesForAccount drives one StartPreTrade for accountID and
// fails unless it is accepted with no rejects.
func assertPreTradePassesForAccount(t *testing.T, engine *Engine, accountID uint64) {
	t.Helper()
	request, rejects, err := engine.StartPreTrade(rateLimitTestOrder(t, accountID))
	if err != nil {
		t.Fatalf("StartPreTrade(%d) error = %v", accountID, err)
	}
	if request == nil {
		t.Fatalf("StartPreTrade(%d): rejects = %v, want request", accountID, rejects)
	}
	request.Close()
}

func newFailingCommitEngineForNativeE2E(t *testing.T) *Engine {
	t.Helper()
	return newEngineWithPreTradePolicyForNativeE2E(t, &mutationTrackingPolicy{
		name:             "mutation-failing-commit",
		commitPanicValue: "go mutation commit exploded",
	})
}

// TestMutationsNativeE2E_FinalizerFailureBlocksEveryAccount pins the scope of
// the kill switch a failed finalizer arms. The mutation belongs to a custom
// policy, whose state reach the engine cannot bound, so an account the failing
// order never touched is blocked too.
func TestMutationsNativeE2E_FinalizerFailureBlocksEveryAccount(t *testing.T) {
	const untouchedAccountID uint64 = 2002
	engine := newFailingCommitEngineForNativeE2E(t)
	defer engine.Stop()
	assertPreTradePassesForAccount(t, engine, untouchedAccountID)

	commitFailingReservation(t, engine)

	assertMutationFinalizerBlock(t, engine, untouchedAccountID)

	engine.Accounts().UnblockAll()
	assertPreTradePassesForAccount(t, engine, untouchedAccountID)
}

// TestMutationsNativeE2E_UnblockAllKeepsIndividuallyBlockedAccount proves
// UnblockAll lifts only the engine-wide block: an account blocked by an
// operator keeps its own recorded cause, which wins over the engine-wide one.
func TestMutationsNativeE2E_UnblockAllKeepsIndividuallyBlockedAccount(t *testing.T) {
	const heldAccountID uint64 = 2002
	const untouchedAccountID uint64 = 2003
	engine := newFailingCommitEngineForNativeE2E(t)
	defer engine.Stop()
	engine.Accounts().Block(param.NewAccountIDFromUint64(heldAccountID), "compliance hold")
	commitFailingReservation(t, engine)

	engine.Accounts().UnblockAll()

	request, rejects, err := engine.StartPreTrade(rateLimitTestOrder(t, heldAccountID))
	if err != nil {
		t.Fatalf("StartPreTrade(%d) error = %v", heldAccountID, err)
	}
	if request != nil {
		request.Close()
		t.Fatalf("StartPreTrade(%d): request != nil, want operator block", heldAccountID)
	}
	if len(rejects) != 1 || rejects[0].Code != reject.CodeAccountBlocked {
		t.Fatalf("StartPreTrade(%d): rejects = %v, want AccountBlocked", heldAccountID, rejects)
	}
	if rejects[0].Reason != "compliance hold" {
		t.Fatalf(
			"StartPreTrade(%d): reject reason = %q, want %q",
			heldAccountID, rejects[0].Reason, "compliance hold",
		)
	}
	assertPreTradePassesForAccount(t, engine, untouchedAccountID)
}

func TestMutationsNativeE2E_CommitAndRollbackCallbacks(t *testing.T) {
	policy := &mutationTrackingPolicy{name: "mutation-tracking"}
	engine := newEngineWithPreTradePolicyForNativeE2E(t, policy)
	defer engine.Stop()

	reservation, rejects, err := engine.ExecutePreTrade(newValidOrderForNativeE2E(t))
	if err != nil {
		t.Fatalf("ExecutePreTrade() first error = %v", err)
	}
	if rejects != nil {
		t.Fatalf("ExecutePreTrade() first rejects = %v, want nil", rejects)
	}
	if reservation == nil {
		t.Fatal("ExecutePreTrade() first reservation = nil, want non-nil")
	}
	reservation.CommitAndClose()

	if policy.commitCalls != 1 {
		t.Fatalf("commitCalls after first reservation = %d, want 1", policy.commitCalls)
	}
	if policy.rollbackCalls != 0 {
		t.Fatalf("rollbackCalls after first reservation = %d, want 0", policy.rollbackCalls)
	}

	secondReservation, secondRejects, err := engine.ExecutePreTrade(newValidOrderForNativeE2E(t))
	if err != nil {
		t.Fatalf("ExecutePreTrade() second error = %v", err)
	}
	if secondRejects != nil {
		t.Fatalf("ExecutePreTrade() second rejects = %v, want nil", secondRejects)
	}
	if secondReservation == nil {
		t.Fatal("ExecutePreTrade() second reservation = nil, want non-nil")
	}
	secondReservation.RollbackAndClose()

	if policy.commitCalls != 1 {
		t.Fatalf("commitCalls after second reservation = %d, want 1", policy.commitCalls)
	}
	if policy.rollbackCalls != 1 {
		t.Fatalf("rollbackCalls after second reservation = %d, want 1", policy.rollbackCalls)
	}
}

func TestMutationsNativeE2E_RejectingPolicyReturnsRejectList(t *testing.T) {
	engine := newEngineWithPreTradePolicyForNativeE2E(
		t,
		&mutationTrackingPolicy{
			name:         "mutation-reject",
			shouldReject: true,
		},
	)
	defer engine.Stop()

	reservation, rejects, err := engine.ExecutePreTrade(newValidOrderForNativeE2E(t))
	if err != nil {
		t.Fatalf("ExecutePreTrade() error = %v", err)
	}
	if reservation != nil {
		reservation.Close()
		t.Fatal("ExecutePreTrade() reservation != nil, want nil")
	}
	if rejects == nil {
		t.Fatal("ExecutePreTrade() rejects = nil, want non-nil")
	}
	if len(rejects) != 1 {
		t.Fatalf("ExecutePreTrade() rejects len = %d, want 1", len(rejects))
	}
	if rejects[0].Policy != "mutation-reject" {
		t.Fatalf("reject policy = %q, want %q", rejects[0].Policy, "mutation-reject")
	}
}

func TestMutationsNativeE2E_DropCopyCommitsRejectingPolicyMutation(t *testing.T) {
	policy := &mutationTrackingPolicy{
		name:         "mutation-drop-copy",
		shouldReject: true,
		lockPrice:    mustOrderNativePrice(t, "13"),
		hasLockPrice: true,
	}
	engine := newEngineWithPreTradePolicyForNativeE2E(t, policy)
	defer engine.Stop()
	order := newValidOrderForNativeE2E(t)

	operation := requireAppliedDropCopy(t, engine, order)
	lock, err := operation.Lock()
	if err != nil {
		t.Fatalf("ApplyDropCopy() lock error = %v", err)
	}
	prices, err := lock.Prices()
	if err != nil {
		t.Fatalf("ApplyDropCopy() lock prices error = %v", err)
	}
	if len(prices) != 1 || prices[0].String() != "13" {
		t.Fatalf("ApplyDropCopy() lock prices = %v, want [13]", prices)
	}
	block, err := operation.AccountBlock()
	if err != nil {
		t.Fatalf("ApplyDropCopy() AccountBlock() error = %v", err)
	}
	if block == nil || block.Reason != "forced reject" {
		t.Fatalf("ApplyDropCopy() AccountBlock = %+v, want forced reject", block)
	}
	if block.Code != reject.CodeAccountBlocked {
		t.Fatalf(
			"ApplyDropCopy() AccountBlock code = %q, want %q",
			block.Code,
			reject.CodeAccountBlocked,
		)
	}
	blocked, err := operation.IsAccountBlocked()
	if err != nil {
		t.Fatalf("ApplyDropCopy() IsAccountBlocked() error = %v", err)
	}
	if !blocked {
		t.Fatal("ApplyDropCopy() IsAccountBlocked() = false, want true")
	}
	operation.CommitAndClose()
	if policy.commitCalls != 1 {
		t.Fatalf("commitCalls = %d, want 1", policy.commitCalls)
	}

	blockedReservation, blockedRejects, err := engine.ExecutePreTrade(
		newValidOrderForNativeE2E(t),
	)
	if err != nil {
		t.Fatalf("ExecutePreTrade() after drop-copy block error = %v", err)
	}
	if blockedReservation != nil {
		blockedReservation.Close()
		t.Fatal("ExecutePreTrade() after drop-copy block reservation != nil, want nil")
	}
	if len(blockedRejects) != 1 {
		t.Fatalf(
			"ExecutePreTrade() after drop-copy block rejects len = %d, want 1",
			len(blockedRejects),
		)
	}
	if blockedRejects[0].Reason != "forced reject" {
		t.Fatalf(
			"ExecutePreTrade() after drop-copy block reason = %q, want %q",
			blockedRejects[0].Reason,
			"forced reject",
		)
	}
}

func TestMutationsNativeE2E_DropCopyCommitIsIdempotent(t *testing.T) {
	policy := &mutationTrackingPolicy{
		name: "mutation-drop-copy-commit",
	}
	engine := newEngineWithPreTradePolicyForNativeE2E(t, policy)
	defer engine.Stop()

	operation := requireAppliedDropCopy(t, engine, newValidOrderForNativeE2E(t))
	operation.Commit()
	operation.Commit()
	operation.Rollback()
	if _, err := operation.AccountAdjustments(); err != nil {
		t.Fatalf("AccountAdjustments() after Commit() error = %v", err)
	}
	operation.Close()
	operation.Close()

	if policy.commitCalls != 1 {
		t.Fatalf("commitCalls = %d, want 1", policy.commitCalls)
	}
	if policy.rollbackCalls != 0 {
		t.Fatalf("rollbackCalls = %d, want 0", policy.rollbackCalls)
	}
}

func TestMutationsNativeE2E_DropCopyRollbackIsIdempotent(t *testing.T) {
	policy := &mutationTrackingPolicy{name: "mutation-drop-copy-rollback"}
	engine := newEngineWithPreTradePolicyForNativeE2E(t, policy)
	defer engine.Stop()

	operation := requireAppliedDropCopy(t, engine, newValidOrderForNativeE2E(t))
	operation.Rollback()
	operation.Rollback()
	operation.Commit()
	operation.Close()

	if policy.commitCalls != 0 {
		t.Fatalf("commitCalls = %d, want 0", policy.commitCalls)
	}
	if policy.rollbackCalls != 1 {
		t.Fatalf("rollbackCalls = %d, want 1", policy.rollbackCalls)
	}
}

func TestMutationsNativeE2E_DropCopyCloseRollsBackImplicitly(t *testing.T) {
	policy := &mutationTrackingPolicy{name: "mutation-drop-copy-close"}
	engine := newEngineWithPreTradePolicyForNativeE2E(t, policy)
	defer engine.Stop()

	operation := requireAppliedDropCopy(t, engine, newValidOrderForNativeE2E(t))
	operation.Close()
	operation.Close()
	if _, err := operation.AccountAdjustments(); !errors.Is(
		err, pretrade.ErrDropCopyOperationClosed,
	) {
		t.Fatalf(
			"AccountAdjustments() after Close() error = %v, want ErrDropCopyOperationClosed",
			err,
		)
	}

	if policy.commitCalls != 0 {
		t.Fatalf("commitCalls = %d, want 0", policy.commitCalls)
	}
	if policy.rollbackCalls != 1 {
		t.Fatalf("rollbackCalls = %d, want 1", policy.rollbackCalls)
	}
}

func TestMutationsNativeE2E_DropCopyStartMutationCommits(t *testing.T) {
	policy := &mutationTrackingPolicy{
		name:                  "mutation-drop-copy-start",
		registerStartMutation: true,
	}
	engine := newEngineWithPreTradePolicyForNativeE2E(t, policy)
	defer engine.Stop()

	requireAppliedDropCopy(t, engine, newValidOrderForNativeE2E(t)).CommitAndClose()
	if !policy.startSawDropCopy {
		t.Fatal("CheckPreTradeStart() IsDropCopy = false, want true")
	}
	if policy.startCommitCalls != 1 {
		t.Fatalf("startCommitCalls = %d, want 1", policy.startCommitCalls)
	}
	if policy.startRollbackCalls != 0 {
		t.Fatalf("startRollbackCalls = %d, want 0", policy.startRollbackCalls)
	}
}

func TestMutationsNativeE2E_DropCopyStartMutationRollsBackOnFatalReject(t *testing.T) {
	policy := &mutationTrackingPolicy{
		name:                  "mutation-drop-copy-start-fatal",
		registerStartMutation: true,
		startMissingField:     true,
	}
	engine := newEngineWithPreTradePolicyForNativeE2E(t, policy)
	defer engine.Stop()

	rejects := requireRejectedDropCopy(t, engine, newValidOrderForNativeE2E(t))
	if len(rejects) != 1 || rejects[0].Code != reject.CodeMissingRequiredField {
		t.Fatalf("ApplyDropCopy() rejects = %v, want missing field", rejects)
	}
	if policy.startCommitCalls != 0 {
		t.Fatalf("startCommitCalls = %d, want 0", policy.startCommitCalls)
	}
	if policy.startRollbackCalls != 1 {
		t.Fatalf("startRollbackCalls = %d, want 1", policy.startRollbackCalls)
	}
}

func TestMutationsNativeE2E_DropCopyPolicyPanicReturnsFatalReject(t *testing.T) {
	policy := &mutationTrackingPolicy{
		name:       "mutation-drop-copy-panic",
		panicValue: "go policy exploded",
	}
	engine := newEngineWithPreTradePolicyForNativeE2E(t, policy)
	defer engine.Stop()

	rejects := requireRejectedDropCopy(t, engine, newValidOrderForNativeE2E(t))
	if len(rejects) != 1 || rejects[0].Code != reject.CodeSystemUnavailable {
		t.Fatalf("ApplyDropCopy() rejects = %v, want SystemUnavailable", rejects)
	}
	if !strings.Contains(rejects[0].Details, policy.panicValue) {
		t.Fatalf(
			"reject details = %q, want panic value %q",
			rejects[0].Details,
			policy.panicValue,
		)
	}
	if policy.commitCalls != 0 {
		t.Fatalf("commitCalls = %d, want 0", policy.commitCalls)
	}
	if policy.rollbackCalls != 1 {
		t.Fatalf("rollbackCalls = %d, want 1", policy.rollbackCalls)
	}
}

// TestMutationsNativeE2E_DropCopyCommitPanicArmsKillSwitch pins the commit-side
// contract the operation shares with a reservation: a panicking commit callback
// is recovered at the language boundary and never crosses it, and the failure
// it reports is not discarded - it arms the engine kill switch, which an
// operator lifts with UnblockAll.
func TestMutationsNativeE2E_DropCopyCommitPanicArmsKillSwitch(t *testing.T) {
	policy := &mutationTrackingPolicy{
		name:             "mutation-drop-copy-commit-panic",
		commitPanicValue: "go mutation commit exploded",
	}
	engine := newEngineWithPreTradePolicyForNativeE2E(t, policy)
	defer engine.Stop()
	order := newValidOrderForNativeE2E(t)

	requireAppliedDropCopy(t, engine, order).CommitAndClose()
	if policy.commitCalls != 1 {
		t.Fatalf("commitCalls = %d, want 1", policy.commitCalls)
	}
	if policy.rollbackCalls != 0 {
		t.Fatalf("rollbackCalls = %d, want 0", policy.rollbackCalls)
	}

	assertMutationFinalizerBlock(t, engine, mutationOrderAccountID)

	engine.Accounts().UnblockAll()
	assertPreTradePassesForAccount(t, engine, mutationOrderAccountID)
}

func TestMutationsNativeE2E_DropCopyRollbackPanicFailsClosed(t *testing.T) {
	policy := &mutationTrackingPolicy{
		name:               "mutation-drop-copy-rollback-panic",
		shouldReject:       true,
		missingField:       true,
		rollbackPanicValue: "go mutation rollback exploded",
	}
	engine := newEngineWithPreTradePolicyForNativeE2E(t, policy)
	defer engine.Stop()
	order := newValidOrderForNativeE2E(t)

	rejects := requireRejectedDropCopy(t, engine, order)
	if len(rejects) != 2 || rejects[0].Code != reject.CodeMissingRequiredField ||
		rejects[1].Code != reject.CodeSystemUnavailable {
		t.Fatalf(
			"ApplyDropCopy() rejects = %v, want MissingRequiredField then SystemUnavailable",
			rejects,
		)
	}
	if !strings.Contains(rejects[1].Details, policy.rollbackPanicValue) {
		t.Fatalf(
			"reject details = %q, want panic value %q",
			rejects[1].Details,
			policy.rollbackPanicValue,
		)
	}
	if policy.rollbackCalls != 1 {
		t.Fatalf("rollbackCalls = %d, want 1", policy.rollbackCalls)
	}

	assertMutationFinalizerBlock(t, engine, mutationOrderAccountID)
}

// TestMutationsNativeE2E_DropCopyExplicitRollbackPanicArmsKillSwitch pins the
// caller-driven compensation contract the operation shares with a reservation:
// rollback stays void when its callback panics, the panic is recovered at the
// language boundary, and the reported failure arms the engine kill switch
// instead of being dropped.
func TestMutationsNativeE2E_DropCopyExplicitRollbackPanicArmsKillSwitch(t *testing.T) {
	policy := &mutationTrackingPolicy{
		name:               "mutation-drop-copy-explicit-rollback-panic",
		rollbackPanicValue: "go explicit rollback exploded",
	}
	engine := newEngineWithPreTradePolicyForNativeE2E(t, policy)
	defer engine.Stop()
	operation := requireAppliedDropCopy(t, engine, newValidOrderForNativeE2E(t))

	operation.Rollback()
	operation.Rollback()
	operation.Close()
	if policy.rollbackCalls != 1 {
		t.Fatalf("rollbackCalls = %d, want 1", policy.rollbackCalls)
	}

	assertMutationFinalizerBlock(t, engine, mutationOrderAccountID)

	engine.Accounts().UnblockAll()
	assertPreTradePassesForAccount(t, engine, mutationOrderAccountID)
}

func TestMutationsNativeE2E_DropCopyMissingFieldReturnsRejectAndRollsBack(t *testing.T) {
	policy := &mutationTrackingPolicy{
		name:         "mutation-drop-copy-missing",
		shouldReject: true,
		missingField: true,
	}
	engine := newEngineWithPreTradePolicyForNativeE2E(t, policy)
	defer engine.Stop()

	rejects := requireRejectedDropCopy(t, engine, newValidOrderForNativeE2E(t))
	if len(rejects) != 1 || rejects[0].Code != reject.CodeMissingRequiredField {
		t.Fatalf("ApplyDropCopy() rejects = %v, want missing field", rejects)
	}
	if policy.commitCalls != 0 {
		t.Fatalf("commitCalls = %d, want 0", policy.commitCalls)
	}
	if policy.rollbackCalls != 1 {
		t.Fatalf("rollbackCalls = %d, want 1", policy.rollbackCalls)
	}
}

func TestMutationsNativeE2E_DropCopyReportsAccountAdjustments(t *testing.T) {
	asset := mustOrderNativeAsset(t, "USD")
	policy := &mutationTrackingPolicy{
		name:            "mutation-drop-copy-adjustments",
		shouldReject:    true,
		adjustmentAsset: asset,
		hasAdjustment:   true,
		policyGroupID:   7,
	}
	engine := newEngineWithPreTradePolicyForNativeE2E(t, policy)
	defer engine.Stop()

	operation := requireAppliedDropCopy(t, engine, newValidOrderForNativeE2E(t))
	defer operation.Close()

	adjustments, err := operation.AccountAdjustments()
	if err != nil {
		t.Fatalf("AccountAdjustments() error = %v", err)
	}
	if len(adjustments) != 1 {
		t.Fatalf("AccountAdjustments() len = %d, want 1", len(adjustments))
	}
	if adjustments[0].PolicyGroupID != 7 {
		t.Fatalf("AccountAdjustments()[0].PolicyGroupID = %d, want 7", adjustments[0].PolicyGroupID)
	}
	if !adjustments[0].Entry.Asset.Equal(asset) {
		t.Fatalf(
			"AccountAdjustments()[0].Entry.Asset = %v, want %v",
			adjustments[0].Entry.Asset,
			asset,
		)
	}
}

// TestMutationsNativeE2E_DropCopyKeepsGatedBookkeepingUnderBusinessReject
// proves a policy that registers its mutation and account adjustment only when
// IsDropCopy is true keeps both under an ordinary reject in drop-copy, while
// the same reject in ordinary pre-trade registers neither.
func TestMutationsNativeE2E_DropCopyKeepsGatedBookkeepingUnderBusinessReject(t *testing.T) {
	asset := mustOrderNativeAsset(t, "EUR")
	policy := &mutationTrackingPolicy{
		name:            "mutation-drop-copy-gated",
		shouldReject:    true,
		adjustmentAsset: asset,
		hasAdjustment:   true,
		dropCopyGated:   true,
	}
	engine := newEngineWithPreTradePolicyForNativeE2E(t, policy)
	defer engine.Stop()

	reservation, rejects, err := engine.ExecutePreTrade(newValidOrderForNativeE2E(t))
	if err != nil {
		t.Fatalf("ExecutePreTrade() error = %v", err)
	}
	if reservation != nil {
		reservation.Close()
		t.Fatal("ExecutePreTrade() reservation != nil, want nil")
	}
	if len(rejects) != 1 || rejects[0].Code != reject.CodeOther {
		t.Fatalf("ExecutePreTrade() rejects = %v, want one %v", rejects, reject.CodeOther)
	}
	if !slices.Equal(policy.startDropCopyModes, []bool{false}) ||
		!slices.Equal(policy.mainDropCopyModes, []bool{false}) {
		t.Fatalf(
			"ExecutePreTrade() IsDropCopy start = %v, main = %v, want [false] and [false]",
			policy.startDropCopyModes,
			policy.mainDropCopyModes,
		)
	}
	if policy.commitCalls != 0 || policy.rollbackCalls != 0 {
		t.Fatalf(
			"ExecutePreTrade() commitCalls = %d, rollbackCalls = %d, want 0 and 0",
			policy.commitCalls,
			policy.rollbackCalls,
		)
	}

	operation := requireAppliedDropCopy(t, engine, newValidOrderForNativeE2E(t))
	if !slices.Equal(policy.startDropCopyModes, []bool{false, true}) ||
		!slices.Equal(policy.mainDropCopyModes, []bool{false, true}) {
		t.Fatalf(
			"ApplyDropCopy() IsDropCopy start = %v, main = %v, want [false true] and [false true]",
			policy.startDropCopyModes,
			policy.mainDropCopyModes,
		)
	}
	adjustments, err := operation.AccountAdjustments()
	if err != nil {
		t.Fatalf("AccountAdjustments() error = %v", err)
	}
	if len(adjustments) != 1 || !adjustments[0].Entry.Asset.Equal(asset) {
		t.Fatalf("AccountAdjustments() = %v, want one %v entry", adjustments, asset)
	}
	operation.CommitAndClose()
	if policy.commitCalls != 1 || policy.rollbackCalls != 0 {
		t.Fatalf(
			"commitCalls = %d, rollbackCalls = %d, want 1 and 0",
			policy.commitCalls,
			policy.rollbackCalls,
		)
	}
}

func TestMutationsNativeE2E_DropCopyMarketOrderRunsPriceIndependentPolicy(t *testing.T) {
	engine := newEngineWithPreTradePolicyForNativeE2E(
		t,
		&mutationTrackingPolicy{name: "must-not-run"},
	)
	defer engine.Stop()

	order := newValidOrderForNativeE2E(t)
	operationView := order.EnsureOperationView()
	operationView.UnsetPrice()
	requireAppliedDropCopy(t, engine, order).CommitAndClose()
}

func TestMutationsNativeE2E_DropCopyMissingAccountRejectsBeforePolicy(t *testing.T) {
	policy := &mutationTrackingPolicy{name: "must-not-run"}
	engine := newEngineWithPreTradePolicyForNativeE2E(t, policy)
	defer engine.Stop()

	order := newValidOrderForNativeE2E(t)
	operationView := order.EnsureOperationView()
	operationView.UnsetAccountID()
	rejects := requireRejectedDropCopy(t, engine, order)
	if len(rejects) != 1 || rejects[0].Code != reject.CodeMissingRequiredField {
		t.Fatalf("ApplyDropCopy() rejects = %v, want MissingRequiredField", rejects)
	}
	if policy.startSawDropCopy || policy.commitCalls != 0 || policy.rollbackCalls != 0 {
		t.Fatal("drop copy invoked a policy before rejecting the missing account")
	}
}

// TestMutationsNativeE2E_DropCopyCloseIsIdempotent proves repeated sequential
// Close calls on the same operation are safe. Concurrent Close on the same
// operation is out of contract: DropCopyOperation documents that callers must
// serialize methods on the same operation, so calling Close from multiple
// goroutines at once is a caller bug, not a property this binding guarantees.
func TestMutationsNativeE2E_DropCopyCloseIsIdempotent(t *testing.T) {
	engine := newEngineWithPreTradePolicyForNativeE2E(
		t,
		&mutationTrackingPolicy{name: "mutation-drop-copy-idempotent-close"},
	)
	defer engine.Stop()

	operation := requireAppliedDropCopy(t, engine, newValidOrderForNativeE2E(t))
	operation.Close()
	operation.Close() // must not panic or double-free

	if _, err := operation.AccountAdjustments(); !errors.Is(
		err, pretrade.ErrDropCopyOperationClosed,
	) {
		t.Fatalf(
			"AccountAdjustments() after repeated Close error = %v, want ErrDropCopyOperationClosed",
			err,
		)
	}
}

func TestMutationsNativeE2E_DropCopyRunsOnBlockedAccount(t *testing.T) {
	engine := newEngineWithPreTradePolicyForNativeE2E(
		t,
		&mutationTrackingPolicy{name: "mutation-drop-copy-blocked-account"},
	)
	defer engine.Stop()
	account := param.NewAccountIDFromUint64(1001)
	engine.Accounts().Block(account, "existing account block")

	operation := requireAppliedDropCopy(t, engine, newValidOrderForNativeE2E(t))
	defer operation.Close()
	block, err := operation.AccountBlock()
	if err != nil {
		t.Fatalf("AccountBlock() error = %v", err)
	}
	if block != nil {
		t.Fatal("AccountBlock() != nil, want no block requested by this call")
	}
	blocked, err := operation.IsAccountBlocked()
	if err != nil {
		t.Fatalf("IsAccountBlocked() error = %v", err)
	}
	if !blocked {
		t.Fatal("IsAccountBlocked() = false, want pre-existing blocked state")
	}
}

func TestMutationsNativeE2E_DropCopyRunsOnBlockedGroup(t *testing.T) {
	engine := newEngineWithPreTradePolicyForNativeE2E(
		t,
		&mutationTrackingPolicy{name: "mutation-drop-copy-blocked-group"},
	)
	defer engine.Stop()
	account := param.NewAccountIDFromUint64(1001)
	group, err := param.NewAccountGroupIDFromUint32(7)
	if err != nil {
		t.Fatalf("NewAccountGroupIDFromUint32() error = %v", err)
	}
	if err := engine.Accounts().RegisterGroup(
		[]param.AccountID{account},
		group,
	); err != nil {
		t.Fatalf("RegisterGroup() error = %v", err)
	}
	if err := engine.Accounts().BlockGroup(group, "existing group block"); err != nil {
		t.Fatalf("BlockGroup() error = %v", err)
	}

	operation := requireAppliedDropCopy(t, engine, newValidOrderForNativeE2E(t))
	defer operation.Close()
	block, err := operation.AccountBlock()
	if err != nil {
		t.Fatalf("AccountBlock() error = %v", err)
	}
	if block != nil {
		t.Fatal("AccountBlock() != nil, want no block requested by this call")
	}
	blocked, err := operation.IsAccountBlocked()
	if err != nil {
		t.Fatalf("IsAccountBlocked() error = %v", err)
	}
	if !blocked {
		t.Fatal("IsAccountBlocked() = false, want pre-existing group block state")
	}
}

type mutationTrackingPolicy struct {
	name                  string
	commitCalls           int
	rollbackCalls         int
	shouldReject          bool
	missingField          bool
	lockPrice             param.Price
	hasLockPrice          bool
	adjustmentAsset       param.Asset
	hasAdjustment         bool
	policyGroupID         model.PolicyGroupID
	registerStartMutation bool
	startMissingField     bool
	startSawDropCopy      bool
	startDropCopyModes    []bool
	mainDropCopyModes     []bool
	startCommitCalls      int
	startRollbackCalls    int
	panicValue            string
	commitPanicValue      string
	rollbackPanicValue    string
	// dropCopyGated registers the main-stage mutation and account adjustment
	// only in drop-copy, and scopes the forced reject to the order, so an
	// ordinary pre-trade leaves no state behind.
	dropCopyGated bool
}

func (mutationTrackingPolicy) Close() {}

func (p mutationTrackingPolicy) Name() string {
	return p.name
}

func (p mutationTrackingPolicy) PolicyGroupID() model.PolicyGroupID { return p.policyGroupID }

func (p *mutationTrackingPolicy) PerformPreTradeCheck(
	ctx pretrade.Context,
	_ model.Order,
	mutations tx.Mutations,
	result pretrade.Result,
) []reject.Reject {
	p.mainDropCopyModes = append(p.mainDropCopyModes, ctx.IsDropCopy())
	keepsBookkeeping := !p.dropCopyGated || ctx.IsDropCopy()
	if keepsBookkeeping {
		if err := mutations.Push(
			func() {
				p.commitCalls++
				if p.commitPanicValue != "" {
					panic(p.commitPanicValue)
				}
			},
			func() {
				p.rollbackCalls++
				if p.rollbackPanicValue != "" {
					panic(p.rollbackPanicValue)
				}
			},
		); err != nil {
			return reject.NewSingleItemList(
				reject.CodeOther,
				p.name,
				"mutation registration failed",
				err.Error(),
				reject.ScopeOrder,
			)
		}
	}
	if p.panicValue != "" {
		panic(p.panicValue)
	}
	if p.hasLockPrice {
		if err := result.PushLockPrice(p.lockPrice); err != nil {
			return reject.NewSingleItemList(
				reject.CodeOther,
				p.name,
				"lock price registration failed",
				err.Error(),
				reject.ScopeOrder,
			)
		}
	}
	if p.hasAdjustment && keepsBookkeeping {
		entry := accountadjustment.AccountOutcomeEntry{Asset: p.adjustmentAsset}
		if err := result.PushAccountAdjustment(entry); err != nil {
			return reject.NewSingleItemList(
				reject.CodeOther,
				p.name,
				"account adjustment registration failed",
				err.Error(),
				reject.ScopeOrder,
			)
		}
	}
	if p.shouldReject {
		code := reject.CodeOther
		if p.missingField {
			code = reject.CodeMissingRequiredField
		}
		scope := reject.ScopeAccount
		if p.dropCopyGated {
			scope = reject.ScopeOrder
		}
		return reject.NewSingleItemList(
			code,
			p.name,
			"forced reject",
			"forced in test",
			scope,
		)
	}
	return nil
}

func (p *mutationTrackingPolicy) CheckPreTradeStart(
	ctx pretrade.Context,
	_ model.Order,
) []reject.Reject {
	p.startSawDropCopy = ctx.IsDropCopy()
	p.startDropCopyModes = append(p.startDropCopyModes, ctx.IsDropCopy())
	if p.registerStartMutation {
		if err := ctx.RecordDropCopyStartMutation(
			func() { p.startCommitCalls++ },
			func() { p.startRollbackCalls++ },
		); err != nil {
			return reject.NewSingleItemList(
				reject.CodeSystemUnavailable,
				p.name,
				"start mutation registration failed",
				err.Error(),
				reject.ScopeOrder,
			)
		}
	}
	if p.startMissingField {
		return reject.NewSingleItemList(
			reject.CodeMissingRequiredField,
			p.name,
			"missing field",
			"forced in test",
			reject.ScopeOrder,
		)
	}
	return nil
}

func (mutationTrackingPolicy) ApplyExecutionReport(
	pretrade.PostTradeContext,
	model.ExecutionReport,
	pretrade.PostTradeAdjustments,
	pretrade.PostTradePnls,
) []reject.AccountBlock {
	return nil
}

func (mutationTrackingPolicy) ApplyAccountAdjustment(
	accountadjustment.Context,
	param.AccountID,
	model.AccountAdjustment,
	tx.Mutations,
	pretrade.AccountOutcomes,
) (pretrade.PolicyAccountAdjustmentResult, []reject.Reject) {
	return pretrade.PolicyAccountAdjustmentResult{}, nil
}

func newEngineWithPreTradePolicyForNativeE2E(
	t *testing.T,
	policy pretrade.Policy,
) *Engine {
	t.Helper()

	engine, err := NewEngineBuilder().FullSync().PreTrade(policy).Build()
	if err != nil {
		t.Fatalf("Build() error = %v", err)
	}
	return engine
}

func newValidOrderForNativeE2E(t *testing.T) model.Order {
	t.Helper()

	order := model.NewOrder()
	operation := order.EnsureOperationView()
	operation.SetInstrument(
		param.NewInstrument(mustOrderNativeAsset(t, "AAPL"), mustOrderNativeAsset(t, "USD")),
	)
	operation.SetAccountID(param.NewAccountIDFromUint64(mutationOrderAccountID))
	operation.SetSide(param.SideBuy)
	operation.SetTradeAmount(param.NewQuantityTradeAmount(mustOrderNativeQuantity(t, "1")))
	operation.SetPrice(mustOrderNativePrice(t, "100"))
	return order
}
