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
	"fmt"

	"go.openpit.dev/openpit/marketdata"
	"go.openpit.dev/openpit/param"
)

// MarketFeed wraps a live marketdata.Service and replays TICK rows against it.
//
// Each execution mode owns one feed over its own service: the runner registers
// every instrument that any TICK row mentions up front, then pushes quotes live
// at each TICK's row position. The feed also remembers the last price pushed per
// instrument so a FILL row may omit its price and reuse the latest quote as the
// lock price.
type MarketFeed struct {
	service *marketdata.Service
	ids     map[string]marketdata.InstrumentID
	latest  map[string]string
}

// NewMarketFeed wraps an already-built market-data service. The caller retains
// ownership of the service and is responsible for closing it.
func NewMarketFeed(service *marketdata.Service) *MarketFeed {
	return &MarketFeed{
		service: service,
		ids:     make(map[string]marketdata.InstrumentID),
		latest:  make(map[string]string),
	}
}

// RegisterInstruments registers every instrument named by a TICK row so that
// later live pushes for those instruments resolve to a known slot. Registration
// only creates the slot; quotes are published later by Push/PushFor.
func (m *MarketFeed) RegisterInstruments(rows []Row) error {
	for _, row := range rows {
		if row.Action != "TICK" {
			continue
		}
		if _, ok := m.ids[row.Instrument]; ok {
			continue
		}
		instrument, err := parseInstrument(row.Instrument)
		if err != nil {
			return fmt.Errorf("line %d: %w", row.Line, err)
		}
		id, err := m.service.Register(instrument)
		if err != nil {
			return fmt.Errorf(
				"line %d: register %s: %w", row.Line, row.Instrument, err,
			)
		}
		m.ids[row.Instrument] = id
	}
	return nil
}

// Push publishes a global mark-price snapshot for instrument, replacing the
// instrument-default quote that every account reads when it has no addressed
// quote of its own.
func (m *MarketFeed) Push(instrument, price string) error {
	id, quote, err := m.quote(instrument, price)
	if err != nil {
		return err
	}
	if err := m.service.Push(id, quote); err != nil {
		return err
	}
	m.latest[instrument] = price
	return nil
}

// PushFor publishes an addressed mark-price snapshot for instrument, replacing
// the stored quote for each listed account and account group only. At least one
// account or group must be supplied.
func (m *MarketFeed) PushFor(
	instrument, price string,
	accounts []param.AccountID,
	groups []param.AccountGroupID,
) error {
	id, quote, err := m.quote(instrument, price)
	if err != nil {
		return err
	}
	if err := m.service.PushFor(id, quote, accounts, groups); err != nil {
		return err
	}
	m.latest[instrument] = price
	return nil
}

// LatestPrice returns the last price string pushed for instrument, or "" when
// no TICK has been replayed for it yet. The runner uses it to fill in the lock
// price of a FILL row that omits its own price.
func (m *MarketFeed) LatestPrice(instrument string) string {
	return m.latest[instrument]
}

func (m *MarketFeed) quote(
	instrument, price string,
) (marketdata.InstrumentID, marketdata.Quote, error) {
	id, ok := m.ids[instrument]
	if !ok {
		return marketdata.InstrumentID{}, marketdata.Quote{}, fmt.Errorf(
			"instrument %s is not registered (every TICK instrument must "+
				"appear in the table)", instrument,
		)
	}
	mark, err := param.NewPriceFromString(price)
	if err != nil {
		return marketdata.InstrumentID{}, marketdata.Quote{}, fmt.Errorf(
			"price %q: %w", price, err,
		)
	}
	return id, marketdata.NewQuote().WithMark(mark), nil
}

func splitInstrument(s string) (base, quote string, err error) {
	i := -1
	for k, ch := range s {
		if ch == '/' {
			i = k
			break
		}
	}
	if i <= 0 || i == len(s)-1 {
		return "", "", fmt.Errorf(
			"instrument %q must be BASE/QUOTE", s,
		)
	}
	return s[:i], s[i+1:], nil
}
