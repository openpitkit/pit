# Copyright The Pit Project Owners. All rights reserved.
# SPDX-License-Identifier: Apache-2.0
#
# Licensed under the Apache License, Version 2.0 (the "License");
# you may not use this file except in compliance with the License.
# You may obtain a copy of the License at
#
#     http://www.apache.org/licenses/LICENSE-2.0
#
# Unless required by applicable law or agreed to in writing, software
# distributed under the License is distributed on an "AS IS" BASIS,
# WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
# See the License for the specific language governing permissions and
# limitations under the License.
#
# Please see https://github.com/openpitkit and the OWNERS file for details.

"""Scenario table parser for the spot_table example."""

from __future__ import annotations

from dataclasses import dataclass, field

# ---------------------------------------------------------------------------
# Data model
# ---------------------------------------------------------------------------


@dataclass(frozen=True)
class Frontmatter:
    """Per-file configuration block."""

    name: str = ""
    slippage_bps: int = 0


@dataclass(frozen=True)
class Row:
    """One parsed table row.

    Empty fields mean "not applicable to this action"; per-action validation
    enforces which cells each action requires or forbids.
    """

    line: int = 0
    step: str = ""
    account: str = ""
    action: str = ""
    instrument: str = ""
    side: str = ""
    qty: str = ""
    volume: str = ""
    price: str = ""
    asset: str = ""
    amount: str = ""
    fee: str = ""
    pnl: str = ""
    group: str = ""
    expect: str = ""
    reject: str = ""
    note: str = ""


@dataclass
class Table:
    """Parsed scenario file."""

    fm: Frontmatter
    rows: list[Row] = field(default_factory=list)


# ---------------------------------------------------------------------------
# Required headers
# ---------------------------------------------------------------------------

# Every other recognized column is optional and read by name when present;
# per-action validation then enforces the cells each action needs.
_REQUIRED_HEADERS = ["account", "action", "expect"]


# ---------------------------------------------------------------------------
# Public entry points
# ---------------------------------------------------------------------------


def parse_file(path: str) -> Table:
    """Read and parse a table file."""
    with open(path) as fh:
        text = fh.read()
    return parse(text, path)


def parse(text: str, name: str) -> Table:
    """Parse the table from *text*. *name* is used in error messages."""
    lines = text.splitlines()
    t = Table(fm=Frontmatter())
    line_no = 0
    state = _STATE_START
    headers: list[str] = []

    for raw in lines:
        line_no += 1
        trimmed = raw.strip()

        if state == _STATE_START:
            if trimmed == "---":
                state = _STATE_FM
                continue
            if _is_table_row(trimmed):
                headers = _split_row(trimmed)
                state = _STATE_AWAIT_DIVIDER
                continue
            # other text - skip

        elif state == _STATE_FM:
            if trimmed == "---":
                state = _STATE_BODY
                continue
            new_fm = _parse_fm_line(t.fm, trimmed, line_no, name)
            t.fm = new_fm

        elif state == _STATE_BODY:
            if _is_table_row(trimmed):
                headers = _split_row(trimmed)
                state = _STATE_AWAIT_DIVIDER

        elif state == _STATE_AWAIT_DIVIDER:
            if not _is_divider_row(trimmed):
                raise ValueError(
                    f"{name}:{line_no}: expected table divider after header,"
                    f" got {trimmed!r}"
                )
            try:
                _check_headers(headers)
            except ValueError as exc:
                raise ValueError(f"{name}:{line_no - 1}: {exc}") from None
            state = _STATE_ROWS

        elif state == _STATE_ROWS:
            if not _is_table_row(trimmed):
                # table ended; v1 takes only the first table block.
                state = _STATE_DONE
                continue
            fields = _split_row(trimmed)
            try:
                row = _build_row(fields, headers, line_no)
            except ValueError as exc:
                raise ValueError(f"{name}:{line_no}: {exc}") from None
            t.rows.append(row)

        # _STATE_DONE: ignore trailing prose

    if state not in (_STATE_ROWS, _STATE_DONE):
        raise ValueError(f"{name}: no table found")
    if not t.rows:
        raise ValueError(f"{name}: table has no rows")
    return t


# ---------------------------------------------------------------------------
# Parse states
# ---------------------------------------------------------------------------

_STATE_START = 0
_STATE_FM = 1
_STATE_BODY = 2
_STATE_AWAIT_DIVIDER = 3
_STATE_ROWS = 4
_STATE_DONE = 5


# ---------------------------------------------------------------------------
# Front-matter
# ---------------------------------------------------------------------------


def _parse_fm_line(fm: Frontmatter, line: str, line_no: int, name: str) -> Frontmatter:
    if not line or line.startswith("#"):
        return fm
    i = line.find(":")
    if i < 0:
        raise ValueError(
            f"{name}:{line_no}: front-matter expects key: value, got {line!r}"
        )
    key = line[:i].strip()
    value = line[i + 1 :].strip()
    if key == "name":
        return Frontmatter(name=value, slippage_bps=fm.slippage_bps)
    if key == "slippage_bps":
        try:
            n = int(value)
        except ValueError:
            raise ValueError(
                f"{name}:{line_no}: slippage_bps: invalid literal for int: {value!r}"
            ) from None
        if n < 0 or n > 65535:
            raise ValueError(
                f"{name}:{line_no}: slippage_bps: value {n} out of range 0..65535"
            )
        return Frontmatter(name=fm.name, slippage_bps=n)
    raise ValueError(f"{name}:{line_no}: unknown front-matter key {key!r}")


