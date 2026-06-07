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
	"time"

	"go.openpit.dev/openpit/asyncengine"
	"go.openpit.dev/openpit/param"

	"openpit-loadtest-spot-funds-go/internal/measurement"
)

// metricsObserver is the Phase-4 implementation of asyncengine.Observer. It
// records queue-wait and engine-compute durations into the measurement
// ObserverSink and tracks queue lifecycle counts. All callbacks run on worker
// or submitter goroutines and must be cheap and thread-safe; the ObserverSink
// uses a mutex-guarded HdrHistogram for the durations and atomics for the
// counters, satisfying both requirements.
//
// These are per-account AGGREGATE diagnostics (not correlated to a specific
// order), kept strictly separate from the headline latency streams.
type metricsObserver struct {
	sink *measurement.ObserverSink
}

// newObserver returns the observer to install (and its sink), or nil when
// disabled. A nil return makes buildEngine skip WithObserver.
func newObserver(enabled bool) (*metricsObserver, *measurement.ObserverSink) {
	if !enabled {
		return nil, nil
	}
	sink := measurement.NewObserverSink()
	return &metricsObserver{sink: sink}, sink
}

func (*metricsObserver) OnEnqueue(param.AccountID, int) {}

// OnDequeue records the time a task spent waiting in the queue.
func (o *metricsObserver) OnDequeue(_ param.AccountID, waited time.Duration) {
	o.sink.RecordDequeue(waited)
}

// OnComplete records the wall-clock duration of the engine call.
func (o *metricsObserver) OnComplete(_ param.AccountID, ran time.Duration) {
	o.sink.RecordComplete(ran)
}

func (*metricsObserver) OnSlowSubmit(param.AccountID, time.Duration, int) {}

func (*metricsObserver) OnQueueFullBlocked(param.AccountID, time.Duration) {}

// OnQueueCreated increments the diagnostic queue-created counter.
func (o *metricsObserver) OnQueueCreated(_ param.AccountID, _ int) {
	o.sink.RecordQueueCreated()
}

// OnQueueRemoved increments the diagnostic queue-removed counter.
func (o *metricsObserver) OnQueueRemoved(_ param.AccountID, _ int) {
	o.sink.RecordQueueRemoved()
}

func (*metricsObserver) OnSubmitCancelled(param.AccountID, error) {}

// asObserver adapts the concrete observer to the interface, returning a nil
// interface (not a typed-nil) when disabled so buildEngine's nil check works.
func (o *metricsObserver) asObserver() asyncengine.Observer {
	if o == nil {
		return nil
	}
	return o
}
