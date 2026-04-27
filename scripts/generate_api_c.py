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

from __future__ import annotations

import ast
import contextlib
import re
import shutil
import sys
import textwrap
import traceback
from dataclasses import dataclass, field
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
SRC_DIR = ROOT / "crates" / "pit-ffi" / "src"
HEADER_PATH = ROOT / "bindings" / "c" / "pit.h"
HEADER_COPIES = [
    ROOT / "bindings" / "go" / "internal" / "native" / "pit.h",
]
DOCS_DIR = ROOT / "docs" / "c-api"
OPENPIT_LEVERAGE_RS = ROOT / "crates" / "openpit" / "src" / "param" / "leverage.rs"

RUST_TO_C = {
    "bool": "bool",
    "f32": "float",
    "f64": "double",
    "u8": "uint8_t",
    "u16": "uint16_t",
    "u32": "uint32_t",
    "u64": "uint64_t",
    "usize": "size_t",
    "i8": "int8_t",
    "i16": "int16_t",
    "i32": "int32_t",
    "i64": "int64_t",
    "isize": "ptrdiff_t",
    "c_char": "char",
    "c_void": "void",
    "void": "void",
}

SECTION_INFO = {
    "param.rs": ("params", "Parameter Types"),
    "order.rs": ("orders", "Orders"),
    "execution_report.rs": ("execution-reports", "Execution Reports"),
    "account_adjustment.rs": ("account-adjustments", "Account Adjustments"),
    "reject.rs": ("rejects", "Rejects"),
    "last_error.rs": ("runtime", "Runtime and Errors"),
    "engine.rs": ("engine", "Engine"),
    "policy.rs": ("policies", "Policies"),
    "lib.rs": ("runtime", "Runtime and Errors"),
}


@dataclass
class Field:
    name: str
    rust_type: str
    docs: list[str] = field(default_factory=list)


@dataclass
class Item:
    kind: str
    name: str
    docs: list[str]
    section: str = ""
    attrs: list[str] = field(default_factory=list)
    fields: list[Field] = field(default_factory=list)
    variants: list[tuple[str, int, list[str]]] = field(default_factory=list)
    alias: str | None = None
    args: list[tuple[str, str]] = field(default_factory=list)
    ret: str | None = None
    value: str | None = None
    opaque: bool = False
    repr_name: str | None = None


@dataclass(frozen=True)
class DecimalWrapperDocTemplate:
    wrapper: list[str]
    create: list[str]
    create_args: list[tuple[str, str]]
    create_ret: str | None
    get_decimal: list[str]
    get_decimal_args: list[tuple[str, str]]
    get_decimal_ret: str | None


@dataclass(frozen=True)
class MacroFnSpec:
    meta: str
    docs: list[str]


DECIMAL_PARAM_WRAPPER_CREATE_SIGNATURE = (
    [
        ("value", "PitParamDecimal"),
        ("out", "*mut $wrapper"),
        ("out_error", "PitOutError"),
    ],
    "bool",
)

DECIMAL_PARAM_WRAPPER_GET_DECIMAL_SIGNATURE = (
    [("value", "$wrapper")],
    "PitParamDecimal",
)

DECIMAL_PARAM_FFI_COMMON_SIGNATURES: dict[
    str, tuple[list[tuple[str, str]], str | None]
] = {
    "from_str_fn": (
        [
            ("value", "PitStringView"),
            ("out", "*mut $wrapper"),
            ("out_error", "PitOutError"),
        ],
        "bool",
    ),
    "from_f64_fn": (
        [("value", "f64"), ("out", "*mut $wrapper"), ("out_error", "PitOutError")],
        "bool",
    ),
    "from_i64_fn": (
        [("value", "i64"), ("out", "*mut $wrapper"), ("out_error", "PitOutError")],
        "bool",
    ),
    "from_u64_fn": (
        [("value", "u64"), ("out", "*mut $wrapper"), ("out_error", "PitOutError")],
        "bool",
    ),
    "from_str_rounded_fn": (
        [
            ("value", "PitStringView"),
            ("scale", "u32"),
            ("rounding", "PitParamRoundingStrategy"),
            ("out", "*mut $wrapper"),
            ("out_error", "PitOutError"),
        ],
        "bool",
    ),
    "from_f64_rounded_fn": (
        [
            ("value", "f64"),
            ("scale", "u32"),
            ("rounding", "PitParamRoundingStrategy"),
            ("out", "*mut $wrapper"),
            ("out_error", "PitOutError"),
        ],
        "bool",
    ),
    "from_decimal_rounded_fn": (
        [
            ("value", "PitParamDecimal"),
            ("scale", "u32"),
            ("rounding", "PitParamRoundingStrategy"),
            ("out", "*mut $wrapper"),
            ("out_error", "PitOutError"),
        ],
        "bool",
    ),
    "to_f64_fn": (
        [("value", "$wrapper"), ("out", "*mut f64"), ("out_error", "PitOutError")],
        "bool",
    ),
    "is_zero_fn": (
        [("value", "$wrapper"), ("out", "*mut bool"), ("out_error", "PitOutError")],
        "bool",
    ),
    "compare_fn": (
        [
            ("lhs", "$wrapper"),
            ("rhs", "$wrapper"),
            ("out", "*mut i8"),
            ("out_error", "PitOutError"),
        ],
        "bool",
    ),
    "to_string_fn": (
        [("value", "$wrapper"), ("out_error", "PitOutError")],
        "*mut PitSharedString",
    ),
    "checked_add_fn": (
        [
            ("lhs", "$wrapper"),
            ("rhs", "$wrapper"),
            ("out", "*mut $wrapper"),
            ("out_error", "PitOutError"),
        ],
        "bool",
    ),
    "checked_sub_fn": (
        [
            ("lhs", "$wrapper"),
            ("rhs", "$wrapper"),
            ("out", "*mut $wrapper"),
            ("out_error", "PitOutError"),
        ],
        "bool",
    ),
    "checked_mul_i64_fn": (
        [
            ("value", "$wrapper"),
            ("multiplier", "i64"),
            ("out", "*mut $wrapper"),
            ("out_error", "PitOutError"),
        ],
        "bool",
    ),
    "checked_mul_u64_fn": (
        [
            ("value", "$wrapper"),
            ("multiplier", "u64"),
            ("out", "*mut $wrapper"),
            ("out_error", "PitOutError"),
        ],
        "bool",
    ),
    "checked_mul_f64_fn": (
        [
            ("value", "$wrapper"),
            ("multiplier", "f64"),
            ("out", "*mut $wrapper"),
            ("out_error", "PitOutError"),
        ],
        "bool",
    ),
    "checked_div_i64_fn": (
        [
            ("value", "$wrapper"),
            ("divisor", "i64"),
            ("out", "*mut $wrapper"),
            ("out_error", "PitOutError"),
        ],
        "bool",
    ),
    "checked_div_u64_fn": (
        [
            ("value", "$wrapper"),
            ("divisor", "u64"),
            ("out", "*mut $wrapper"),
            ("out_error", "PitOutError"),
        ],
        "bool",
    ),
    "checked_div_f64_fn": (
        [
            ("value", "$wrapper"),
            ("divisor", "f64"),
            ("out", "*mut $wrapper"),
            ("out_error", "PitOutError"),
        ],
        "bool",
    ),
    "checked_rem_i64_fn": (
        [
            ("value", "$wrapper"),
            ("divisor", "i64"),
            ("out", "*mut $wrapper"),
            ("out_error", "PitOutError"),
        ],
        "bool",
    ),
    "checked_rem_u64_fn": (
        [
            ("value", "$wrapper"),
            ("divisor", "u64"),
            ("out", "*mut $wrapper"),
            ("out_error", "PitOutError"),
        ],
        "bool",
    ),
    "checked_rem_f64_fn": (
        [
            ("value", "$wrapper"),
            ("divisor", "f64"),
            ("out", "*mut $wrapper"),
            ("out_error", "PitOutError"),
        ],
        "bool",
    ),
}

