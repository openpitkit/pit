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

package pretrade

import (
	"errors"
	"testing"

	"go.openpit.dev/openpit/internal/native"
	"go.openpit.dev/openpit/model"
	"go.openpit.dev/openpit/param"
)

func TestRequestExecuteReturnsErrorOnSecondCall(t *testing.T) {
	engine := newNativeEngineForPreTradeTests(t)

	requestHandle, rejects, err := native.EngineStartPreTrade(
		engine,
		newValidOrderForPreTradeTests(t).Handle(),
	)
	if err != nil {
		t.Fatalf("EngineStartPreTrade() error = %v", err)
	}
	if rejects != nil {
		native.DestroyPretradeRejectList(rejects)
		t.Fatalf("EngineStartPreTrade() rejects = %v, want nil", rejects)
	}

	request := NewRequestFromHandle(requestHandle)
	defer request.Close()

	reservation, executeRejects, err := request.Execute()
	if err != nil {
		t.Fatalf("Execute() first error = %v", err)
	}
	if executeRejects != nil {
		t.Fatalf("Execute() first rejects = %v, want nil", executeRejects)
	}
	if reservation == nil {
		t.Fatal("Execute() first reservation = nil, want non-nil")
	}
	reservation.Close()

	secondReservation, secondRejects, err := request.Execute()
	if secondReservation != nil {
		secondReservation.Close()
		t.Fatal("Execute() second reservation != nil, want nil")
	}
	if secondRejects != nil {
		t.Fatalf("Execute() second rejects = %v, want nil", secondRejects)
	}
	if err == nil {
		t.Fatal("Execute() second error = nil, want non-nil")
	}
}

func TestRequestExecuteAfterCloseReturnsError(t *testing.T) {
	engine := newNativeEngineForPreTradeTests(t)

	requestHandle, rejects, err := native.EngineStartPreTrade(
		engine,
		newValidOrderForPreTradeTests(t).Handle(),
	)
	if err != nil {
		t.Fatalf("EngineStartPreTrade() error = %v", err)
	}
	if rejects != nil {
		native.DestroyPretradeRejectList(rejects)
		t.Fatalf("EngineStartPreTrade() rejects = %v, want nil", rejects)
	}

	request := NewRequestFromHandle(requestHandle)
	request.Close()

	reservation, executeRejects, err := request.Execute()
	if reservation != nil {
		reservation.Close()
		t.Fatal("Execute() reservation != nil, want nil")
	}
	if executeRejects != nil {
		t.Fatalf("Execute() rejects = %v, want nil", executeRejects)
	}
	if !errors.Is(err, ErrRequestClosed) {
		t.Fatalf("Execute() error = %v, want ErrRequestClosed", err)
	}
}

func TestReservationCommit(t *testing.T) {
	reservation := newReservationForPreTradeTests(t, newValidOrderForPreTradeTests(t))
	reservation.Commit()
	reservation.Close()
}

func TestReservationCommitAndCloseAllowsSubsequentCommit(t *testing.T) {
	reservation := newReservationForPreTradeTests(t, newValidOrderForPreTradeTests(t))
	reservation.CommitAndClose()
	reservation.Commit()
}

func TestReservationRollback(t *testing.T) {
	reservation := newReservationForPreTradeTests(t, newValidOrderForPreTradeTests(t))
	reservation.Rollback()
	reservation.Close()
}

func TestReservationRollbackAndCloseAllowsSubsequentRollback(t *testing.T) {
	reservation := newReservationForPreTradeTests(t, newValidOrderForPreTradeTests(t))
	reservation.RollbackAndClose()
	reservation.Rollback()
}

func TestReservationCloseIsIdempotent(t *testing.T) {
	reservation := newReservationForPreTradeTests(t, newValidOrderForPreTradeTests(t))
	reservation.Close()
	reservation.Close()
	if reservation.handle != nil {
		t.Fatal("Reservation handle remains after repeated Close calls")
	}
}

// TestReservationCommitAfterRollbackIsNoOp pins the resolve-once contract on
// the sequential path: the first of Commit/Rollback wins and the second call
// does nothing rather than re-entering the native finalizer.
func TestReservationCommitAfterRollbackIsNoOp(t *testing.T) {
	reservation := newReservationForPreTradeTests(t, newValidOrderForPreTradeTests(t))
	defer reservation.Close()

	reservation.Rollback()
	reservation.Commit()
	if !reservation.resolved {
		t.Fatal("reservation not resolved after Rollback")
	}
}

