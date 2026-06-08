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

package config

import "strings"

// contains reports whether s contains substr.
func contains(s, substr string) bool { return strings.Contains(s, substr) }

// replaceLine swaps the first occurrence of old in fullConfig with replacement.
func replaceLine(old, replacement string) string {
	return strings.Replace(fullConfig, old, replacement, 1)
}

// removeLine drops the first line of fullConfig containing marker (the whole
// physical line).
func removeLine(marker string) string {
	lines := strings.Split(fullConfig, "\n")
	for i, ln := range lines {
		if strings.Contains(ln, marker) {
			return strings.Join(append(append([]string{}, lines[:i]...), lines[i+1:]...), "\n")
		}
	}
	return fullConfig
}

// dropLines removes every physical line of s that contains any of the given
// markers. Used to delete a whole config section (its header plus its keys).
func dropLines(s string, markers ...string) string {
	lines := strings.Split(s, "\n")
	out := make([]string, 0, len(lines))
	for _, ln := range lines {
		drop := false
		for _, m := range markers {
			if strings.Contains(ln, m) {
				drop = true
				break
			}
		}
		if !drop {
			out = append(out, ln)
		}
	}
	return strings.Join(out, "\n")
}

// replaceLines replaces two lines in fullConfig: the line containing oldLine1 is
// replaced with replacement1, and the line containing oldLine2 is replaced with
// replacement2. Used to mutate two related keys in one step (e.g. strategy +
// sharded_workers together).
func replaceLines(oldLine1, replacement1, oldLine2, replacement2 string) string {
	s := strings.Replace(fullConfig, oldLine1, replacement1, 1)
	return strings.Replace(s, oldLine2, replacement2, 1)
}

// removeSectionBlock removes the [cohort.<name>] header and the contiguous
// key=value lines beneath it (until the next blank line or section header).
func removeSectionBlock(ini, cohortName string) string {
	lines := strings.Split(ini, "\n")
	header := "[cohort." + cohortName + "]"
	out := make([]string, 0, len(lines))
	skipping := false
	for _, ln := range lines {
		trimmed := strings.TrimSpace(ln)
		if trimmed == header {
			skipping = true
			continue
		}
		if skipping {
			// Stop skipping at a blank line or a new section header.
			if trimmed == "" || strings.HasPrefix(trimmed, "[") {
				skipping = false
				// Re-emit a section header (it belongs to the next section).
				if strings.HasPrefix(trimmed, "[") {
					out = append(out, ln)
				}
			}
			continue
		}
		out = append(out, ln)
	}
	return strings.Join(out, "\n")
}