DECIMAL_PARAM_FFI_SIGNED_SIGNATURES: dict[
    str, tuple[list[tuple[str, str]], str | None]
] = {
    "checked_neg_fn": (
        [("value", "$wrapper"), ("out", "*mut $wrapper"), ("out_error", "PitOutError")],
        "bool",
    )
}


class UnmappedRustTypeError(ValueError):
    pass


class UnsupportedStructShapeError(ValueError):
    pass


def load_openpit_leverage_constants() -> dict[str, str]:
    if not OPENPIT_LEVERAGE_RS.exists():
        return {}
    text = OPENPIT_LEVERAGE_RS.read_text(encoding="utf-8")
    matches = re.findall(
        r"pub const (SCALE|MIN|MAX|STEP):\s*[^=]+=\s*([^;]+);",
        text,
    )
    return {name: value.strip() for name, value in matches}


OPENPIT_LEVERAGE_CONSTS = load_openpit_leverage_constants()


def main() -> None:
    source_files = list_source_files()
    items = []
    for rel in source_files:
        parsed = parse_file(SRC_DIR / rel)
        for item in parsed:
            item.section = rel
        items.extend(parsed)

    items = dedupe_items(items)
    header = render_header(items)
    docs = render_docs(items, source_files)

    HEADER_PATH.parent.mkdir(parents=True, exist_ok=True)
    DOCS_DIR.mkdir(parents=True, exist_ok=True)
    HEADER_PATH.write_text(header, encoding="utf-8")
    for path in DOCS_DIR.glob("*.md"):
        path.unlink()
    for rel_path, text in docs.items():
        path = DOCS_DIR / rel_path
        path.write_text(text, encoding="utf-8")
        print(f"Generated {path.relative_to(ROOT)}")
    print(f"Generated {HEADER_PATH.relative_to(ROOT)}")

    for dest in HEADER_COPIES:
        try:
            dest.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(HEADER_PATH, dest)
            print(f"Copied {HEADER_PATH.relative_to(ROOT)} -> {dest.relative_to(ROOT)}")
        except Exception as e:
            print(
                (
                    f"error: failed to copy {HEADER_PATH.relative_to(ROOT)}"
                    f" to {dest.relative_to(ROOT)}: {e}"
                ),
                file=sys.stderr,
            )
            sys.exit(1)


def dedupe_items(items: list[Item]) -> list[Item]:
    seen: set[tuple[str, str]] = set()
    out: list[Item] = []
    for item in items:
        if not should_export(item):
            continue
        key = (item.kind, item.name)
        if key in seen:
            continue
        seen.add(key)
        out.append(item)
    return out


def map_const_value(value: str) -> str:
    mapped = value
    for name, const_value in OPENPIT_LEVERAGE_CONSTS.items():
        mapped = mapped.replace(f"Leverage::{name}", const_value)
    mapped = re.sub(r"\b([A-Za-z_]\w*)::([A-Za-z_]\w*)\b", r"\1_\2", mapped)
    mapped = re.sub(r"\s+as\s+[A-Za-z_][A-Za-z0-9_:<>]*", "", mapped)
    return mapped


def should_export(item: Item) -> bool:
    if item.kind == "const":
        return item.name.startswith("PIT_")
    if item.kind == "function":
        return item.name.startswith("pit_")
    return item.name.startswith("Pit")


def list_source_files() -> list[str]:
    files = [path.name for path in SRC_DIR.glob("*.rs") if path.is_file()]
    section_order = {name: idx for idx, name in enumerate(SECTION_INFO)}
    return sorted(
        files, key=lambda name: (section_order.get(name, len(section_order)), name)
    )


def section_info(source: str) -> tuple[str, str]:
    info = SECTION_INFO.get(source)
    if info:
        return info
    stem = Path(source).stem
    words = stem.split("_")
    title = " ".join(word.capitalize() for word in words)
    return stem.replace("_", "-"), title


