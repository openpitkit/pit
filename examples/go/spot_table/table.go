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
	"bufio"
	"fmt"
	"io"
	"os"
	"strconv"
	"strings"
)

// Frontmatter holds the per-file configuration block.
type Frontmatter struct {
	Name        string
	SlippageBps uint16
}

// Row is one parsed table row. Empty fields mean "not applicable to this
// action"; per-action validation enforces which cells each action requires
// or forbids.
type Row struct {
	Line       int
	Step       string
	Account    string
	Action     string
	Instrument string
	Side       string
	Qty        string
	Volume     string
	Price      string
	Asset      string
	Amount     string
	Fee        string
	Pnl        string
	Group      string
	Expect     string
	Reject     string
	Note       string
}

// Table is the parsed file.
type Table struct {
	FM   Frontmatter
	Rows []Row
}

// requiredHeaders lists the column headers a table must declare, in any
// order. Every other recognized column is optional and read by name when
// present; per-action validation then enforces the cells each action needs.
var requiredHeaders = []string{"account", "action", "expect"}

// Scanner buffer sizes, large enough for wide pipe-tables with many columns.
const (
	scanInitBuffer = 64 * 1024
	scanMaxBuffer  = 1024 * 1024
)

// ParseFile reads and parses a table file.
func ParseFile(path string) (*Table, error) {
	f, err := os.Open(path) // #nosec G304 -- example CLI opens a user-named scenario file
	if err != nil {
		return nil, err
	}
	defer func() { _ = f.Close() }()
	return Parse(f, path)
}

// Parse parses the table from r. name is used in error messages.
func Parse(r io.Reader, name string) (*Table, error) {
	scanner := bufio.NewScanner(r)
	scanner.Buffer(make([]byte, 0, scanInitBuffer), scanMaxBuffer)

	t := &Table{}
	lineNo := 0
	state := stateStart
	var headers []string

	for scanner.Scan() {
		lineNo++
		raw := scanner.Text()
		trimmed := strings.TrimSpace(raw)

		switch state {
		case stateStart:
			if trimmed == "---" {
				state = stateFM
				continue
			}
			if isTableRow(trimmed) {
				headers = splitRow(trimmed)
				state = stateAwaitDivider
				continue
			}
			// other text - skip

		case stateFM:
			if trimmed == "---" {
				state = stateBody
				continue
			}
			if err := parseFMLine(&t.FM, trimmed, lineNo, name); err != nil {
				return nil, err
			}

		case stateBody:
			if isTableRow(trimmed) {
				headers = splitRow(trimmed)
				state = stateAwaitDivider
			}

		case stateAwaitDivider:
			if !isDividerRow(trimmed) {
				return nil, fmt.Errorf(
					"%s:%d: expected table divider after header, got %q",
					name, lineNo, trimmed,
				)
			}
			if err := checkHeaders(headers); err != nil {
				return nil, fmt.Errorf("%s:%d: %w", name, lineNo-1, err)
			}
			state = stateRows

		case stateRows:
			if !isTableRow(trimmed) {
				// table ended; stop reading rows but keep scanning for
				// additional tables would be straightforward - v1 takes
				// only the first table block.
				state = stateDone
				continue
			}
			fields := splitRow(trimmed)
			row, err := buildRow(fields, headers, lineNo)
			if err != nil {
				return nil, fmt.Errorf("%s:%d: %w", name, lineNo, err)
			}
			t.Rows = append(t.Rows, row)

		case stateDone:
			// ignore trailing prose
		}
	}
	if err := scanner.Err(); err != nil {
		return nil, fmt.Errorf("%s: read: %w", name, err)
	}
	if state != stateRows && state != stateDone {
		return nil, fmt.Errorf("%s: no table found", name)
	}
	if len(t.Rows) == 0 {
		return nil, fmt.Errorf("%s: table has no rows", name)
	}
	return t, nil
}

type parseState int