func TestDropCopyOperationCommit(t *testing.T) {
	operation := newDropCopyOperationForPreTradeTests(t)
	operation.Commit()
	operation.Close()
}

func TestDropCopyOperationCommitAndCloseAllowsSubsequentCommit(t *testing.T) {
	operation := newDropCopyOperationForPreTradeTests(t)
	operation.CommitAndClose()
	operation.Commit()
}

func TestDropCopyOperationRollback(t *testing.T) {
	operation := newDropCopyOperationForPreTradeTests(t)
	operation.Rollback()
	operation.Close()
}

func TestDropCopyOperationRollbackAndCloseAllowsSubsequentRollback(t *testing.T) {
	operation := newDropCopyOperationForPreTradeTests(t)
	operation.RollbackAndClose()
	operation.Rollback()
}

func TestDropCopyOperationCloseIsIdempotent(t *testing.T) {
	operation := newDropCopyOperationForPreTradeTests(t)
	operation.Close()
	operation.Close()
	if operation.handle != nil {
		t.Fatal("DropCopyOperation handle remains after repeated Close calls")
	}
}

// TestDropCopyOperationCommitAfterRollbackIsNoOp pins the resolve-once contract
// on the sequential path: the first of Commit/Rollback wins and the second call
// does nothing rather than re-entering the native finalizer.
func TestDropCopyOperationCommitAfterRollbackIsNoOp(t *testing.T) {
	operation := newDropCopyOperationForPreTradeTests(t)
	defer operation.Close()

	operation.Rollback()
	operation.Commit()
	if !operation.resolved {
		t.Fatal("operation not resolved after Rollback")
	}
}

// TestDropCopyOperationAccessorsAfterCloseReportClosed releases a real native
// operation and then checks that every accessor returns an explicit lifecycle
// error.
func TestDropCopyOperationAccessorsAfterCloseReportClosed(t *testing.T) {
	operation := newDropCopyOperationForPreTradeTests(t)
	operation.Close()

	if _, err := operation.Lock(); !errors.Is(err, ErrDropCopyOperationClosed) {
		t.Fatalf("Lock() after Close error = %v, want ErrDropCopyOperationClosed", err)
	}
	if _, err := operation.AccountAdjustments(); !errors.Is(err, ErrDropCopyOperationClosed) {
		t.Fatalf("AccountAdjustments() after Close error = %v, want ErrDropCopyOperationClosed", err)
	}
	if _, err := operation.AccountBlock(); !errors.Is(err, ErrDropCopyOperationClosed) {
		t.Fatalf("AccountBlock() after Close error = %v, want ErrDropCopyOperationClosed", err)
	}
	if _, err := operation.IsAccountBlocked(); !errors.Is(err, ErrDropCopyOperationClosed) {
		t.Fatalf("IsAccountBlocked() after Close error = %v, want ErrDropCopyOperationClosed", err)
	}
}

// TestDropCopyOperationAccessorsOnLiveHandleAnswer proves the sentinel marks a
// released handle rather than an empty result.
func TestDropCopyOperationAccessorsOnLiveHandleAnswer(t *testing.T) {
	operation := newDropCopyOperationForPreTradeTests(t)

	if _, err := operation.AccountAdjustments(); err != nil {
		t.Fatalf("AccountAdjustments() on live operation error = %v", err)
	}
	if _, err := operation.IsAccountBlocked(); err != nil {
		t.Fatalf("IsAccountBlocked() on live operation error = %v", err)
	}
}

// TestReservationAccessorsAfterCloseReportClosed pins the same contract on the
// reservation.
func TestReservationAccessorsAfterCloseReportClosed(t *testing.T) {
	reservation := newReservationForPreTradeTests(t, newValidOrderForPreTradeTests(t))
	reservation.Close()

	if _, err := reservation.Lock(); !errors.Is(err, ErrReservationClosed) {
		t.Fatalf("Lock() after Close error = %v, want ErrReservationClosed", err)
	}
	if _, err := reservation.AccountAdjustments(); !errors.Is(err, ErrReservationClosed) {
		t.Fatalf("AccountAdjustments() after Close error = %v, want ErrReservationClosed", err)
	}
}