def parse_file(path: Path) -> list[Item]:
    lines = path.read_text(encoding="utf-8").splitlines()
    items: list[Item] = []
    docs: list[str] = []
    attrs: list[str] = []
    decimal_wrapper_template: DecimalWrapperDocTemplate | None = None
    decimal_ffi_common_specs: list[MacroFnSpec] = []
    decimal_ffi_signed_specs: list[MacroFnSpec] = []
    i = 0
    skip_depth = 0
    while i < len(lines):
        line = lines[i]
        stripped = line.strip()
        if skip_depth:
            skip_depth += line.count("{") - line.count("}")
            i += 1
            continue
        if stripped.startswith("#[cfg(test)]"):
            j = i + 1
            while j < len(lines) and not lines[j].strip():
                j += 1
            if j < len(lines) and lines[j].strip().startswith("mod tests"):
                skip_depth = lines[j].count("{") - lines[j].count("}")
                i = j + 1
                docs = []
                attrs = []
                continue
        if stripped.startswith("///"):
            docs.append(stripped[3:].lstrip())
            i += 1
            continue
        if stripped.startswith("#["):
            attr_block, i = collect_attribute(lines, i)
            if is_doc_attribute(attr_block):
                docs.append(parse_doc_attribute(attr_block, {}))
            else:
                attrs.append(normalize_inline_block(attr_block))
            continue
        if not stripped:
            docs = []
            attrs = []
            i += 1
            continue
        if stripped.startswith("macro_rules! define_decimal_param_wrapper"):
            block, i = collect_braced(lines, i, "{", "}")
            decimal_wrapper_template = parse_decimal_wrapper_template(block)
            docs = []
            attrs = []
            continue
        if stripped.startswith("macro_rules! define_decimal_param_ffi_common"):
            block, i = collect_braced(lines, i, "{", "}")
            decimal_ffi_common_specs = parse_macro_fn_specs(block)
            docs = []
            attrs = []
            continue
        if stripped.startswith("macro_rules! define_decimal_param_ffi_signed"):
            block, i = collect_braced(lines, i, "{", "}")
            decimal_ffi_signed_specs = parse_macro_fn_specs(block)
            docs = []
            attrs = []
            continue
        if stripped.startswith("macro_rules!"):
            i = skip_block(lines, i)
            docs = []
            attrs = []
            continue
        if stripped.startswith("define_decimal_param_wrapper!("):
            block, i = collect_macro_invocation(lines, i)
            items.extend(parse_decimal_wrapper(block, decimal_wrapper_template))
            docs = []
            attrs = []
            continue
        if stripped.startswith("define_optional!("):
            block, i = collect_macro_invocation(lines, i)
            item = parse_optional_wrapper(block, docs, attrs)
            if item:
                items.append(item)
            docs = []
            attrs = []
            continue
        if stripped.startswith("policy_release_fn!("):
            block, i = collect_macro_invocation(lines, i)
            item = parse_macro_function(block, docs, "release")
            if item:
                items.append(item)
            docs = []
            attrs = []
            continue
        if stripped.startswith("policy_destroy_fn!("):
            block, i = collect_macro_invocation(lines, i)
            item = parse_macro_function(block, docs, "destroy")
            if item:
                items.append(item)
            docs = []
            attrs = []
            continue
        if stripped.startswith("policy_get_name_fn!("):
            block, i = collect_macro_invocation(lines, i)
            item = parse_macro_function(block, docs, "get_name")
            if item:
                items.append(item)
            docs = []
            attrs = []
            continue
        if stripped.startswith("define_decimal_param_ffi_common!("):
            block, i = collect_macro_invocation(lines, i)
            items.extend(parse_decimal_ffi_common(block, decimal_ffi_common_specs))
            docs = []
            attrs = []
            continue
        if stripped.startswith("define_decimal_param_ffi_signed!("):
            block, i = collect_macro_invocation(lines, i)
            items.extend(parse_decimal_ffi_signed(block, decimal_ffi_signed_specs))
            docs = []
            attrs = []
            continue
        if stripped.startswith("pub const "):
            block, i = collect_until_semicolon(lines, i)
            item = parse_const(block, docs, attrs)
            if item:
                items.append(item)
            docs = []
            attrs = []
            continue
        if stripped.startswith("pub struct "):
            struct_name_match = re.match(r"pub struct (\w+)", stripped)
            if struct_name_match and not struct_name_match.group(1).startswith("Pit"):
                _block, i = collect_item(lines, i)
                docs = []
                attrs = []
                continue
            block, i = collect_item(lines, i)
            item = parse_struct(block, docs, attrs)
            if item:
                items.append(item)
            docs = []
            attrs = []
            continue
        if stripped.startswith("pub enum "):
            block, i = collect_item(lines, i)
            item = parse_enum(block, docs, attrs)
            if item:
                items.append(item)
            docs = []
            attrs = []
            continue
        if stripped.startswith("pub type "):
            block, i = collect_until_semicolon(lines, i)
            item = parse_type_alias(block, docs, attrs)
            if item:
                items.append(item)
            docs = []
            attrs = []
            continue
        if 'pub extern "C" fn ' in stripped or 'pub unsafe extern "C" fn ' in stripped:
            block, i = collect_function(lines, i)
            item = parse_function(block, docs, attrs)
            if item:
                items.append(item)
            docs = []
            attrs = []
            continue
        if (
            stripped.startswith("impl ")
            or stripped.startswith("fn ")
            or stripped.startswith("trait ")
        ):
            i = skip_block(lines, i)
            docs = []
            attrs = []
            continue
        docs = []
        attrs = []
        i += 1
    return items


def skip_block(lines: list[str], start: int) -> int:
    depth = 0
    i = start
    saw_open_brace = "{" in lines[start]
    while i < len(lines):
        line = lines[i]
        if "{" in line:
            saw_open_brace = True
        depth += line.count("{") - line.count("}")
        i += 1
        if saw_open_brace and depth <= 0:
            return i
        if not saw_open_brace and line.strip().endswith(";"):
            return i
    return i


def collect_item(lines: list[str], start: int) -> tuple[str, int]:
    first = lines[start]
    if "{" in first:
        return collect_braced(lines, start, "{", "}")
    if "(" in first and first.rstrip().endswith(");"):
        return first, start + 1
    return collect_until_semicolon(lines, start)


def collect_function(lines: list[str], start: int) -> tuple[str, int]:
    parts = []
    i = start
    while i < len(lines):
        parts.append(lines[i].rstrip())
        if "{" in lines[i]:
            break
        i += 1
    return " ".join(parts), i + 1


def collect_until_semicolon(lines: list[str], start: int) -> tuple[str, int]:
    parts = []
    i = start
    while i < len(lines):
        parts.append(lines[i].rstrip())
        if lines[i].strip().endswith(";"):
            break
        i += 1
    return " ".join(parts), i + 1


def collect_macro_invocation(lines: list[str], start: int) -> tuple[str, int]:
    parts = []
    depth = 0
    i = start
    while i < len(lines):
        line = lines[i].rstrip()
        parts.append(line)
        depth += line.count("(") - line.count(")")
        i += 1
        if depth <= 0 and line.endswith(");"):
            break
    return "\n".join(parts), i


def collect_attribute(lines: list[str], start: int) -> tuple[str, int]:
    parts = []
    depth = 0
    i = start
    while i < len(lines):
        line = lines[i].rstrip()
        parts.append(line)
        depth += line.count("[") - line.count("]")
        i += 1
        if depth <= 0:
            break
    return "\n".join(parts), i