const (
	stateStart parseState = iota
	stateFM
	stateBody
	stateAwaitDivider
	stateRows
	stateDone
)

func parseFMLine(fm *Frontmatter, line string, lineNo int, name string) error {
	if line == "" || strings.HasPrefix(line, "#") {
		return nil
	}
	i := strings.IndexByte(line, ':')
	if i < 0 {
		return fmt.Errorf("%s:%d: front-matter expects key: value, got %q",
			name, lineNo, line)
	}
	key := strings.TrimSpace(line[:i])
	value := strings.TrimSpace(line[i+1:])
	switch key {
	case "name":
		fm.Name = value
	case "slippage_bps":
		n, err := strconv.ParseUint(value, 10, 16)
		if err != nil {
			return fmt.Errorf("%s:%d: slippage_bps: %w", name, lineNo, err)
		}
		fm.SlippageBps = uint16(n)
	default:
		return fmt.Errorf("%s:%d: unknown front-matter key %q",
			name, lineNo, key)
	}
	return nil
}

func isTableRow(s string) bool {
	return strings.HasPrefix(s, "|") && strings.HasSuffix(s, "|")
}

func isDividerRow(s string) bool {
	if !isTableRow(s) {
		return false
	}
	for _, ch := range s {
		switch ch {
		case '|', '-', ':', ' ', '\t':
			// ok
		default:
			return false
		}
	}
	return true
}

func splitRow(s string) []string {
	inner := strings.TrimPrefix(strings.TrimSuffix(s, "|"), "|")
	parts := strings.Split(inner, "|")
	out := make([]string, len(parts))
	for i, p := range parts {
		out[i] = strings.TrimSpace(p)
	}
	return out
}

// checkHeaders verifies that every required column is present. Column order is
// free and additional columns are tolerated; the parser reads every cell by
// header name.
func checkHeaders(got []string) error {
	for _, want := range requiredHeaders {
		if !hasHeader(got, want) {
			return fmt.Errorf(
				"missing required column %q (required: %s)",
				want, strings.Join(requiredHeaders, ","),
			)
		}
	}
	return nil
}

func hasHeader(headers []string, name string) bool {
	for _, h := range headers {
		if strings.EqualFold(h, name) {
			return true
		}
	}
	return false
}

func buildRow(fields []string, headers []string, lineNo int) (Row, error) {
	cell := func(name string) string {
		for i, h := range headers {
			if strings.EqualFold(h, name) {
				if i < len(fields) {
					return fields[i]
				}
				return ""
			}
		}
		return ""
	}
	row := Row{
		Line:       lineNo,
		Step:       cell("#"),
		Account:    cell("account"),
		Action:     strings.ToUpper(cell("action")),
		Instrument: cell("instrument"),
		Side:       strings.ToUpper(cell("side")),
		Qty:        cell("qty"),
		Volume:     cell("volume"),
		Price:      cell("price"),
		Asset:      cell("asset"),
		Amount:     cell("amount"),
		Fee:        cell("fee"),
		Pnl:        cell("pnl"),
		Group:      cell("group"),
		Expect:     strings.ToUpper(cell("expect")),
		Reject:     cell("reject"),
		Note:       cell("note"),
	}
	if err := validateRow(row); err != nil {
		return Row{}, err
	}
	return row, nil
}

// validateRow enforces the per-action required and forbidden cells. It runs at
// parse time so the runner can assume well-formed rows.
func validateRow(row Row) error {
	switch row.Action {
	case "SEED":
		return validateSeed(row)
	case "TICK":
		return validateTick(row)
	case "ORDER":
		return validateOrder(row)
	case "FILL":
		return validateFill(row)
	case "GROUP":
		return validateGroup(row)
	default:
		return fmt.Errorf("unknown action %q", row.Action)
	}
}

