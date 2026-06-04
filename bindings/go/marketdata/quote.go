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

package marketdata

import (
	"time"

	"go.openpit.dev/openpit/internal/native"
	"go.openpit.dev/openpit/param"
	"go.openpit.dev/openpit/pkg/optional"
)

//------------------------------------------------------------------------------
// Quote

// Quote is a market snapshot. Every field is optional: an unset field means the
// producer did not publish that field.
type Quote struct{ value native.MarketDataQuote }

// NewQuote creates a new empty quote with every field unset.
func NewQuote() Quote {
	return newQuoteFromHandle(native.CreateMarketDataQuote())
}

func newQuoteFromHandle(value native.MarketDataQuote) Quote {
	return Quote{value: value}
}

// WithMark returns a copy of the quote with the mark price set.
func (q Quote) WithMark(mark param.Price) Quote {
	native.MarketDataQuoteSetMark(&q.value, mark.Handle())
	return q
}

// WithBid returns a copy of the quote with the best-bid price set.
func (q Quote) WithBid(bid param.Price) Quote {
	native.MarketDataQuoteSetBid(&q.value, bid.Handle())
	return q
}

// WithAsk returns a copy of the quote with the best-ask price set.
func (q Quote) WithAsk(ask param.Price) Quote {
	native.MarketDataQuoteSetAsk(&q.value, ask.Handle())
	return q
}

// Mark returns the optional mark price.
func (q Quote) Mark() optional.Option[param.Price] {
	return param.NewPriceOptionFromHandle(native.MarketDataQuoteGetMark(q.value))
}

// Bid returns the optional best-bid price.
func (q Quote) Bid() optional.Option[param.Price] {
	return param.NewPriceOptionFromHandle(native.MarketDataQuoteGetBid(q.value))
}

// Ask returns the optional best-ask price.
func (q Quote) Ask() optional.Option[param.Price] {
	return param.NewPriceOptionFromHandle(native.MarketDataQuoteGetAsk(q.value))
}

// Handle returns the underlying native quote value.
func (q Quote) Handle() native.MarketDataQuote {
	return q.value
}

//------------------------------------------------------------------------------
// QuoteResolution

// QuoteResolution controls how [Service.Get] resolves a quote for a specific
// account.
type QuoteResolution = native.MarketDataQuoteResolution

const (
	// QuoteResolutionAccountOnly consults only the per-account bucket; no
	// fallback is performed when the account bucket has no quote.
	QuoteResolutionAccountOnly QuoteResolution = native.MarketDataQuoteResolutionAccountOnly
	// QuoteResolutionAccountThenGroup consults the per-account bucket, then the
	// account's group bucket when the account bucket has no quote.
	QuoteResolutionAccountThenGroup QuoteResolution = native.MarketDataQuoteResolutionAccountThenGroup
	// QuoteResolutionAccountThenGroupThenDefault consults the per-account
	// bucket, then the account's group bucket, then the default account-group
	// ("everyone-else") bucket, in that order. Each next bucket is consulted
	// only when the previous one has no quote.
	QuoteResolutionAccountThenGroupThenDefault QuoteResolution = native.MarketDataQuoteResolutionAccountThenGroupThenDefault
)

//------------------------------------------------------------------------------
// QuoteTTL

// QuoteTTL is a service-wide or per-instrument quote lifetime. An infinite TTL
// means quotes never expire on their own; a finite TTL expires a quote after
// the configured duration following the push that wrote it.
type QuoteTTL struct{ value native.MarketDataQuoteTTL }

// InfiniteTTL returns a quote lifetime under which quotes never expire on their
// own.
func InfiniteTTL() QuoteTTL {
	return newQuoteTTLFromHandle(native.CreateMarketDataQuoteTTLInfinite())
}

// WithinTTL returns a finite quote lifetime of the given duration.
func WithinTTL(d time.Duration) QuoteTTL {
	secs := uint64(d / time.Second)  //nolint:gosec // d is a positive TTL duration
	nanos := uint32(d % time.Second) //nolint:gosec // remainder fits in nanoseconds [0, 1e9)
	return newQuoteTTLFromHandle(native.CreateMarketDataQuoteTTLWithin(secs, nanos))
}

func newQuoteTTLFromHandle(value native.MarketDataQuoteTTL) QuoteTTL {
	return QuoteTTL{value: value}
}

// Handle returns the underlying native TTL value.
func (t QuoteTTL) Handle() native.MarketDataQuoteTTL {
	return t.value
}
