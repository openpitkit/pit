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

package measurement

import (
	"sync"
	"sync/atomic"
	"time"

	hdrhistogram "github.com/HdrHistogram/hdrhistogram-go"
)

// ObserverSink accumulates inner-metrics from the asyncengine Observer callbacks
// into SEPARATE diagnostic histograms that are kept strictly out of the headline
// latency streams.
//
// Observer callbacks run on worker or submitter goroutines; they must be cheap
// and thread-safe. ObserverSink satisfies that by recording into mutex-guarded
// HdrHistograms (same bounds as the headline streams) and maintaining lock-free
// atomic counters for the queue lifecycle events.
//
// Important caveat: these are per-account AGGREGATE distributions, not
// correlated to a specific order. OnDequeue/waited is the time a task spent
// queued across ALL accounts; OnComplete/ran is the engine compute time across
// ALL accounts. Reporting them as decomposition of the headline requires
// windowed-aggregate arithmetic, not per-op subtraction.
type ObserverSink struct {
	mu sync.Mutex
	// queueWait records the queue-wait duration from OnDequeue.
	queueWait *hdrhistogram.Histogram
	// engineCompute records the engine compute duration from OnComplete.
	engineCompute *hdrhistogram.Histogram

	// queuesCreated / queuesRemoved are diagnostic counters for queue lifecycle.
	queuesCreated atomic.Int64
	queuesRemoved atomic.Int64
	// dequeues / completes are raw callback counts.
	dequeues  atomic.Int64
	completes atomic.Int64
	// clamped counts observer samples (queue-wait + engine-compute) saturated to
	// the histogram ceiling by recordClamped rather than dropped. Guarded by mu.
	clamped int64
}

// NewObserverSink allocates an ObserverSink with standard histogram bounds.
func NewObserverSink() *ObserverSink {
	return &ObserverSink{
		queueWait:     newHist(),
		engineCompute: newHist(),
	}
}

// RecordDequeue records one OnDequeue callback (queue-wait duration).
func (o *ObserverSink) RecordDequeue(waited time.Duration) {
	ns := toNs(waited)
	o.mu.Lock()
	if recordClamped(o.queueWait, ns) {
		o.clamped++
	}
	o.mu.Unlock()
	o.dequeues.Add(1)
}

// RecordComplete records one OnComplete callback (engine compute duration).
// Aborted tasks report ran = 0; those are recorded as 1 ns to keep them
// inside the histogram range while marking them as near-zero.
func (o *ObserverSink) RecordComplete(ran time.Duration) {
	ns := toNs(ran)
	o.mu.Lock()
	if recordClamped(o.engineCompute, ns) {
		o.clamped++
	}
	o.mu.Unlock()
	o.completes.Add(1)
}

// RecordQueueCreated increments the queue-created counter.
func (o *ObserverSink) RecordQueueCreated() {
	o.queuesCreated.Add(1)
}

// RecordQueueRemoved increments the queue-removed counter.
func (o *ObserverSink) RecordQueueRemoved() {
	o.queuesRemoved.Add(1)
}

// InnerMetrics is the immutable diagnostic snapshot of the observer inner metrics.
type InnerMetrics struct {
	// QueueWait is the aggregate queue-wait distribution from OnDequeue.
	QueueWait Percentiles
	// EngineCompute is the aggregate engine-compute distribution from OnComplete.
	EngineCompute Percentiles
	// QueuesCreated / QueuesRemoved are lifetime queue lifecycle counts.
	QueuesCreated int64
	QueuesRemoved int64
	// Dequeues / Completes are raw callback counts (may differ for aborted tasks).
	Dequeues  int64
	Completes int64
	// Clamped is the number of observer samples saturated to the histogram
	// ceiling rather than dropped.
	Clamped int64
}

// Snapshot returns an immutable copy of the inner metrics. Call after the run
// has fully drained.
func (o *ObserverSink) Snapshot() InnerMetrics {
	o.mu.Lock()
	qw := extract(o.queueWait)
	ec := extract(o.engineCompute)
	clamped := o.clamped
	o.mu.Unlock()
	return InnerMetrics{
		QueueWait:     qw,
		EngineCompute: ec,
		QueuesCreated: o.queuesCreated.Load(),
		QueuesRemoved: o.queuesRemoved.Load(),
		Dequeues:      o.dequeues.Load(),
		Completes:     o.completes.Load(),
		Clamped:       clamped,
	}
}