func validateSeed(row Row) error {
	if err := requireExpect(row, "SEED", "OK", "REJECT"); err != nil {
		return err
	}
	if row.Account == "" {
		return fmt.Errorf("SEED requires account")
	}
	if row.Asset == "" || row.Amount == "" {
		return fmt.Errorf("SEED requires asset and amount")
	}
	return forbid("SEED", cellForbid{
		"instrument": row.Instrument, "side": row.Side, "qty": row.Qty,
		"volume": row.Volume, "price": row.Price, "group": row.Group,
	})
}

func validateTick(row Row) error {
	if err := requireExpect(row, "TICK", "OK"); err != nil {
		return err
	}
	if row.Instrument == "" || row.Price == "" {
		return fmt.Errorf("TICK requires instrument and price")
	}
	// account and group are optional: empty = global push, set = addressed push.
	return forbid("TICK", cellForbid{
		"side": row.Side, "qty": row.Qty, "volume": row.Volume,
		"asset": row.Asset, "amount": row.Amount, "fee": row.Fee,
		"pnl": row.Pnl, "reject": row.Reject,
	})
}

func validateOrder(row Row) error {
	if err := requireExpect(row, "ORDER", "ACCEPT", "REJECT"); err != nil {
		return err
	}
	if row.Account == "" {
		return fmt.Errorf("ORDER requires account")
	}
	if row.Instrument == "" || row.Side == "" {
		return fmt.Errorf("ORDER requires instrument and side")
	}
	hasQty := row.Qty != ""
	hasVolume := row.Volume != ""
	switch {
	case hasQty && hasVolume:
		return fmt.Errorf("ORDER must set exactly one of qty or volume, not both")
	case !hasQty && !hasVolume:
		return fmt.Errorf("ORDER must set exactly one of qty or volume")
	}
	if row.Expect != "REJECT" && row.Reject != "" {
		return fmt.Errorf("ORDER reject code is only valid with expect REJECT")
	}
	return forbid("ORDER", cellForbid{
		"asset": row.Asset, "amount": row.Amount, "fee": row.Fee,
		"pnl": row.Pnl, "group": row.Group,
	})
}

func validateFill(row Row) error {
	if err := requireExpect(row, "FILL", "OK", "REJECT"); err != nil {
		return err
	}
	if row.Account == "" {
		return fmt.Errorf("FILL requires account")
	}
	if row.Instrument == "" || row.Side == "" || row.Qty == "" || row.Price == "" {
		return fmt.Errorf("FILL requires instrument, side, qty and price")
	}
	if row.Expect != "REJECT" && row.Reject != "" {
		return fmt.Errorf("FILL reject code is only valid with expect REJECT")
	}
	return forbid("FILL", cellForbid{
		"volume": row.Volume, "asset": row.Asset,
		"amount": row.Amount, "group": row.Group,
	})
}

func validateGroup(row Row) error {
	if err := requireExpect(row, "GROUP", "OK"); err != nil {
		return err
	}
	if row.Account == "" || row.Group == "" {
		return fmt.Errorf("GROUP requires account and group")
	}
	return forbid("GROUP", cellForbid{
		"instrument": row.Instrument, "side": row.Side, "qty": row.Qty,
		"volume": row.Volume, "price": row.Price, "asset": row.Asset,
		"amount": row.Amount, "fee": row.Fee, "pnl": row.Pnl,
		"reject": row.Reject,
	})
}

// cellForbid maps a column name to the cell value an action does not allow.
type cellForbid map[string]string

func forbid(action string, cells cellForbid) error {
	for col, value := range cells {
		if value != "" {
			return fmt.Errorf("%s does not use the %q column", action, col)
		}
	}
	return nil
}

func requireExpect(row Row, action string, allowed ...string) error {
	for _, a := range allowed {
		if row.Expect == a {
			return nil
		}
	}
	return fmt.Errorf(
		"%s expect must be one of %s, got %q",
		action, strings.Join(allowed, "/"), row.Expect,
	)
}
