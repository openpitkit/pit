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

package marketdata_test

import (
	"errors"
	"testing"
	"time"

	"go.openpit.dev/openpit/marketdata"
	"go.openpit.dev/openpit/param"
	"go.openpit.dev/openpit/pkg/optional"
)

// Handle 0 of an AccountGroupID legally names the default ("everyone-else")
// bucket, so the service cannot tell an unset group from a deliberate one. These
// tests pin that the binding rejects the unset value before it reaches the
// service, and that param.DefaultAccountGroup keeps addressing that bucket.

type fixedGroupAccountInfo struct {
	group param.AccountGroupID
}

func (i fixedGroupAccountInfo) AccountGroup() optional.Option[param.AccountGroupID] {
	return optional.Some(i.group)
}

func newServiceWithoutQuote(t *testing.T) (*marketdata.Service, marketdata.InstrumentID) {
	t.Helper()
	service, err := marketdata.NewBuilder(marketdata.InfiniteTTL()).FullSync().Build()
	if err != nil {
		t.Fatalf("Build() error = %v", err)
	}
	t.Cleanup(service.Close)

	aapl, err := param.NewAsset("AAPL")
	if err != nil {
		t.Fatalf("NewAsset(AAPL) error = %v", err)
	}
	usd, err := param.NewAsset("USD")
	if err != nil {
		t.Fatalf("NewAsset(USD) error = %v", err)
	}
	id, err := service.Register(param.NewInstrument(aapl, usd))
	if err != nil {
		t.Fatalf("Register() error = %v", err)
	}
	return service, id
}

// newServiceWithAgedDefaultBucketQuote publishes an hour-old quote into the
// default bucket, so any finite TTL that reaches a group-less read expires it.
func newServiceWithAgedDefaultBucketQuote(
	t *testing.T,
) (*marketdata.Service, marketdata.InstrumentID) {
	t.Helper()
	service, id := newServiceWithoutQuote(t)
	mark, err := param.NewPriceFromString("150")
	if err != nil {
		t.Fatalf("NewPriceFromString() error = %v", err)
	}
	if err := service.Push(id, marketdata.NewQuote().WithMark(mark), time.Hour); err != nil {
		t.Fatalf("Push() error = %v", err)
	}
	return service, id
}

func getForGroupLessAccount(
	service *marketdata.Service,
	id marketdata.InstrumentID,
) error {
	_, err := service.Get(
		id,
		param.NewAccountIDFromUint64(7),
		noGroupAccountInfo{},
		marketdata.QuoteResolutionAccountThenGroupThenDefault,
	)
	return err
}

func TestPushForRejectsUninitializedAccountGroup(t *testing.T) {
	service, id := newServiceWithoutQuote(t)
	account := param.NewAccountIDFromUint64(7)
	mark, err := param.NewPriceFromString("150")
	if err != nil {
		t.Fatalf("NewPriceFromString() error = %v", err)
	}
	quote := marketdata.NewQuote().WithMark(mark)

	err = service.PushFor(
		id,
		quote,
		0,
		[]param.AccountID{account},
		[]param.AccountGroupID{param.DefaultAccountGroup, {}},
	)
	if !errors.Is(err, param.ErrUninitializedAccountGroupID) {
		t.Fatalf("PushFor() error = %v, want ErrUninitializedAccountGroupID", err)
	}
	// Nothing was published: neither the account bucket nor the default one.
	if err := getForGroupLessAccount(service, id); !errors.Is(err, marketdata.ErrQuoteUnavailable) {
		t.Fatalf("Get() after rejected PushFor error = %v, want ErrQuoteUnavailable", err)
	}

	if err := service.PushFor(
		id,
		quote,
		0,
		nil,
		[]param.AccountGroupID{param.DefaultAccountGroup},
	); err != nil {
		t.Fatalf("PushFor(DefaultAccountGroup) error = %v", err)
	}
	if err := getForGroupLessAccount(service, id); err != nil {
		t.Fatalf("Get() after PushFor(DefaultAccountGroup) error = %v", err)
	}
}

