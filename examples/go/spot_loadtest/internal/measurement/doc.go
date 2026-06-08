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

// Package measurement provides HdrHistogram-backed windows, result-sink
// accumulation, harness-overhead characterisation, and observer inner metrics
// for the spot-limit load test.
//
// # Histogram bounds and out-of-range clamping
//
// Every latency histogram uses [1 µs .. 60 s] at 3 significant figures.
// The lower bound excludes sub-microsecond values that time.Duration cannot
// represent reliably and that the Go FFI path never achieves in practice.
// The 60 s upper bound is a safe ceiling for any hung operation.
//
// The vendored hdrhistogram-go records NOTHING when a value exceeds the
// histogram's range: RecordValue returns an error and leaves the count
// unchanged. Discarding that error would SILENTLY DROP exactly the
// coordinated-omission upper tail. To prevent that, every record goes through
// recordClamped, which clamps the value to the histogram's highest trackable
// value BEFORE recording: an over-range sample SATURATES at the ceiling and its
// COUNT is preserved (only the exact magnitude above the ceiling is lost). The
// number of clamped samples is accumulated and surfaced in the report
// (Snapshot.ClampedSamples, "clamped samples (> hist max): N"); when non-zero,
// the report notes that the upper tail saturated and the tail percentiles at or
// above the ceiling are a lower bound on the true latency.
//
// Methodology invariant: latencies above the histogram ceiling are clamped
// (saturated), never dropped; the clamped count is reported.
//
// # Windowing
//
// Windows are sized by operation count (default; reproducible boundaries) or
// by wall-clock duration (config.WindowUnitWall). Each window holds its own
// order-check and settlement histograms. A merged histogram accumulates the
// full run for the headline percentiles.
//
// # Coordinated-omission note
//
// The latency values arriving from the driver are already CO-correct: the
// driver stamps intended_t0 *before* the non-blocking submit and computes
// latency = resolve_time - intended_t0, so any queue-wait or submit backpressure
// is inside the measured interval. This package does not recompute CO.
//
// # Observer inner metrics (diagnostic, separate from headline)
//
// ObserverSink records queue_wait and engine_compute durations reported by the
// asyncengine Observer callbacks. These are per-account *aggregate* distributions
// (not correlated to a specific order): the dispatcher reports OnDequeue/waited
// and OnComplete/ran for every task on every account, summed over all accounts.
// They belong exclusively to the diagnostic snapshot section, never the headline.
// Wire only when config.Run.Observer = true.
//
// # Anti-dead-code-elimination checksum
//
// Every recorded decision (accept/reject + a stable counter field) is folded
// into a running checksum so the compiler/runtime cannot elide the work. The
// checksum is printed in the report as proof of work.
package measurement