def collect_braced(
    lines: list[str], start: int, open_char: str, close_char: str
) -> tuple[str, int]:
    parts = []
    depth = 0
    i = start
    while i < len(lines):
        line = lines[i].rstrip()
        parts.append(line)
        depth += line.count(open_char) - line.count(close_char)
        i += 1
        if depth <= 0:
            break
    return "\n".join(parts), i


def parse_const(block: str, docs: list[str], attrs: list[str]) -> Item | None:
    normalized = " ".join(block.split())
    match = re.match(r"pub const (\w+): ([^=]+)= (.+);", normalized)
    if not match:
        return None
    return Item(
        kind="const",
        name=match.group(1),
        docs=list(docs),
        attrs=list(attrs),
        alias=match.group(2).strip(),
        value=match.group(3).strip(),
    )


def parse_struct(block: str, docs: list[str], attrs: list[str]) -> Item | None:
    header = block.splitlines()[0].strip()
    unit_match = re.match(r"pub struct (\w+)\s*;", header)
    if unit_match:
        name = unit_match.group(1)
        repr_name = parse_repr(attrs)
        opaque = repr_name is None
        return Item(
            kind="struct",
            name=name,
            docs=list(docs),
            attrs=list(attrs),
            fields=[],
            opaque=opaque,
            repr_name=repr_name,
        )
    if "{" in header:
        match = re.match(r"pub struct (\w+)\s*\{", header)
        if not match:
            raise UnsupportedStructShapeError(
                f"unsupported struct declaration: `{header}`"
            )
        name = match.group(1)
        field_docs: list[str] = []
        fields: list[Field] = []
        for line in block.splitlines()[1:]:
            stripped = line.strip()
            if stripped in {"}", "};"}:
                break
            if stripped.startswith("///"):
                field_docs.append(stripped[3:].lstrip())
                continue
            if stripped.startswith("#["):
                continue
            if not stripped or not stripped.startswith("pub "):
                continue
            field_match = re.match(r"pub (\w+): (.+),", stripped)
            if field_match:
                fields.append(
                    Field(
                        field_match.group(1), field_match.group(2).strip(), field_docs
                    )
                )
                field_docs = []
        repr_name = parse_repr(attrs)
        opaque = repr_name is None
        return Item(
            kind="struct",
            name=name,
            docs=list(docs),
            attrs=list(attrs),
            fields=fields,
            opaque=opaque,
            repr_name=repr_name,
        )
    match = re.match(r"pub struct (\w+)\((.+)\);", header)
    if not match:
        raise UnsupportedStructShapeError(f"unsupported struct declaration: `{header}`")
    name = match.group(1)
    raw_fields = split_top_level(match.group(2), ",")
    fields = []
    for index, raw in enumerate(raw_fields):
        raw = raw.strip()
        raw = re.sub(r"^pub\s+", "", raw)
        fields.append(Field(f"_{index}", raw))
    return Item(
        kind="struct",
        name=name,
        docs=list(docs),
        attrs=list(attrs),
        fields=fields,
        repr_name=parse_repr(attrs),
    )


def parse_enum(block: str, docs: list[str], attrs: list[str]) -> Item | None:
    header = block.splitlines()[0].strip()
    match = re.match(r"pub enum (\w+)\s*\{", header)
    if not match:
        return None
    current_value = -1
    current_docs: list[str] = []
    variants: list[tuple[str, int, list[str]]] = []
    for line in block.splitlines()[1:]:
        stripped = line.strip()
        if stripped in {"}", "};"}:
            break
        if stripped.startswith("///"):
            current_docs.append(stripped[3:].lstrip())
            continue
        if stripped.startswith("#["):
            continue
        if not stripped:
            continue
        variant_match = re.match(r"(\w+)(?: = ([^,]+))?,", stripped)
        if variant_match:
            if variant_match.group(2):
                current_value = int(variant_match.group(2).strip())
            else:
                current_value += 1
            variants.append((variant_match.group(1), current_value, current_docs))
            current_docs = []
    return Item(
        kind="enum",
        name=match.group(1),
        docs=list(docs),
        attrs=list(attrs),
        variants=variants,
        repr_name=parse_repr(attrs),
    )


def parse_type_alias(block: str, docs: list[str], attrs: list[str]) -> Item | None:
    match = re.match(r"pub type (\w+)\s*=\s*(.+);", " ".join(block.split()))
    if not match:
        return None
    name = match.group(1)
    rhs = match.group(2).strip()
    if 'extern "C" fn' in rhs:
        args, ret = parse_fn_pointer(rhs)
        return Item(
            kind="typedef_fn",
            name=name,
            docs=list(docs),
            attrs=list(attrs),
            args=args,
            ret=ret,
        )
    try:
        map_type(rhs)
        return Item(
            kind="alias",
            name=name,
            docs=list(docs),
            attrs=list(attrs),
            alias=rhs,
        )
    except UnmappedRustTypeError:
        pass
    if re.match(r"\w+(?:::\w+)*(?:<.+>)?$", rhs):
        return Item(
            kind="opaque", name=name, docs=list(docs), attrs=list(attrs), opaque=True
        )
    return None


def parse_function(block: str, docs: list[str], attrs: list[str]) -> Item | None:
    signature = block.split("{", 1)[0].strip()
    signature = re.sub(r"\s+", " ", signature)
    match = re.match(
        r"pub(?: unsafe)? extern \"C\" fn (\w+)\((.*)\)(?: -> (.+))?$",
        signature,
    )
    if not match:
        return None
    return Item(
        kind="function",
        name=match.group(1),
        docs=list(docs),
        attrs=list(attrs),
        args=parse_args(match.group(2)),
        ret=match.group(3).strip() if match.group(3) else None,
    )


def parse_decimal_wrapper(
    block: str, template: DecimalWrapperDocTemplate | None
) -> list[Item]:
    if template is None:
        raise ValueError("missing define_decimal_param_wrapper documentation template")
    values = parse_macro_keyword_values(block)
    wrapper = values["wrapper"]
    create_fn = values["create_fn"]
    get_decimal_fn = values["get_decimal_fn"]
    wrapper_docs = expand_doc_entries(template.wrapper, values)
    create_docs = expand_doc_entries(template.create, values)
    get_decimal_docs = expand_doc_entries(template.get_decimal, values)

    def sub(t: str) -> str:
        return t.replace("$wrapper", wrapper)

    return [
        Item(
            kind="struct",
            name=wrapper,
            docs=wrapper_docs,
            section="param.rs",
            attrs=["#[repr(transparent)]"],
            fields=[Field("_0", "PitParamDecimal")],
            repr_name="transparent",
        ),
        Item(
            kind="function",
            name=create_fn,
            docs=create_docs,
            section="param.rs",
            args=[(n, sub(t)) for n, t in template.create_args],
            ret=sub(template.create_ret) if template.create_ret else None,
        ),
        Item(
            kind="function",
            name=get_decimal_fn,
            docs=get_decimal_docs,
            section="param.rs",
            args=[(n, sub(t)) for n, t in template.get_decimal_args],
            ret=sub(template.get_decimal_ret) if template.get_decimal_ret else None,
        ),
    ]


