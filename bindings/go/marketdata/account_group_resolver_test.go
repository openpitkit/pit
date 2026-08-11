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
	"strings"
	"testing"

	"go.openpit.dev/openpit/marketdata"
	"go.openpit.dev/openpit/param"
	"go.openpit.dev/openpit/pkg/optional"
)

const accountGroupPanicValue = "account group callback failed"

type panickingAccountInfo struct{}

func (panickingAccountInfo) AccountGroup() optional.Option[param.AccountGroupID] {
	panic(accountGroupPanicValue)
}

type noGroupAccountInfo struct{}

func (noGroupAccountInfo) AccountGroup() optional.Option[param.AccountGroupID] {
	return optional.None[param.AccountGroupID]()
}

// newServiceWithDefaultBucketQuote registers one instrument and publishes a
// quote into the default ("everyone-else") bucket, which is exactly the bucket
// a group-less account inherits.
func newServiceWithDefaultBucketQuote(
	t *testing.T,
) (*marketdata.Service, marketdata.InstrumentID) {
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
	mark, err := param.NewPriceFromString("150")
	if err != nil {
		t.Fatalf("NewPriceFromString() error = %v", err)
	}
	if err := service.Push(id, marketdata.NewQuote().WithMark(mark), 0); err != nil {
		t.Fatalf("Push() error = %v", err)
	}
	return service, id
}

// TestGetFailsWhenAccountGroupPanics pins the contract that a failing
// AccountInfo fails the read. Reporting the failure as "no group" would let the
// lookup fall through to the default bucket and hand back a quote resolved for
// the wrong scope, silently bypassing every group-scoped rule.
func TestGetFailsWhenAccountGroupPanics(t *testing.T) {
	service, id := newServiceWithDefaultBucketQuote(t)

	quote, err := service.Get(
		id,
		param.NewAccountIDFromUint64(7),
		panickingAccountInfo{},
		marketdata.QuoteResolutionAccountThenGroupThenDefault,
	)
	if !errors.Is(err, marketdata.ErrAccountGroupResolution) {
		t.Fatalf("Get() err = %v, want ErrAccountGroupResolution", err)
	}
	if quote.Mark().IsSet() {
		t.Fatal("Get() returned a default-bucket quote for an unresolved account group")
	}

	var resolutionErr *marketdata.AccountGroupResolutionError
	if !errors.As(err, &resolutionErr) {
		t.Fatalf("Get() err type = %T, want *AccountGroupResolutionError", err)
	}
	if !strings.Contains(resolutionErr.Error(), accountGroupPanicValue) {
		t.Fatalf(
			"error text = %q, want the panic value %q",
			resolutionErr.Error(),
			accountGroupPanicValue,
		)
	}
}

// TestGetFallsThroughForGroupLessAccount is the counterpart: a genuine "no
// group" answer still resolves through the default bucket.
func TestGetFallsThroughForGroupLessAccount(t *testing.T) {
	service, id := newServiceWithDefaultBucketQuote(t)

	quote, err := service.Get(
		id,
		param.NewAccountIDFromUint64(7),
		noGroupAccountInfo{},
		marketdata.QuoteResolutionAccountThenGroupThenDefault,
	)
	if err != nil {
		t.Fatalf("Get() err = %v, want nil", err)
	}
	mark, ok := quote.Mark().Get()
	if !ok {
		t.Fatal("Get() returned no mark for a group-less account")
	}
	if mark.String() != "150" {
		t.Fatalf("mark = %s, want 150", mark)
	}
}

func TestGetOptionalYieldsNoneWhenAccountGroupPanics(t *testing.T) {
	service, id := newServiceWithDefaultBucketQuote(t)

	quote := service.GetOptional(
		id,
		param.NewAccountIDFromUint64(7),
		panickingAccountInfo{},
		marketdata.QuoteResolutionAccountThenGroupThenDefault,
	)
	if quote.IsSet() {
		t.Fatal("GetOptional() returned a quote for an unresolved account group")
	}
}

