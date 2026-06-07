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

package progress_test

import (
	"strings"
	"testing"
	"time"

	"openpit-loadtest-spot-funds-go/internal/measurement"
	"openpit-loadtest-spot-funds-go/internal/progress"
)

// TestFormatLineContainsElapsed verifies that the progress line contains the
// elapsed time field.
func TestFormatLineContainsElapsed(t *testing.T) {
	c := measurement.LiveCounters{
		Submitted: 5000,
		Decided:   4800,
		InFlight:  200,
	}
	line := progress.FormatLine(c, 100000, 10*time.Second)
	if !strings.Contains(line, "elapsed") {
		t.Errorf("progress line missing 'elapsed': %q", line)
	}
}

// TestFormatLineContainsDecided verifies that the decided count appears.
func TestFormatLineContainsDecided(t *testing.T) {
	c := measurement.LiveCounters{
		Submitted: 5000,
		Decided:   4800,
		InFlight:  200,
	}
	line := progress.FormatLine(c, 100000, 10*time.Second)
	if !strings.Contains(line, "decided") {
		t.Errorf("progress line missing 'decided': %q", line)
	}
}

// TestFormatLineContainsInFlight verifies that in-flight depth appears.
func TestFormatLineContainsInFlight(t *testing.T) {
	c := measurement.LiveCounters{
		Submitted: 5000,
		Decided:   4800,
		InFlight:  200,
	}
	line := progress.FormatLine(c, 100000, 10*time.Second)
	if !strings.Contains(line, "in-flight") {
		t.Errorf("progress line missing 'in-flight': %q", line)
	}
	if !strings.Contains(line, "200") {
		t.Errorf("progress line must show in-flight count 200: %q", line)
	}
}

// TestFormatLineContainsRate verifies that the offered rate appears.
func TestFormatLineContainsRate(t *testing.T) {
	c := measurement.LiveCounters{
		Submitted: 50000,
		Decided:   50000,
		InFlight:  0,
	}
	line := progress.FormatLine(c, 100000, 1*time.Second)
	if !strings.Contains(line, "rate") {
		t.Errorf("progress line missing 'rate': %q", line)
	}
}

// TestFormatLineContainsPercent verifies percentage progress is shown when
// total is known.
func TestFormatLineContainsPercent(t *testing.T) {
	c := measurement.LiveCounters{
		Submitted: 50000,
		Decided:   50000,
		InFlight:  0,
	}
	line := progress.FormatLine(c, 100000, 1*time.Second)
	if !strings.Contains(line, "%") {
		t.Errorf("progress line missing percentage: %q", line)
	}
}

// TestFormatLineZeroTotal verifies that unknown total shows "?" for remaining.
func TestFormatLineZeroTotal(t *testing.T) {
	c := measurement.LiveCounters{
		Submitted: 1000,
		Decided:   1000,
		InFlight:  0,
	}
	line := progress.FormatLine(c, 0, 2*time.Second)
	if !strings.Contains(line, "?") {
		t.Errorf("with total=0 the line should show ? for remaining: %q", line)
	}
}

// TestFormatLineZeroDecided verifies no division-by-zero when decided=0.
func TestFormatLineZeroDecided(t *testing.T) {
	c := measurement.LiveCounters{
		Submitted: 0,
		Decided:   0,
		InFlight:  0,
	}
	// Must not panic.
	line := progress.FormatLine(c, 100000, 0)
	if line == "" {
		t.Error("progress line must not be empty even with zero counters")
	}
}

// TestFormatLineDurationRender verifies that elapsed duration renders compactly.
func TestFormatLineDurationRender(t *testing.T) {
	c := measurement.LiveCounters{Decided: 1000}
	// 2 minutes 5 seconds
	line := progress.FormatLine(c, 10000, 2*time.Minute+5*time.Second)
	if !strings.Contains(line, "2m") {
		t.Errorf("elapsed 2m05s should show '2m': %q", line)
	}
}