def parse_macro_keyword_values(block: str) -> dict[str, str]:
    pairs = re.findall(
        r"(\w+)\s*=\s*(\"(?:\\.|[^\"])*\"|[A-Za-z_]\w*(?:::\w+)*)",
        block,
    )
    return {key: value for key, value in pairs}


def parse_decimal_wrapper_template(block: str) -> DecimalWrapperDocTemplate:
    docs_by_item: dict[str, list[str]] = {}
    pending_docs: list[str] = []
    lines = block.splitlines()
    i = 0
    while i < len(lines):
        stripped = lines[i].strip()
        if stripped.startswith("///"):
            pending_docs.append(lines[i])
            i += 1
            continue
        if stripped.startswith("#["):
            attr_block, i = collect_attribute(lines, i)
            if is_doc_attribute(attr_block):
                pending_docs.append(attr_block)
            continue
        if "pub struct $wrapper" in stripped:
            docs_by_item["wrapper"] = list(pending_docs)
            pending_docs = []
            i += 1
            continue
        if 'pub unsafe extern "C" fn $create_fn' in stripped:
            docs_by_item["create"] = list(pending_docs)
            pending_docs = []
            i += 1
            continue
        if 'pub extern "C" fn $get_decimal_fn' in stripped:
            docs_by_item["get_decimal"] = list(pending_docs)
            pending_docs = []
            i += 1
            continue
        pending_docs = []
        i += 1
    missing = {"wrapper", "create", "get_decimal"} - docs_by_item.keys()
    if missing:
        missing_list = ", ".join(sorted(missing))
        raise ValueError(
            "failed to extract define_decimal_param_wrapper docs for " f"{missing_list}"
        )
    create_args, create_ret = DECIMAL_PARAM_WRAPPER_CREATE_SIGNATURE
    get_decimal_args, get_decimal_ret = DECIMAL_PARAM_WRAPPER_GET_DECIMAL_SIGNATURE
    return DecimalWrapperDocTemplate(
        wrapper=docs_by_item["wrapper"],
        create=docs_by_item["create"],
        create_args=create_args,
        create_ret=create_ret,
        get_decimal=docs_by_item["get_decimal"],
        get_decimal_args=get_decimal_args,
        get_decimal_ret=get_decimal_ret,
    )


def expand_doc_entries(entries: list[str], substitutions: dict[str, str]) -> list[str]:
    docs: list[str] = []
    for entry in entries:
        stripped = entry.strip()
        if stripped.startswith("///"):
            docs.append(stripped[3:].lstrip())
            continue
        docs.append(parse_doc_attribute(entry, substitutions))
    return normalize_doc_lines(docs)


def is_doc_attribute(attr_block: str) -> bool:
    return normalize_inline_block(attr_block).startswith("#[doc")


def parse_doc_attribute(attr_block: str, substitutions: dict[str, str]) -> str:
    normalized = normalize_inline_block(attr_block)
    match = re.fullmatch(r"#\[doc\s*=\s*(.+)\]", normalized)
    if not match:
        raise ValueError(f"unsupported doc attribute: {normalized}")
    return evaluate_doc_expr(match.group(1), substitutions)


def normalize_inline_block(block: str) -> str:
    return " ".join(line.strip() for line in block.splitlines())


def normalize_doc_lines(lines: list[str]) -> list[str]:
    normalized: list[str] = []
    in_code_block = False
    for raw_line in lines:
        line = raw_line.rstrip()
        stripped = line.strip()
        if not stripped:
            if normalized and normalized[-1] != "":
                normalized.append("")
            continue
        if stripped.startswith("```"):
            normalized.append(stripped)
            in_code_block = not in_code_block
            continue
        if in_code_block:
            normalized.append(line)
            continue
        if not normalized or normalized[-1] == "":
            normalized.append(stripped)
            continue
        if starts_new_doc_block(stripped):
            normalized.append(stripped)
            continue
        normalized[-1] = f"{normalized[-1]} {stripped}"
    return normalized


def starts_new_doc_block(line: str) -> bool:
    if line.startswith(("#", ">", "|", "- ", "* ", "```")):
        return True
    return bool(re.match(r"\d+\. ", line))


def evaluate_doc_expr(expr: str, substitutions: dict[str, str]) -> str:
    expr = expr.strip()
    if expr.startswith("concat!(") and expr.endswith(")"):
        inner = expr[len("concat!(") : -1]
        return "".join(
            evaluate_doc_expr(part, substitutions)
            for part in split_top_level(inner, ",")
            if part.strip()
        )
    if expr.startswith("stringify!(") and expr.endswith(")"):
        inner = expr[len("stringify!(") : -1].strip()
        return substitutions[inner.lstrip("$")]
    if expr.startswith('"'):
        return ast.literal_eval(expr)
    if expr.startswith("$"):
        value = substitutions[expr[1:]]
        return ast.literal_eval(value) if value.startswith('"') else value
    raise ValueError(f"unsupported doc expression: {expr}")


def parse_optional_wrapper(
    block: str, docs: list[str], attrs: list[str]
) -> Item | None:
    optional_match = re.search(r"optional\s*=\s*(\w+)", block)
    value_match = re.search(r"value\s*=\s*([A-Za-z_]\w*(?:::\w+)*)", block)
    if not optional_match or not value_match:
        return None
    return Item(
        kind="struct",
        name=optional_match.group(1),
        docs=list(docs),
        attrs=["#[repr(C)]"] + list(attrs),
        fields=[Field("value", value_match.group(1)), Field("is_set", "bool")],
        opaque=False,
        repr_name="C",
    )