func TestAccountGroupTTLRejectsUninitializedAccountGroup(t *testing.T) {
	expiring := marketdata.WithinTTL(time.Second)

	tests := []struct {
		name  string
		set   func(*marketdata.Service, marketdata.InstrumentID, param.AccountGroupID) error
		clear func(*marketdata.Service, marketdata.InstrumentID, param.AccountGroupID) error
	}{
		{
			name: "service-wide",
			set: func(s *marketdata.Service, _ marketdata.InstrumentID, g param.AccountGroupID) error {
				return s.SetAccountGroupTTL(g, expiring)
			},
			clear: func(s *marketdata.Service, _ marketdata.InstrumentID, g param.AccountGroupID) error {
				return s.ClearAccountGroupTTL(g)
			},
		},
		{
			name: "per-instrument",
			set: func(s *marketdata.Service, id marketdata.InstrumentID, g param.AccountGroupID) error {
				return s.SetInstrumentAccountGroupTTL(id, g, expiring)
			},
			clear: func(s *marketdata.Service, id marketdata.InstrumentID, g param.AccountGroupID) error {
				return s.ClearInstrumentAccountGroupTTL(id, g)
			},
		},
	}
	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			service, id := newServiceWithAgedDefaultBucketQuote(t)

			err := test.set(service, id, param.AccountGroupID{})
			if !errors.Is(err, param.ErrUninitializedAccountGroupID) {
				t.Fatalf("set error = %v, want ErrUninitializedAccountGroupID", err)
			}
			if err := getForGroupLessAccount(service, id); err != nil {
				t.Fatalf("Get() after rejected set error = %v, want the quote", err)
			}

			if err := test.set(service, id, param.DefaultAccountGroup); err != nil {
				t.Fatalf("set(DefaultAccountGroup) error = %v", err)
			}
			if err := getForGroupLessAccount(service, id); !errors.Is(err, marketdata.ErrQuoteExpired) {
				t.Fatalf("Get() after set(DefaultAccountGroup) error = %v, want ErrQuoteExpired", err)
			}

			err = test.clear(service, id, param.AccountGroupID{})
			if !errors.Is(err, param.ErrUninitializedAccountGroupID) {
				t.Fatalf("clear error = %v, want ErrUninitializedAccountGroupID", err)
			}
			if err := getForGroupLessAccount(service, id); !errors.Is(err, marketdata.ErrQuoteExpired) {
				t.Fatalf("Get() after rejected clear error = %v, want ErrQuoteExpired", err)
			}

			if err := test.clear(service, id, param.DefaultAccountGroup); err != nil {
				t.Fatalf("clear(DefaultAccountGroup) error = %v", err)
			}
			if err := getForGroupLessAccount(service, id); err != nil {
				t.Fatalf("Get() after clear(DefaultAccountGroup) error = %v, want the quote", err)
			}
		})
	}
}

// TestGetFailsWhenAccountGroupIsUninitialized pins that an AccountInfo
// answering with an unset group fails the read instead of reading the default
// bucket, which would hand back a quote resolved for the wrong scope.
func TestGetFailsWhenAccountGroupIsUninitialized(t *testing.T) {
	service, id := newServiceWithDefaultBucketQuote(t)
	account := param.NewAccountIDFromUint64(7)
	unset := fixedGroupAccountInfo{}

	quote, err := service.Get(
		id,
		account,
		unset,
		marketdata.QuoteResolutionAccountThenGroupThenDefault,
	)
	if !errors.Is(err, marketdata.ErrAccountGroupResolution) {
		t.Fatalf("Get() err = %v, want ErrAccountGroupResolution", err)
	}
	if !errors.Is(err, param.ErrUninitializedAccountGroupID) {
		t.Fatalf("Get() err = %v, want ErrUninitializedAccountGroupID", err)
	}
	var resolutionErr *marketdata.AccountGroupResolutionError
	if !errors.As(err, &resolutionErr) || resolutionErr.Panic != nil {
		t.Fatalf("Get() err = %#v, want *AccountGroupResolutionError without a panic", err)
	}
	if quote.Mark().IsSet() {
		t.Fatal("Get() returned a default-bucket quote for an uninitialized account group")
	}
	if service.GetOptional(
		id,
		account,
		unset,
		marketdata.QuoteResolutionAccountThenGroupThenDefault,
	).IsSet() {
		t.Fatal("GetOptional() returned a quote for an uninitialized account group")
	}

	quote, err = service.Get(
		id,
		account,
		fixedGroupAccountInfo{group: param.DefaultAccountGroup},
		marketdata.QuoteResolutionAccountThenGroupThenDefault,
	)
	if err != nil {
		t.Fatalf("Get(DefaultAccountGroup) err = %v, want nil", err)
	}
	if mark, ok := quote.Mark().Get(); !ok || mark.String() != "150" {
		t.Fatalf("Get(DefaultAccountGroup) mark = %v, want the default-bucket 150", quote.Mark())
	}
}