func TestServiceCloseIsIdempotentAndMethodsDoNotUseReleasedHandle(t *testing.T) {
	service, id := newServiceWithDefaultBucketQuote(t)
	aapl, err := param.NewAsset("AAPL")
	if err != nil {
		t.Fatalf("NewAsset(AAPL) error = %v", err)
	}
	usd, err := param.NewAsset("USD")
	if err != nil {
		t.Fatalf("NewAsset(USD) error = %v", err)
	}
	instrument := param.NewInstrument(aapl, usd)
	account := param.NewAccountIDFromUint64(7)
	ttl := marketdata.InfiniteTTL()
	quote := marketdata.NewQuote()
	service.Close()
	service.Close()

	checks := map[string]func() error{
		"Register": func() error {
			_, err := service.Register(instrument)
			return err
		},
		"RegisterWithTTL": func() error {
			_, err := service.RegisterWithTTL(instrument, ttl)
			return err
		},
		"RegisterWithID": func() error {
			_, err := service.RegisterWithID(instrument, id)
			return err
		},
		"RegisterWithIDAndTTL": func() error {
			_, err := service.RegisterWithIDAndTTL(instrument, id, ttl)
			return err
		},
		"PushFor": func() error {
			return service.PushFor(id, quote, 0, []param.AccountID{account}, nil)
		},
		"SetInstrumentTTL":   func() error { return service.SetInstrumentTTL(id, ttl) },
		"ClearInstrumentTTL": func() error { return service.ClearInstrumentTTL(id) },
		"SetInstrumentAccountTTL": func() error {
			return service.SetInstrumentAccountTTL(id, account, ttl)
		},
		"ClearInstrumentAccountTTL": func() error {
			return service.ClearInstrumentAccountTTL(id, account)
		},
		"SetInstrumentAccountGroupTTL": func() error {
			return service.SetInstrumentAccountGroupTTL(id, param.DefaultAccountGroup, ttl)
		},
		"ClearInstrumentAccountGroupTTL": func() error {
			return service.ClearInstrumentAccountGroupTTL(id, param.DefaultAccountGroup)
		},
		"Push": func() error { return service.Push(id, quote, 0) },
		"PushByInstrument": func() error {
			_, err := service.PushByInstrument(instrument, quote, 0)
			return err
		},
		"Get": func() error {
			_, err := service.Get(
				id,
				account,
				noGroupAccountInfo{},
				marketdata.QuoteResolutionAccountThenGroupThenDefault,
			)
			return err
		},
	}
	for name, check := range checks {
		t.Run(name, func(t *testing.T) {
			if err := check(); !errors.Is(err, marketdata.ErrServiceClosed) {
				t.Fatalf("error = %v, want ErrServiceClosed", err)
			}
		})
	}

	service.SetAccountTTL(account, ttl)
	service.ClearAccountTTL(account)
	service.SetAccountGroupTTL(param.DefaultAccountGroup, ttl)
	service.ClearAccountGroupTTL(param.DefaultAccountGroup)
	service.Clear(id)
	if service.GetOptional(
		id,
		account,
		noGroupAccountInfo{},
		marketdata.QuoteResolutionAccountThenGroupThenDefault,
	).IsSet() {
		t.Fatal("GetOptional() after Close returned a quote")
	}
	if _, ok := service.Resolve(instrument); ok {
		t.Fatal("Resolve() after Close found an instrument")
	}
	// A nil *Service would only fail later, inside the first method call on it.
	clone, err := service.Clone()
	if !errors.Is(err, marketdata.ErrServiceClosed) {
		t.Fatalf("Clone() after Close error = %v, want ErrServiceClosed", err)
	}
	if clone != nil {
		clone.Close()
		t.Fatal("Clone() after Close returned a service, want nil")
	}
}

func TestServiceCloneSharesTheUnderlyingService(t *testing.T) {
	service, id := newServiceWithDefaultBucketQuote(t)

	clone, err := service.Clone()
	if err != nil {
		t.Fatalf("Clone() error = %v", err)
	}
	defer clone.Close()

	// Closing the original must leave the clone on the same live service.
	service.Close()
	if err := clone.Push(id, marketdata.NewQuote(), 0); err != nil {
		t.Fatalf("Push() on clone error = %v", err)
	}
}
