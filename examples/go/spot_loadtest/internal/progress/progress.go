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

// Package progress writes live run progress to an io.Writer (typically
// os.Stderr) at a configurable tick interval. It reads counters from the
// driver's LiveSource via race-safe atomics and NEVER writes to the same
// stream as the final report (stdout). The separation is guaranteed by the
// caller passing os.Stderr here and os.Stdout to the reporter.
//
// Each tick overwrites the previous line using a carriage-return prefix so the
// terminal stays clean. The final stop clears the line so no progress noise
// remains in the terminal after the report prints.
package progress

import (
	"fmt"
	"io"
	"time"

	"openpit-loadtest-spot-funds-go/internal/driver"
	"openpit-loadtest-spot-funds-go/internal/measurement"
)

// DefaultInterval is the default tick interval used when the caller passes zero.
const DefaultInterval = 500 * time.Millisecond

// Scale factors for compact count/rate rendering.
const (
	scaleKilo = 1_000.0
	scaleMega = 1_000_000.0
)

// pctScale converts a fraction in [0,1] to a percentage.
const pctScale = 100.0

// clearLine is written on Stop() to erase the progress line from the terminal.
// It uses CR + spaces (wider than typical terminal width) + CR to position the
// cursor back at column zero, leaving no visible progress noise.
const clearLine = "\r                                                                              \r"

// Reporter writes periodic live progress to w.
type Reporter struct {
	w        io.Writer
	src      *driver.LiveSource
	total    uint64
	interval time.Duration
	done     chan struct{}
}

// New creates a Reporter that writes to w every interval. total is the target
// number of decided ops (order-checks + settlements) used to estimate progress
// and the remaining wall time; pass 0 when unknown. interval <= 0 uses 500 ms.
func New(w io.Writer, src *driver.LiveSource, total uint64, interval time.Duration) *Reporter {
	if interval <= 0 {
		interval = DefaultInterval
	}
	return &Reporter{
		w:        w,
		src:      src,
		total:    total,
		interval: interval,
		done:     make(chan struct{}),
	}
}

// Start begins the progress loop in a background goroutine. Call Stop when the
// run completes to clean up and erase the progress line.
func (r *Reporter) Start(start time.Time) {
	go r.loop(start)
}

// Stop signals the progress goroutine to exit and erases the current progress
// line from the terminal. It blocks until the goroutine has stopped.
func (r *Reporter) Stop() {
	close(r.done)
	// Erase the progress line so no progress noise remains before the report.
	_, _ = fmt.Fprint(r.w, clearLine)
}

// loop is the background goroutine.
func (r *Reporter) loop(start time.Time) {
	tick := time.NewTicker(r.interval)
	defer tick.Stop()
	for {
		select {
		case <-r.done:
			return
		case now := <-tick.C:
			r.render(now, start)
		}
	}
}

// render writes one progress line using a carriage-return prefix to overwrite
// the previous line in place. It reads counters via the race-safe LiveSource.
func (r *Reporter) render(now, start time.Time) {
	c := r.src.Counters()
	elapsed := now.Sub(start)
	line := FormatLine(c, r.total, elapsed)
	// "\r" returns to column 0 without advancing to the next line, so this tick
	// overwrites the previous tick's output on supporting terminals.
	_, _ = fmt.Fprintf(r.w, "\r%s", line)
}

// FormatLine builds one progress line from the provided counters, total, and
// elapsed time. Exported so tests can verify formatting without spawning a
// goroutine.
func FormatLine(c measurement.LiveCounters, total uint64, elapsed time.Duration) string {
	elapsedStr := fmtDuration(elapsed)

	// Estimated remaining from decided / total progress.
	remainStr := "?"
	if total > 0 && c.Decided > 0 && c.Decided <= total {
		fraction := float64(c.Decided) / float64(total)
		if fraction > 0 {
			totalSec := elapsed.Seconds() / fraction
			remainSec := totalSec - elapsed.Seconds()
			if remainSec > 0 {
				remain := time.Duration(remainSec * float64(time.Second))
				remainStr = fmtDuration(remain)
			} else {
				remainStr = "0s"
			}
		}
	}

	// Offered rate = decided / elapsed.
	rateStr := "?"
	if elapsed.Seconds() > 0 && c.Decided > 0 {
		rate := float64(c.Decided) / elapsed.Seconds()
		rateStr = fmtRate(rate)
	}

	// Progress percentage.
	pctStr := "?%"
	if total > 0 {
		pct := float64(c.Decided) * pctScale / float64(total)
		pctStr = fmt.Sprintf("%.1f%%", pct)
	}

	return fmt.Sprintf(
		"elapsed %s | remaining ~%s | %s decided | in-flight %d | rate %s | %s",
		elapsedStr, remainStr, fmtCount(c.Decided), c.InFlight, rateStr, pctStr,
	)
}

// fmtDuration renders a duration compactly (e.g. "1h23m", "4m05s", "12s").
func fmtDuration(d time.Duration) string {
	d = d.Round(time.Second)
	if d < 0 {
		d = 0
	}
	h := d / time.Hour
	d -= h * time.Hour
	m := d / time.Minute
	d -= m * time.Minute
	s := d / time.Second
	switch {
	case h > 0:
		return fmt.Sprintf("%dh%02dm", h, m)
	case m > 0:
		return fmt.Sprintf("%dm%02ds", m, s)
	default:
		return fmt.Sprintf("%ds", s)
	}
}

// fmtCount renders a large integer compactly (e.g. "1.2M", "45K", "999").
func fmtCount(n uint64) string {
	switch {
	case n >= uint64(scaleMega):
		return fmt.Sprintf("%.1fM", float64(n)/scaleMega)
	case n >= uint64(scaleKilo):
		return fmt.Sprintf("%.1fK", float64(n)/scaleKilo)
	default:
		return fmt.Sprintf("%d", n)
	}
}

// fmtRate renders an ops/s rate compactly (e.g. "50.0K/s", "1.2M/s").
func fmtRate(r float64) string {
	switch {
	case r >= scaleMega:
		return fmt.Sprintf("%.1fM/s", r/scaleMega)
	case r >= scaleKilo:
		return fmt.Sprintf("%.1fK/s", r/scaleKilo)
	default:
		return fmt.Sprintf("%.0f/s", r)
	}
}