def parse_macro_function(block: str, docs: list[str], kind: str) -> Item | None:
    lines = [line.strip() for line in block.splitlines() if line.strip()]
    args = [line.rstrip(",);") for line in lines[1:] if not line.startswith("///")]
    if len(args) < 2:
        return None
    fn_name = args[0]
    policy_ty = args[1]
    if kind in {"release", "destroy"}:
        return Item(
            kind="function",
            name=fn_name,
            docs=list(docs),
            section="policy.rs",
            args=[("policy", f"*mut {policy_ty}")],
            ret=None,
        )
    if kind == "get_name":
        return Item(
            kind="function",
            name=fn_name,
            docs=list(docs),
            section="policy.rs",
            args=[("policy", f"*const {policy_ty}")],
            ret="PitStringView",
        )
    return Item(
        kind="function",
        name=fn_name,
        docs=list(docs),
        section="policy.rs",
        args=[("policy", f"*const {policy_ty}")],
        ret="PitStringView",
    )


def parse_macro_fn_specs(block: str) -> list[MacroFnSpec]:
    normalized = re.sub(r"\$(\w+)", r"META_\1", block)
    lines = normalized.splitlines()
    specs: list[MacroFnSpec] = []
    pending_docs: list[str] = []
    i = 0
    while i < len(lines):
        stripped = lines[i].strip()
        if stripped.startswith("///"):
            pending_docs.append(stripped[3:].lstrip())
            i += 1
            continue
        if stripped.startswith("#["):
            attr_block, i = collect_attribute(lines, i)
            if is_doc_attribute(attr_block):
                with contextlib.suppress(ValueError):
                    pending_docs.append(parse_doc_attribute(attr_block, {}))
            continue
        if 'extern "C" fn META_' in stripped:
            meta_match = re.match(
                r'pub(?:\s+unsafe)?\s+extern\s+"C"\s+fn\s+META_(\w+)\s*\(',
                stripped,
            )
            if meta_match:
                meta = meta_match.group(1)
                _, i = collect_function(lines, i)
                specs.append(MacroFnSpec(meta=meta, docs=list(pending_docs)))
                pending_docs = []
                continue
        if not stripped or stripped in {"};", "}"}:
            pending_docs = []
        i += 1
    return specs


def _instantiate_ffi_specs(
    block: str,
    specs: list[MacroFnSpec],
    signatures: dict[str, tuple[list[tuple[str, str]], str | None]],
) -> list[Item]:
    values = parse_macro_keyword_values(block)
    w = values["wrapper"]

    def sub(t: str) -> str:
        return t.replace("$wrapper", w)

    items: list[Item] = []
    for spec in specs:
        fn_name = values.get(spec.meta)
        if not fn_name:
            continue
        signature = signatures.get(spec.meta)
        if signature is None:
            raise ValueError(f"missing hardcoded signature for macro key `{spec.meta}`")
        args, ret = signature
        items.append(
            Item(
                kind="function",
                name=fn_name,
                docs=list(spec.docs),
                args=[(n, sub(t)) for n, t in args],
                ret=sub(ret) if ret else None,
            )
        )
    return items


def parse_decimal_ffi_common(block: str, specs: list[MacroFnSpec]) -> list[Item]:
    return _instantiate_ffi_specs(block, specs, DECIMAL_PARAM_FFI_COMMON_SIGNATURES)


def parse_decimal_ffi_signed(block: str, specs: list[MacroFnSpec]) -> list[Item]:
    return _instantiate_ffi_specs(block, specs, DECIMAL_PARAM_FFI_SIGNED_SIGNATURES)


def parse_fn_pointer(rhs: str) -> tuple[list[tuple[str, str]], str | None]:
    match = re.match(r"(?:unsafe )?extern \"C\" fn\((.*)\)(?: -> (.+))?", rhs)
    if not match:
        return [], None
    return parse_args(match.group(1)), (
        match.group(2).strip() if match.group(2) else None
    )


def parse_args(arg_text: str) -> list[tuple[str, str]]:
    arg_text = arg_text.strip()
    if not arg_text:
        return []
    args = []
    for chunk in split_top_level(arg_text, ","):
        chunk = chunk.strip()
        if not chunk:
            continue
        name, rust_type = chunk.split(":", 1)
        args.append((name.strip(), rust_type.strip()))
    return args


def split_top_level(text: str, delimiter: str) -> list[str]:
    parts = []
    depth = 0
    current = []
    pairs = {"(": ")", "[": "]", "<": ">"}
    closing = {value: key for key, value in pairs.items()}
    for char in text:
        if char in pairs:
            depth += 1
        elif char in closing:
            depth -= 1
        if char == delimiter and depth == 0:
            parts.append("".join(current))
            current = []
            continue
        current.append(char)
    parts.append("".join(current))
    return parts


def parse_repr(attrs: list[str]) -> str | None:
    for attr in attrs:
        match = re.match(r"#\[repr\(([^)]+)\)\]", attr)
        if match:
            return match.group(1).strip()
    return None


def rust_type_name(raw: str) -> str:
    return raw.strip().split("::")[-1]


def map_type(rust_type: str) -> str:
    rust_type = rust_type.strip()
    rust_type = rust_type.replace("std::ffi::", "")
    rust_type = rust_type.replace("core::ffi::", "")
    rust_type = rust_type.replace("::std::ffi::", "")
    if rust_type.startswith("*const "):
        inner_type = map_type(rust_type[7:])
        if inner_type.startswith("const "):
            return compact_pointer_spacing(f"{inner_type} *")
        return compact_pointer_spacing(f"const {inner_type} *")
    if rust_type.startswith("*mut "):
        return compact_pointer_spacing(f"{map_type(rust_type[5:])} *")
    m = re.fullmatch(r"Option<(Pit\w+Fn)>", rust_type)
    if m:
        return m.group(1)
    if rust_type in RUST_TO_C:
        return RUST_TO_C[rust_type]
    c_name = rust_type_name(rust_type)
    if not c_name.startswith("Pit"):
        raise UnmappedRustTypeError(
            f"unmapped Rust type `{rust_type}` (resolved to `{c_name}`) "
            f"has no C equivalent in RUST_TO_C"
        )
    return compact_pointer_spacing(c_name)


def compact_pointer_spacing(value: str) -> str:
    value = re.sub(r"\s+\*\s+\*", " **", value)
    value = re.sub(r"\s+\*\s+\*\s+\*", " ***", value)
    return re.sub(r"\s{2,}", " ", value).strip()


def format_field_decl(field: Field) -> str:
    array_match = re.match(r"\[(.+);\s*([^\]]+)\]", field.rust_type)
    if array_match:
        elem_type = map_type(array_match.group(1))
        return f"{elem_type} {field.name}[{array_match.group(2).strip()}];"
    return f"{map_type(field.rust_type)} {field.name};"