// TestReservationAccessorsOnLiveHandleAnswer proves the sentinel marks a
// released handle rather than an empty result.
func TestReservationAccessorsOnLiveHandleAnswer(t *testing.T) {
	reservation := newReservationForPreTradeTests(t, newValidOrderForPreTradeTests(t))

	if _, err := reservation.AccountAdjustments(); err != nil {
		t.Fatalf("AccountAdjustments() on live reservation error = %v", err)
	}
}

func TestRequestCloseIsIdempotent(t *testing.T) {
	engine := newNativeEngineForPreTradeTests(t)
	requestHandle, rejects, err := native.EngineStartPreTrade(
		engine,
		newValidOrderForPreTradeTests(t).Handle(),
	)
	if err != nil {
		t.Fatalf("EngineStartPreTrade() error = %v", err)
	}
	if rejects != nil {
		native.DestroyPretradeRejectList(rejects)
		t.Fatalf("EngineStartPreTrade() rejects = %v, want nil", rejects)
	}
	request := NewRequestFromHandle(requestHandle)

	request.Close()
	request.Close()
	if request.handle != nil {
		t.Fatal("Request handle remains after repeated Close calls")
	}
}

func TestReservationLockOnFreshReservationProducesNonZeroBlob(t *testing.T) {
	reservation := newReservationForPreTradeTests(t, newValidOrderForPreTradeTests(t))
	lock, err := reservation.Lock()
	if err != nil {
		t.Fatalf("Reservation.Lock() error = %v", err)
	}
	if len(lock.Bytes()) == 0 {
		t.Fatal("Reservation.Lock().Bytes() is empty, want canonical empty-lock blob")
	}
}

func TestLockMsgPackRoundTripPreservesIdentity(t *testing.T) {
	reservation := newReservationForPreTradeTests(t, newValidOrderForPreTradeTests(t))
	original, err := reservation.Lock()
	if err != nil {
		t.Fatalf("Reservation.Lock() error = %v", err)
	}

	msgpackBlob, err := original.MarshalMsgpack()
	if err != nil {
		t.Fatalf("Lock.MarshalMsgpack() error = %v", err)
	}
	restored, err := NewLockFromMsgPack(msgpackBlob)
	if err != nil {
		t.Fatalf("NewLockFromMsgPack() error = %v", err)
	}
	if !restored.Equal(original) {
		t.Fatal("restored lock != original lock after msgpack round-trip")
	}
}

func TestLockPricesExposeListOnly(t *testing.T) {
	price := mustPriceForPreTradeTests(t, "185")
	other := mustPriceForPreTradeTests(t, "186")
	lock := newLockForPreTradeTests(t, model.DefaultPolicyGroupID, price, other)

	prices, err := lock.Prices()
	if err != nil {
		t.Fatalf("Lock.Prices() error = %v", err)
	}
	if len(prices) != 2 {
		t.Fatalf("len(Lock.Prices()) = %d, want 2", len(prices))
	}
	if !prices[0].Equal(price) || !prices[1].Equal(other) {
		t.Fatalf("Lock.Prices() = [%s, %s], want [%s, %s]", prices[0], prices[1], price, other)
	}

	missing, err := lock.PricesOf(model.PolicyGroupID(99))
	if err != nil {
		t.Fatalf("Lock.PricesOf(missing) error = %v", err)
	}
	if len(missing) != 0 {
		t.Fatalf("len(Lock.PricesOf(missing)) = %d, want 0", len(missing))
	}

	single := newLockForPreTradeTests(t, model.DefaultPolicyGroupID, price)
	singlePrices, err := single.Prices()
	if err != nil {
		t.Fatalf("Lock.Prices(single) error = %v", err)
	}
	if len(singlePrices) != 1 {
		t.Fatalf("len(Lock.Prices(single)) = %d, want 1", len(singlePrices))
	}
	if !singlePrices[0].Equal(price) {
		t.Fatalf("Lock.Prices(single)[0] = %s, want %s", singlePrices[0], price)
	}
}

func TestLockLen(t *testing.T) {
	price := mustPriceForPreTradeTests(t, "185")
	lock := newLockForPreTradeTests(t, model.DefaultPolicyGroupID, price, price)

	got, err := lock.Len()
	if err != nil {
		t.Fatalf("Lock.Len() error = %v", err)
	}
	if got != 2 {
		t.Fatalf("Lock.Len() = %d, want 2", got)
	}
}