# ---------------------------------------------------------------------------
# Table row helpers
# ---------------------------------------------------------------------------


def _is_table_row(s: str) -> bool:
    return s.startswith("|") and s.endswith("|")


def _is_divider_row(s: str) -> bool:
    if not _is_table_row(s):
        return False
    return all(ch in "|-: \t" for ch in s)


def _split_row(s: str) -> list[str]:
    inner = s[1:-1]  # strip leading and trailing '|'
    return [part.strip() for part in inner.split("|")]


def _check_headers(got: list[str]) -> None:
    for want in _REQUIRED_HEADERS:
        if not _has_header(got, want):
            raise ValueError(
                f"missing required column {want!r}"
                f" (required: {','.join(_REQUIRED_HEADERS)})"
            )


def _has_header(headers: list[str], name: str) -> bool:
    name_lower = name.lower()
    return any(h.lower() == name_lower for h in headers)


def _build_row(fields: list[str], headers: list[str], line_no: int) -> Row:
    def cell(col: str) -> str:
        col_lower = col.lower()
        for i, h in enumerate(headers):
            if h.lower() == col_lower:
                return fields[i] if i < len(fields) else ""
        return ""

    row = Row(
        line=line_no,
        step=cell("#"),
        account=cell("account"),
        action=cell("action").upper(),
        instrument=cell("instrument"),
        side=cell("side").upper(),
        qty=cell("qty"),
        volume=cell("volume"),
        price=cell("price"),
        asset=cell("asset"),
        amount=cell("amount"),
        fee=cell("fee"),
        pnl=cell("pnl"),
        group=cell("group"),
        expect=cell("expect").upper(),
        reject=cell("reject"),
        note=cell("note"),
    )
    _validate_row(row)
    return row


# ---------------------------------------------------------------------------
# Per-action validation
# ---------------------------------------------------------------------------


def _validate_row(row: Row) -> None:
    if row.action == "SEED":
        _validate_seed(row)
    elif row.action == "TICK":
        _validate_tick(row)
    elif row.action == "ORDER":
        _validate_order(row)
    elif row.action == "FILL":
        _validate_fill(row)
    elif row.action == "GROUP":
        _validate_group(row)
    else:
        raise ValueError(f"unknown action {row.action!r}")


def _validate_seed(row: Row) -> None:
    _require_expect(row, "SEED", "OK", "REJECT")
    if not row.account:
        raise ValueError("SEED requires account")
    if not row.asset or not row.amount:
        raise ValueError("SEED requires asset and amount")
    _forbid(
        "SEED",
        {
            "instrument": row.instrument,
            "side": row.side,
            "qty": row.qty,
            "volume": row.volume,
            "price": row.price,
            "group": row.group,
        },
    )


def _validate_tick(row: Row) -> None:
    _require_expect(row, "TICK", "OK")
    if not row.instrument or not row.price:
        raise ValueError("TICK requires instrument and price")
    # account and group are optional: empty = global push, set = addressed push.
    _forbid(
        "TICK",
        {
            "side": row.side,
            "qty": row.qty,
            "volume": row.volume,
            "asset": row.asset,
            "amount": row.amount,
            "fee": row.fee,
            "pnl": row.pnl,
            "reject": row.reject,
        },
    )


def _validate_order(row: Row) -> None:
    _require_expect(row, "ORDER", "ACCEPT", "REJECT")
    if not row.account:
        raise ValueError("ORDER requires account")
    if not row.instrument or not row.side:
        raise ValueError("ORDER requires instrument and side")
    has_qty = bool(row.qty)
    has_volume = bool(row.volume)
    if has_qty and has_volume:
        raise ValueError("ORDER must set exactly one of qty or volume, not both")
    if not has_qty and not has_volume:
        raise ValueError("ORDER must set exactly one of qty or volume")
    if row.expect != "REJECT" and row.reject:
        raise ValueError("ORDER reject code is only valid with expect REJECT")
    _forbid(
        "ORDER",
        {
            "asset": row.asset,
            "amount": row.amount,
            "fee": row.fee,
            "pnl": row.pnl,
            "group": row.group,
        },
    )


def _validate_fill(row: Row) -> None:
    _require_expect(row, "FILL", "OK", "REJECT")
    if not row.account:
        raise ValueError("FILL requires account")
    if not row.instrument or not row.side or not row.qty or not row.price:
        raise ValueError("FILL requires instrument, side, qty and price")
    if row.expect != "REJECT" and row.reject:
        raise ValueError("FILL reject code is only valid with expect REJECT")
    _forbid(
        "FILL",
        {
            "volume": row.volume,
            "asset": row.asset,
            "amount": row.amount,
            "group": row.group,
        },
    )


def _validate_group(row: Row) -> None:
    _require_expect(row, "GROUP", "OK")
    if not row.account or not row.group:
        raise ValueError("GROUP requires account and group")
    _forbid(
        "GROUP",
        {
            "instrument": row.instrument,
            "side": row.side,
            "qty": row.qty,
            "volume": row.volume,
            "price": row.price,
            "asset": row.asset,
            "amount": row.amount,
            "fee": row.fee,
            "pnl": row.pnl,
            "reject": row.reject,
        },
    )


def _forbid(action: str, cells: dict[str, str]) -> None:
    for col, value in cells.items():
        if value:
            raise ValueError(f"{action} does not use the {col!r} column")


def _require_expect(row: Row, action: str, *allowed: str) -> None:
    if row.expect in allowed:
        return
    joined = "/".join(allowed)
    raise ValueError(f"{action} expect must be one of {joined}, got {row.expect!r}")
