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

package driver

import (
	"sync/atomic"

	"openpit-loadtest-spot-funds-go/internal/measurement"
)

// liveFn is a named type for the live-counter accessor function so that
// atomic.Pointer can hold a stable pointer to it.
type liveFn func() measurement.LiveCounters

// LiveSource is a race-safe bridge between the driver's internal sink and the
// progress reporter. It holds an atomic pointer to the live-counter accessor;
// Run stores the real accessor before starting any goroutine, so the progress
// reporter can call Counters() at any time.
//
// Before Run stores the accessor (i.e., before the LiveSource is passed to a
// Run call), Counters() returns zero-value LiveCounters rather than panicking.
type LiveSource struct {
	fn atomic.Pointer[liveFn]
}

// NewLiveSource allocates a LiveSource ready for use with Config.Live.
func NewLiveSource() *LiveSource {
	return &LiveSource{}
}

// store is called by Run exactly once, before goroutines start.
func (s *LiveSource) store(fn liveFn) {
	s.fn.Store(&fn)
}

// Counters returns the current live counters. Returns zero if Run has not yet
// stored the accessor (early progress ticks before the sink exists).
func (s *LiveSource) Counters() measurement.LiveCounters {
	p := s.fn.Load()
	if p == nil {
		return measurement.LiveCounters{}
	}
	return (*p)()
}