func newNativeEngineForPreTradeTests(t *testing.T) native.Engine {
	t.Helper()

	builder, err := native.CreateEngineBuilder(native.SyncPolicyFull)
	if err != nil {
		t.Fatalf("CreateEngineBuilder() error = %v", err)
	}
	if err := native.EngineBuilderAddBuiltinOrderValidation(builder, native.DefaultPolicyGroupID); err != nil {
		native.DestroyEngineBuilder(builder)
		t.Fatalf("EngineBuilderAddBuiltinOrderValidation() error = %v", err)
	}
	engine, _, err := native.EngineBuilderBuild(builder)
	native.DestroyEngineBuilder(builder)
	if err != nil {
		t.Fatalf("EngineBuilderBuild() error = %v", err)
	}
	t.Cleanup(func() { native.DestroyEngine(engine) })
	return engine
}

func newReservationForPreTradeTests(t *testing.T, order model.Order) *Reservation {
	t.Helper()

	engine := newNativeEngineForPreTradeTests(t)
	reservationHandle, rejects, err := native.EngineExecutePreTrade(engine, order.Handle())
	if err != nil {
		t.Fatalf("EngineExecutePreTrade() error = %v", err)
	}
	if rejects != nil {
		native.DestroyPretradeRejectList(rejects)
		t.Fatalf("EngineExecutePreTrade() rejects = %v, want nil", rejects)
	}

	reservation := NewReservationFromHandle(reservationHandle)
	t.Cleanup(reservation.Close)
	return reservation
}

func newDropCopyOperationForPreTradeTests(t *testing.T) *DropCopyOperation {
	t.Helper()

	engine := newNativeEngineForPreTradeTests(t)
	operationHandle, rejects, err := native.EngineApplyDropCopy(
		engine,
		newValidOrderForPreTradeTests(t).Handle(),
	)
	if err != nil {
		t.Fatalf("EngineApplyDropCopy() error = %v", err)
	}
	if rejects != nil {
		native.DestroyPretradeRejectList(rejects)
		t.Fatalf("EngineApplyDropCopy() rejects = %v, want nil", rejects)
	}

	operation := NewDropCopyOperationFromHandle(operationHandle)
	t.Cleanup(operation.Close)
	return operation
}

func mustPriceForPreTradeTests(t *testing.T, value string) param.Price {
	t.Helper()

	price, err := param.NewPriceFromString(value)
	if err != nil {
		t.Fatalf("NewPriceFromString(%q) error = %v", value, err)
	}
	return price
}

func newLockForPreTradeTests(t *testing.T, groupID model.PolicyGroupID, prices ...param.Price) Lock {
	t.Helper()

	handle := native.CreatePretradePreTradeLock()
	defer native.DestroyPretradePreTradeLock(handle)
	for _, price := range prices {
		if err := native.PretradePreTradeLockPush(
			handle,
			native.PolicyGroupID(groupID),
			price.Handle(),
		); err != nil {
			t.Fatalf("PretradePreTradeLockPush() error = %v", err)
		}
	}
	return newLockFromHandle(handle)
}

func mustQuantityForPreTradeTests(t *testing.T, value string) param.Quantity {
	t.Helper()

	quantity, err := param.NewQuantityFromString(value)
	if err != nil {
		t.Fatalf("NewQuantityFromString(%q) error = %v", value, err)
	}
	return quantity
}

func mustAssetForPreTradeTests(t *testing.T, value string) param.Asset {
	t.Helper()

	asset, err := param.NewAsset(value)
	if err != nil {
		t.Fatalf("NewAsset(%q) error = %v", value, err)
	}
	return asset
}

func newValidOrderForPreTradeTests(t *testing.T) model.Order {
	t.Helper()

	order := model.NewOrder()
	operation := order.EnsureOperationView()
	operation.SetInstrument(
		param.NewInstrument(mustAssetForPreTradeTests(t, "AAPL"), mustAssetForPreTradeTests(t, "USD")),
	)
	operation.SetAccountID(param.NewAccountIDFromUint64(1001))
	operation.SetSide(param.SideBuy)
	operation.SetTradeAmount(param.NewQuantityTradeAmount(mustQuantityForPreTradeTests(t, "1")))
	operation.SetPrice(mustPriceForPreTradeTests(t, "100"))
	return order
}