def unwrap_array_type(rust_type: str) -> str | None:
    array_match = re.match(r"\[(.+);\s*([^\]]+)\]", rust_type.strip())
    if not array_match:
        return None
    return array_match.group(1).strip()


def value_type_dependencies(rust_type: str) -> set[str]:
    rust_type = rust_type.strip()
    if not rust_type:
        return set()
    if rust_type.startswith("*const ") or rust_type.startswith("*mut "):
        return set()
    array_inner = unwrap_array_type(rust_type)
    if array_inner:
        return value_type_dependencies(array_inner)
    generic_match = re.match(r"([A-Za-z_]\w*(?:::\w+)*)<(.+)>$", rust_type)
    if generic_match:
        dependencies: set[str] = set()
        for chunk in split_top_level(generic_match.group(2), ","):
            dependencies.update(value_type_dependencies(chunk))
        return dependencies
    base_name = rust_type_name(rust_type)
    return {base_name} if base_name.startswith("Pit") else set()


def order_struct_items(struct_items: list[Item]) -> list[Item]:
    names = {item.name for item in struct_items}
    dependencies: dict[str, set[str]] = {}
    for item in struct_items:
        item_dependencies: set[str] = set()
        for field_item in item.fields:
            for dep in value_type_dependencies(field_item.rust_type):
                if dep in names and dep != item.name:
                    item_dependencies.add(dep)
        dependencies[item.name] = item_dependencies

    ordered: list[Item] = []
    emitted: set[str] = set()
    while len(ordered) < len(struct_items):
        progress = False
        for item in struct_items:
            if item.name in emitted:
                continue
            if dependencies[item.name].issubset(emitted):
                ordered.append(item)
                emitted.add(item.name)
                progress = True
        if progress:
            continue
        # Keep generation robust in case of an unexpected dependency cycle.
        for item in struct_items:
            if item.name not in emitted:
                ordered.append(item)
                emitted.add(item.name)
    return ordered


def format_args(args: list[tuple[str, str]]) -> str:
    if not args:
        return "void"
    return ", ".join(f"{map_type(rust_type)} {name}" for name, rust_type in args)


def format_doc_comment(lines: list[str]) -> str:
    if not lines:
        return ""
    wrapped: list[str] = []
    for line in lines:
        if not line:
            wrapped.append("")
            continue
        if line.startswith("- "):
            chunks = textwrap.wrap(
                line[2:],
                width=74,
                initial_indent="- ",
                subsequent_indent="  ",
                break_long_words=False,
                break_on_hyphens=False,
            )
            wrapped.extend(chunks)
            continue
        wrapped.extend(
            textwrap.wrap(
                line,
                width=76,
                break_long_words=False,
                break_on_hyphens=False,
            )
            or [""]
        )
    body = "\n".join(f" * {line}" if line else " *" for line in wrapped)
    return f"/**\n{body}\n */\n"


def format_markdown_lines(lines: list[str]) -> list[str]:
    wrapped: list[str] = []
    for line in lines:
        if not line:
            wrapped.append("")
            continue
        if line.startswith("# "):
            wrapped.append(f"### {line[2:]}")
            continue
        if line.startswith("- "):
            wrapped.extend(
                textwrap.wrap(
                    line[2:],
                    width=78,
                    initial_indent="- ",
                    subsequent_indent="  ",
                    break_long_words=False,
                    break_on_hyphens=False,
                )
            )
            continue
        wrapped.extend(
            textwrap.wrap(
                line,
                width=80,
                break_long_words=False,
                break_on_hyphens=False,
            )
            or [""]
        )
    normalized: list[str] = []
    in_list = False
    for line in wrapped:
        is_bullet = line.startswith("- ") or line.startswith("  ")
        if is_bullet and not in_list and normalized and normalized[-1] != "":
            normalized.append("")
        if not is_bullet and in_list and normalized and normalized[-1] != "":
            normalized.append("")
        normalized.append(line)
        in_list = is_bullet
    if in_list and normalized and normalized[-1] != "":
        normalized.append("")
    return normalized


def format_multiline_function(
    name: str, args: list[tuple[str, str]], ret: str | None
) -> str:
    ret_type = map_type(ret or "void")
    if not args:
        line = f"{ret_type} {name}(void);"
        if len(line) <= 80:
            return line
        return f"{ret_type}\n{name}(\n    void\n);"
    head = f"{ret_type} {name}("
    parts = [head] if len(head) <= 80 else [ret_type, f"{name}("]
    rendered_args = [
        f"    {compact_pointer_spacing(f'{map_type(arg_type)} {arg_name}')}"
        for arg_name, arg_type in args
    ]
    for index, rendered_arg in enumerate(rendered_args):
        suffix = "," if index + 1 < len(rendered_args) else ""
        parts.append(f"{rendered_arg}{suffix}")
    parts.append(");")
    return "\n".join(parts)


def format_multiline_typedef(
    name: str, args: list[tuple[str, str]], ret: str | None
) -> str:
    ret_type = map_type(ret or "void")
    if not args:
        line = f"typedef {ret_type} (*{name})(void);"
        if len(line) <= 80:
            return line
        return f"typedef {ret_type}\n(*{name})(\n    void\n);"
    head = f"typedef {ret_type} (*{name})("
    parts = [head] if len(head) <= 80 else [f"typedef {ret_type}", f"(*{name})("]
    rendered_args = [
        f"    {compact_pointer_spacing(f'{map_type(arg_type)} {arg_name}')}"
        for arg_name, arg_type in args
    ]
    for index, rendered_arg in enumerate(rendered_args):
        suffix = "," if index + 1 < len(rendered_args) else ""
        parts.append(f"{rendered_arg}{suffix}")
    parts.append(") ;")
    return "\n".join(parts).replace(") ;", ");")


def format_forward_decl(name: str) -> str:
    line = f"typedef struct {name} {name};"
    if len(line) <= 80:
        return line
    return f"typedef struct {name}\n    {name};"


def format_define(name: str, ctype: str, value: str) -> str:
    line = f"#define {name} (({ctype}) {value})"
    if len(line) <= 80:
        return line
    return f"#define {name} \\\n    (({ctype}) {value})"


def collapse_blank_lines(lines: list[str]) -> list[str]:
    collapsed: list[str] = []
    previous_blank = False
    for line in lines:
        is_blank = line == ""
        if is_blank and previous_blank:
            continue
        collapsed.append(line)
        previous_blank = is_blank
    if collapsed and collapsed[-1] != "":
        collapsed.append("")
    return collapsed


