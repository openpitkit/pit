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

package pretrade

import (
	"testing"

	"go.openpit.dev/openpit/internal/native"
	"go.openpit.dev/openpit/model"
	"go.openpit.dev/openpit/param"
)

func TestRequestExecuteReturnsErrorOnSecondCall(t *testing.T) {
	engine := newNativeEngineForPreTradeTests(t)

	requestHandle, rejects, err := native.EngineStartPreTrade(
		engine,
		newValidOrderForPreTradeTests(t).Native(),
	)
	if err != nil {
		t.Fatalf("EngineStartPreTrade() error = %v", err)
	}
	if rejects != nil {
		native.DestroyRejectList(rejects)
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

func TestReservationCommit(t *testing.T) {
	reservation := newReservationForPreTradeTests(t, newValidOrderForPreTradeTests(t))
	reservation.Commit()
	reservation.Close()
}

func TestReservationCommitAndClosePanicsOnClosedReservation(t *testing.T) {
	reservation := newReservationForPreTradeTests(t, newValidOrderForPreTradeTests(t))
	reservation.CommitAndClose()

	didPanic := false
	func() {
		defer func() {
			if recover() != nil {
				didPanic = true
			}
		}()
		reservation.Commit()
	}()
	if !didPanic {
		t.Fatal("Commit() panic = nil, want non-nil")
	}
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

func TestReservationLockReturnsPrice(t *testing.T) {
	price := mustPriceForPreTradeTests(t, "125")
	nativeLock := native.NewPretradePreTradeLock()
	native.PretradePreTradeLockSetPrice(&nativeLock, price.Native())
	lock := newLock(nativeLock)

	lockPrice, ok := lock.Price().Get()
	if !ok {
		t.Fatal("Lock().Price().Get() ok = false, want true")
	}
	if !lockPrice.Equal(price) {
		t.Fatalf("Lock().Price() = %q, want %q", lockPrice.String(), price.String())
	}
}

func TestReservationLockOnFreshReservationReturnsUnsetPrice(t *testing.T) {
	reservation := newReservationForPreTradeTests(t, newValidOrderForPreTradeTests(t))
	lock := reservation.Lock()
	if lock.Price().IsSet() {
		t.Fatal("Reservation.Lock().Price().IsSet() = true, want false")
	}
}

func newNativeEngineForPreTradeTests(t *testing.T) native.Engine {
	t.Helper()

	builder := native.CreateEngineBuilder()
	engine, err := native.EngineBuilderBuild(builder)
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
	reservationHandle, rejects, err := native.EngineExecutePreTrade(engine, order.Native())
	if err != nil {
		t.Fatalf("EngineExecutePreTrade() error = %v", err)
	}
	if rejects != nil {
		native.DestroyRejectList(rejects)
		t.Fatalf("EngineExecutePreTrade() rejects = %v, want nil", rejects)
	}

	reservation := NewReservationFromHandle(reservationHandle)
	t.Cleanup(reservation.Close)
	return reservation
}

func mustPriceForPreTradeTests(t *testing.T, value string) param.Price {
	t.Helper()

	price, err := param.NewPriceFromString(value)
	if err != nil {
		t.Fatalf("NewPriceFromString(%q) error = %v", value, err)
	}
	return price
}

func mustQuantityForPreTradeTests(t *testing.T, value string) param.Quantity {
	t.Helper()

	quantity, err := param.NewQuantityFromString(value)
	if err != nil {
		t.Fatalf("NewQuantityFromString(%q) error = %v", value, err)
	}
	return quantity
}

func newValidOrderForPreTradeTests(t *testing.T) model.Order {
	t.Helper()

	order := model.NewOrder()
	operation := order.EnsureOperationView()
	operation.SetInstrument(param.NewInstrument(param.NewAsset("AAPL"), param.NewAsset("USD")))
	operation.SetAccountID(param.NewAccountIDFromInt(1001))
	operation.SetSide(param.SideBuy)
	operation.SetTradeAmount(param.NewQuantityTradeAmount(mustQuantityForPreTradeTests(t, "1")))
	operation.SetPrice(mustPriceForPreTradeTests(t, "100"))
	return order
}