def render_header(items: list[Item]) -> str:
    forward_names: set[str] = set()
    alias_items = []
    const_items = []
    enum_items = []
    struct_items = []
    typedef_items = []
    fn_items = []
    for item in items:
        if item.kind == "opaque":
            forward_names.add(item.name)
        elif item.kind == "alias":
            alias_items.append(item)
        elif item.kind == "const":
            const_items.append(item)
        elif item.kind == "enum":
            enum_items.append(item)
        elif item.kind == "struct":
            forward_names.add(item.name)
            struct_items.append(item)
        elif item.kind == "typedef_fn":
            typedef_items.append(item)
        elif item.kind == "function":
            fn_items.append(item)

    struct_items = order_struct_items(struct_items)

    parts = [
        "/*",
        " * Copyright The Pit Project Owners. All rights reserved.",
        " * SPDX-License-Identifier: Apache-2.0",
        " *",
        ' * Licensed under the Apache License, Version 2.0 (the "License");',
        " * you may not use this file except in compliance with the License.",
        " * You may obtain a copy of the License at",
        " *",
        " *     http://www.apache.org/licenses/LICENSE-2.0",
        " *",
        " * Unless required by applicable law or agreed to in writing,",
        " * software distributed under the License is distributed on an",
        ' * "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND,',
        " * either express or implied. See the License for the specific",
        " * language governing permissions and limitations under the",
        " * License.",
        " *",
        " * Generated file. Do not edit manually.",
        " * Please see https://github.com/openpitkit and the OWNERS file for details.",
        " */",
        "",
        "#ifndef OPENPIT_PIT_H",
        "#define OPENPIT_PIT_H",
        "",
        "#include <stdbool.h>",
        "#include <stddef.h>",
        "#include <stdint.h>",
        "",
        "#ifdef __cplusplus",
        'extern "C" {',
        "#endif",
        "",
    ]

    for name in sorted(forward_names):
        parts.append(format_forward_decl(name))
    if forward_names:
        parts.append("")

    for item in alias_items:
        parts.append(
            format_doc_comment(item.docs)
            + f"typedef {map_type(item.alias or '')} {item.name};"
        )
        parts.append("")

    for item in const_items:
        c_type = map_type(item.alias or "")
        value = map_const_value(item.value or "0")
        parts.append(
            format_doc_comment(item.docs) + format_define(item.name, c_type, value)
        )
        parts.append("")

    for item in enum_items:
        base = map_type(item.repr_name or "int32_t")
        parts.append(format_doc_comment(item.docs) + f"typedef {base} {item.name};")
        for variant, value, variant_docs in item.variants:
            if variant_docs:
                parts.append(format_doc_comment(variant_docs).rstrip())
            parts.append(format_define(f"{item.name}_{variant}", item.name, str(value)))
        parts.append("")

    for item in struct_items:
        if item.opaque:
            continue
        parts.append(format_doc_comment(item.docs) + f"struct {item.name} {{")
        for field_item in item.fields:
            field_doc = format_doc_comment(field_item.docs)
            if field_doc:
                for doc_line in field_doc.rstrip().splitlines():
                    parts.append(f"    {doc_line}" if doc_line else "")
            parts.append(f"    {format_field_decl(field_item)}")
        parts.append("};")
        parts.append("")

    for item in typedef_items:
        parts.append(
            format_doc_comment(item.docs)
            + format_multiline_typedef(item.name, item.args, item.ret)
        )
        parts.append("")

    for item in fn_items:
        parts.append(
            format_doc_comment(item.docs)
            + format_multiline_function(item.name, item.args, item.ret)
        )
        parts.append("")

    parts.extend(
        [
            "#ifdef __cplusplus",
            "}",
            "#endif",
            "",
            "#endif",
            "",
        ]
    )
    return "\n".join(parts)


def render_docs(items: list[Item], source_files: list[str]) -> str:
    grouped: dict[str, list[Item]] = {}
    for item in items:
        grouped.setdefault(item.section, []).append(item)

    outputs: dict[str, str] = {}
    index_lines = [
        "# Pit C API",
        "",
        f"- Header: `{HEADER_PATH.relative_to(ROOT)}`",
        "- Sections:",
    ]
    for source in source_files:
        section_items = grouped.get(source)
        if not section_items:
            continue
        slug, title = section_info(source)
        link = f"  - [{title}]({slug}.md)"
        if link not in index_lines:
            index_lines.append(link)
    index_lines.append("")
    outputs["index.md"] = "\n".join(collapse_blank_lines(index_lines))

    for source in source_files:
        section_items = grouped.get(source)
        if not section_items:
            continue
        slug, title = section_info(source)
        lines = [f"# {title}", "", "[Back to index](index.md)", ""]
        for item in section_items:
            if item.kind == "opaque" or item.opaque:
                declaration = format_forward_decl(item.name)
            elif item.kind == "const":
                declaration = format_define(
                    item.name,
                    map_type(item.alias or ""),
                    map_const_value(item.value or "0"),
                )
            elif item.kind == "alias":
                declaration = f"typedef {map_type(item.alias or '')} {item.name};"
            elif item.kind == "enum":
                enum_lines = [
                    f"typedef {map_type(item.repr_name or 'int32_t')} {item.name};"
                ]
                for variant, value, variant_docs in item.variants:
                    if variant_docs:
                        enum_lines.append(format_doc_comment(variant_docs).rstrip())
                    enum_lines.append(
                        format_define(f"{item.name}_{variant}", item.name, str(value))
                    )
                declaration = "\n".join(enum_lines)
            elif item.kind == "struct":
                fields = "\n".join(
                    f"    {format_field_decl(field_item)}" for field_item in item.fields
                )
                declaration = (
                    f"typedef struct {item.name} {{\n{fields}\n}} {item.name};"
                )
            elif item.kind == "typedef_fn":
                declaration = format_multiline_typedef(item.name, item.args, item.ret)
            else:
                declaration = format_multiline_function(item.name, item.args, item.ret)
            lines.append(f"## `{item.name}`")
            lines.append("")
            if item.docs:
                lines.extend(format_markdown_lines(item.docs))
                lines.append("")
            lines.append("```c")
            lines.append(declaration)
            lines.append("```")
            lines.append("")
        outputs[f"{slug}.md"] = "\n".join(collapse_blank_lines(lines))
    return outputs


if __name__ == "__main__":
    try:
        main()
    except (
        UnmappedRustTypeError,
        UnsupportedStructShapeError,
    ) as error:
        frame = traceback.extract_tb(error.__traceback__)[-1]
        raise SystemExit(f"{frame.filename}:{frame.lineno}: {error}") from None
