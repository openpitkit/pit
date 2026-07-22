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
# Please see https://openpit.dev and the OWNERS file for details.

from __future__ import annotations

import ast
import contextlib
import re
import shutil
import sys
import textwrap
from collections.abc import Iterator, Mapping
from dataclasses import dataclass, field
from html import escape
from pathlib import Path
from urllib.parse import urlsplit, urlunsplit

from markdown_it import MarkdownIt
from markdown_it.token import Token

ROOT = Path(__file__).resolve().parents[1]
SRC_DIR = ROOT / "crates" / "openpit-ffi" / "src"
HEADER_PATH = ROOT / "bindings" / "c" / "openpit.h"
HEADER_COPIES = [
    ROOT / "bindings" / "go" / "internal" / "native" / "openpit.h",
]
SITE_DIR = ROOT / "docs"
DOCS_DIR = SITE_DIR / "c-api"
PARTIALS_DIR = SITE_DIR / "partials"
# Base of the generated documentation site. It is a subdomain of its own: the
# marketing landing page keeps openpit.dev, everything generated here is
# published under docs.openpit.dev. Every generator that emits an absolute
# self-reference (canonical links, sitemap, robots.txt) reads this constant so
# the two can never drift.
SITE_BASE_URL = "https://docs.openpit.dev"
SOCIAL_PREVIEW_URL = "https://openpit.dev/assets/openpit-social-preview.png"
SOCIAL_PREVIEW_ALT = "OpenPit - Pre-trade Integrity Toolkit"
SOCIAL_PREVIEW_WIDTH = 1200
SOCIAL_PREVIEW_HEIGHT = 201
OPENPIT_LEVERAGE_RS = ROOT / "crates" / "openpit" / "src" / "param" / "leverage.rs"


def clean_html_url(url: str) -> str:
    """Return the public clean URL for an on-disk HTML target.

    Queries and fragments survive link rewriting. Canonical builders pass only
    paths, so those components never enter a canonical URL accidentally.
    """
    parts = urlsplit(url)
    if not parts.path.endswith(".html"):
        return url
    path = parts.path.removesuffix(".html")
    if path == "index":
        path = "./"
    elif path.endswith("/index"):
        path = path.removesuffix("index")
    return urlunsplit((parts.scheme, parts.netloc, path, parts.query, parts.fragment))


@dataclass(frozen=True)
class ApiSection:
    """One generated reference published under its own site directory."""

    slug: str
    title: str
    summary: str

    @property
    def url(self) -> str:
        return f"{SITE_BASE_URL}/{self.slug}/"

    @property
    def site_path(self) -> str:
        return f"/{self.slug}/"


# The section this generator publishes. It is named on its own because every
# self-reference the C API pages emit - the canonical URLs and the per-page
# back link - reads its path from here instead of spelling "c-api" again.
C_API_SECTION = ApiSection(
    "c-api",
    "C API",
    "C header reference for the OpenPit C ABI, split per section.",
)

# The single source for what the documentation site publishes. Every consumer
# reads it - the site-root navigation, the assembled tree, robots.txt, llms.txt
# and the sitemap - so a section cannot reach one of them and miss the others.
API_SECTIONS: tuple[ApiSection, ...] = (
    C_API_SECTION,
    ApiSection(
        "cpp-api",
        "C++ API",
        "C++ SDK reference for the OpenPit::openpit CMake package.",
    ),
    ApiSection(
        "js-api",
        "JavaScript/TypeScript API",
        "Package and subpath reference for @openpit/engine.",
    ),
)

# Public addresses of the C API pages carry no ".html": Cloudflare Pages serves
# every page at its extensionless path and redirects the suffixed form to it.
# Only this generator may rely on that - the Doxygen and TypeDoc trees write
# suffixed links into their own output, so those sections keep the suffix.
C_API_BASE_URL = f"{SITE_BASE_URL}{C_API_SECTION.site_path}"

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
    "policy": ("policies", "Policies"),
    "lib.rs": ("runtime", "Runtime and Errors"),
}
PARAMS_RUNTIME_DUPLICATES = {
    "OpenPitParamErrorCode",
    "OpenPitParamError",
    "OpenPitOutParamError",
    "openpit_destroy_param_error",
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
        ("value", "OpenPitParamDecimal"),
        ("out", "*mut $wrapper"),
        ("out_error", "OpenPitOutParamError"),
    ],
    "bool",
)

DECIMAL_PARAM_WRAPPER_GET_DECIMAL_SIGNATURE = (
    [("value", "$wrapper")],
    "OpenPitParamDecimal",
)

DECIMAL_PARAM_FFI_COMMON_SIGNATURES: dict[
    str, tuple[list[tuple[str, str]], str | None]
] = {
    "from_string_fn": (
        [
            ("value", "OpenPitStringView"),
            ("out", "*mut $wrapper"),
            ("out_error", "OpenPitOutParamError"),
        ],
        "bool",
    ),
    "from_f64_fn": (
        [
            ("value", "f64"),
            ("out", "*mut $wrapper"),
            ("out_error", "OpenPitOutParamError"),
        ],
        "bool",
    ),
    "from_int64_fn": (
        [
            ("value", "i64"),
            ("out", "*mut $wrapper"),
            ("out_error", "OpenPitOutParamError"),
        ],
        "bool",
    ),
    "from_uint64_fn": (
        [
            ("value", "u64"),
            ("out", "*mut $wrapper"),
            ("out_error", "OpenPitOutParamError"),
        ],
        "bool",
    ),
    "from_string_rounded_fn": (
        [
            ("value", "OpenPitStringView"),
            ("scale", "u32"),
            ("rounding", "OpenPitParamRoundingStrategy"),
            ("out", "*mut $wrapper"),
            ("out_error", "OpenPitOutParamError"),
        ],
        "bool",
    ),
    "from_f64_rounded_fn": (
        [
            ("value", "f64"),
            ("scale", "u32"),
            ("rounding", "OpenPitParamRoundingStrategy"),
            ("out", "*mut $wrapper"),
            ("out_error", "OpenPitOutParamError"),
        ],
        "bool",
    ),
    "from_decimal_rounded_fn": (
        [
            ("value", "OpenPitParamDecimal"),
            ("scale", "u32"),
            ("rounding", "OpenPitParamRoundingStrategy"),
            ("out", "*mut $wrapper"),
            ("out_error", "OpenPitOutParamError"),
        ],
        "bool",
    ),
    "to_f64_fn": (
        [
            ("value", "$wrapper"),
            ("out", "*mut f64"),
            ("out_error", "OpenPitOutParamError"),
        ],
        "bool",
    ),
    "is_zero_fn": (
        [
            ("value", "$wrapper"),
            ("out", "*mut bool"),
            ("out_error", "OpenPitOutParamError"),
        ],
        "bool",
    ),
    "compare_fn": (
        [
            ("lhs", "$wrapper"),
            ("rhs", "$wrapper"),
            ("out", "*mut i8"),
            ("out_error", "OpenPitOutParamError"),
        ],
        "bool",
    ),
    "to_string_fn": (
        [("value", "$wrapper"), ("out_error", "OpenPitOutParamError")],
        "*mut OpenPitSharedString",
    ),
    "checked_add_fn": (
        [
            ("lhs", "$wrapper"),
            ("rhs", "$wrapper"),
            ("out", "*mut $wrapper"),
            ("out_error", "OpenPitOutParamError"),
        ],
        "bool",
    ),
    "checked_sub_fn": (
        [
            ("lhs", "$wrapper"),
            ("rhs", "$wrapper"),
            ("out", "*mut $wrapper"),
            ("out_error", "OpenPitOutParamError"),
        ],
        "bool",
    ),
    "checked_mul_i64_fn": (
        [
            ("value", "$wrapper"),
            ("multiplier", "i64"),
            ("out", "*mut $wrapper"),
            ("out_error", "OpenPitOutParamError"),
        ],
        "bool",
    ),
    "checked_mul_u64_fn": (
        [
            ("value", "$wrapper"),
            ("multiplier", "u64"),
            ("out", "*mut $wrapper"),
            ("out_error", "OpenPitOutParamError"),
        ],
        "bool",
    ),
    "checked_mul_f64_fn": (
        [
            ("value", "$wrapper"),
            ("multiplier", "f64"),
            ("out", "*mut $wrapper"),
            ("out_error", "OpenPitOutParamError"),
        ],
        "bool",
    ),
    "checked_div_i64_fn": (
        [
            ("value", "$wrapper"),
            ("divisor", "i64"),
            ("out", "*mut $wrapper"),
            ("out_error", "OpenPitOutParamError"),
        ],
        "bool",
    ),
    "checked_div_u64_fn": (
        [
            ("value", "$wrapper"),
            ("divisor", "u64"),
            ("out", "*mut $wrapper"),
            ("out_error", "OpenPitOutParamError"),
        ],
        "bool",
    ),
    "checked_div_f64_fn": (
        [
            ("value", "$wrapper"),
            ("divisor", "f64"),
            ("out", "*mut $wrapper"),
            ("out_error", "OpenPitOutParamError"),
        ],
        "bool",
    ),
    "checked_rem_i64_fn": (
        [
            ("value", "$wrapper"),
            ("divisor", "i64"),
            ("out", "*mut $wrapper"),
            ("out_error", "OpenPitOutParamError"),
        ],
        "bool",
    ),
    "checked_rem_u64_fn": (
        [
            ("value", "$wrapper"),
            ("divisor", "u64"),
            ("out", "*mut $wrapper"),
            ("out_error", "OpenPitOutParamError"),
        ],
        "bool",
    ),
    "checked_rem_f64_fn": (
        [
            ("value", "$wrapper"),
            ("divisor", "f64"),
            ("out", "*mut $wrapper"),
            ("out_error", "OpenPitOutParamError"),
        ],
        "bool",
    ),
}

DECIMAL_PARAM_FFI_SIGNED_SIGNATURES: dict[
    str, tuple[list[tuple[str, str]], str | None]
] = {
    "checked_neg_fn": (
        [
            ("value", "$wrapper"),
            ("out", "*mut $wrapper"),
            ("out_error", "OpenPitOutParamError"),
        ],
        "bool",
    )
}


class UnmappedRustTypeError(ValueError):
    pass


class UnsupportedStructShapeError(ValueError):
    pass


class UnsupportedDocMarkupError(ValueError):
    """A doc comment uses markup the reference pages cannot represent."""


class MissingSitePartialError(Exception):
    """A shared ``docs/partials`` fragment the page shell needs is missing."""


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
ENUM_DISCRIMINANT_CACHE: dict[tuple[str, str, str], int] = {}


def _display_path(path: Path) -> Path:
    # Progress lines show repo-relative paths; fall back to the absolute path
    # when generating into a tree outside the repo root (e.g. under test).
    try:
        return path.relative_to(ROOT)
    except ValueError:
        return path


def build_items() -> tuple[list[Item], list[str]]:
    source_files = list_source_files()
    items = []
    for rel in source_files:
        section_path = SRC_DIR / rel
        if section_path.is_dir():
            paths = sorted(section_path.glob("*.rs"))
        else:
            paths = [section_path]
        for path in paths:
            parsed = parse_file(path)
            for item in parsed:
                item.section = rel
            items.extend(parsed)

    return dedupe_items(items), source_files


def generate_headers() -> None:
    """Write the FFI C header and its Go-native copy.

    This is the FFI-artifact half of the generator. It never touches the docs
    tree, so the local delivery gate stays free of generated documentation.
    """
    items, _source_files = build_items()
    header = render_header(items)

    HEADER_PATH.parent.mkdir(parents=True, exist_ok=True)
    HEADER_PATH.write_text(header, encoding="utf-8", newline="\n")
    print(f"Generated {_display_path(HEADER_PATH)}")

    for dest in HEADER_COPIES:
        try:
            dest.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(HEADER_PATH, dest)
            print(f"Copied {_display_path(HEADER_PATH)} -> {_display_path(dest)}")
        except Exception as e:
            print(
                (
                    f"error: failed to copy {_display_path(HEADER_PATH)}"
                    f" to {_display_path(dest)}: {e}"
                ),
                file=sys.stderr,
            )
            sys.exit(1)


def generate_docs() -> None:
    """Write the C API HTML reference under ``docs/c-api``.

    This is the documentation half of the generator. The site is built and
    published only from the CI pipeline, never by the local gate.
    """
    items, source_files = build_items()
    docs = render_docs(items, source_files)

    DOCS_DIR.mkdir(parents=True, exist_ok=True)
    # Both extensions are cleared: the reference used to be Markdown, so a
    # switched-over tree must not keep serving the stale pages.
    for pattern in ("*.md", "*.html"):
        for path in DOCS_DIR.glob(pattern):
            path.unlink()
    for rel_path, text in docs.items():
        path = DOCS_DIR / rel_path
        path.write_text(text, encoding="utf-8", newline="\n")
        print(f"Generated {_display_path(path)}")


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
        return item.name.startswith("OPENPIT_")
    if item.kind == "function":
        return item.name.startswith("openpit_")
    return item.name.startswith("OpenPit")


def list_source_files() -> list[str]:
    keys = [path.name for path in SRC_DIR.glob("*.rs") if path.is_file()]
    for entry in SRC_DIR.iterdir():
        if entry.is_dir() and (entry / "mod.rs").exists():
            keys.append(entry.name)
    section_order = {name: idx for idx, name in enumerate(SECTION_INFO)}
    return sorted(
        keys, key=lambda name: (section_order.get(name, len(section_order)), name)
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
            if struct_name_match and not struct_name_match.group(1).startswith(
                "OpenPit"
            ):
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
        if stripped.startswith("pub union "):
            union_name_match = re.match(r"pub union (\w+)", stripped)
            if union_name_match and not union_name_match.group(1).startswith("OpenPit"):
                _block, i = collect_item(lines, i)
                docs = []
                attrs = []
                continue
            block, i = collect_item(lines, i)
            item = parse_union(block, docs, attrs)
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
        if stripped.startswith("pub use ") and " as " in stripped:
            block, i = collect_until_semicolon(lines, i)
            item = parse_use_reexport_as(block, docs, attrs)
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


def parse_union(block: str, docs: list[str], attrs: list[str]) -> Item | None:
    header = block.splitlines()[0].strip()
    match = re.match(r"pub union (\w+)\s*\{", header)
    if not match:
        raise UnsupportedStructShapeError(f"unsupported union declaration: `{header}`")
    name = match.group(1)
    field_docs: list[str] = []
    fields: list[Field] = []
    # A field type may span several source lines (e.g. a long type wrapped onto
    # the next line). Accumulate `pub name: type,` across lines until the
    # trailing comma closes the field.
    pending: str | None = None
    for line in block.splitlines()[1:]:
        stripped = line.strip()
        if pending is None:
            if stripped in {"}", "};"}:
                break
            if stripped.startswith("///"):
                field_docs.append(stripped[3:].lstrip())
                continue
            if stripped.startswith("#["):
                continue
            if not stripped or not stripped.startswith("pub "):
                continue
            pending = stripped
        else:
            pending = f"{pending} {stripped}"
        if not pending.endswith(","):
            continue
        field_match = re.match(r"pub (\w+):\s*(.+),$", " ".join(pending.split()))
        if field_match:
            fields.append(
                Field(field_match.group(1), field_match.group(2).strip(), field_docs)
            )
            field_docs = []
        pending = None
    repr_name = parse_repr(attrs)
    return Item(
        kind="union",
        name=name,
        docs=list(docs),
        attrs=list(attrs),
        fields=fields,
        opaque=repr_name is None,
        repr_name=repr_name,
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
                current_value = parse_enum_discriminant(variant_match.group(2).strip())
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


def parse_enum_discriminant(value: str) -> int:
    compact = re.sub(r"\s+", "", value)
    path_cast_match = re.match(
        r"((?:[A-Za-z_]\w*::)+)([A-Za-z_]\w*)as(?:u32|u64|usize)$",
        compact,
    )
    if path_cast_match:
        path_prefix = path_cast_match.group(1).split("::")
        if path_prefix and path_prefix[-1] == "":
            path_prefix = path_prefix[:-1]
        variant = path_cast_match.group(2)
        resolved = resolve_enum_path_discriminant(path_prefix, variant)
        if resolved is not None:
            return resolved
    normalized = compact.replace("_", "")
    if normalized == "u32::MAX":
        return 2**32 - 1
    return int(normalized, 0)


def resolve_enum_path_discriminant(path_parts: list[str], variant: str) -> int | None:
    if len(path_parts) < 3:
        return None
    crate_name = path_parts[0]
    enum_name = path_parts[-1]
    module_parts = path_parts[1:-1]
    key = (crate_name, "::".join(module_parts), f"{enum_name}::{variant}")
    if key in ENUM_DISCRIMINANT_CACHE:
        return ENUM_DISCRIMINANT_CACHE[key]

    if crate_name != "openpit":
        return None
    crate_dir_name = crate_name
    src_root = ROOT / "crates" / crate_dir_name / "src"
    module_rs = src_root.joinpath(*module_parts).with_suffix(".rs")
    module_mod_rs = src_root.joinpath(*module_parts) / "mod.rs"
    module_path = module_rs if module_rs.exists() else module_mod_rs
    if not module_path.exists():
        return None

    text = module_path.read_text(encoding="utf-8")
    match = re.search(rf"pub enum {re.escape(enum_name)}\s*\{{(.*?)\n\}}", text, re.S)
    if not match:
        return None

    body = match.group(1)
    current_value = -1
    for raw_line in body.splitlines():
        stripped = raw_line.strip()
        if not stripped or stripped.startswith("//") or stripped.startswith("///"):
            continue
        if stripped.startswith("#["):
            continue
        variant_match = re.match(r"(\w+)(?: = ([^,]+))?,", stripped)
        if not variant_match:
            continue
        if variant_match.group(2):
            current_value = parse_enum_discriminant(variant_match.group(2).strip())
        else:
            current_value += 1
        found_variant = variant_match.group(1)
        ENUM_DISCRIMINANT_CACHE[
            (crate_name, "::".join(module_parts), f"{enum_name}::{found_variant}")
        ] = current_value

    return ENUM_DISCRIMINANT_CACHE.get(key)


def parse_use_reexport_as(block: str, docs: list[str], attrs: list[str]) -> Item | None:
    normalized = " ".join(block.split())
    match = re.match(r"pub use (\w+)::(.+) as (\w+);", normalized)
    if not match:
        return None
    crate_name, type_path, alias_name = match.group(1), match.group(2), match.group(3)
    if not alias_name.startswith("OpenPit"):
        return None
    return resolve_reexported_enum(crate_name, type_path, alias_name, docs, attrs)


def resolve_reexported_enum(
    crate_name: str,
    type_path: str,
    alias_name: str,
    docs: list[str],
    attrs: list[str],
) -> Item | None:
    crate_src = ROOT / "crates" / crate_name.replace("_", "-") / "src"
    if not crate_src.exists():
        return None
    type_name = type_path.split("::")[-1]
    for candidate in sorted(crate_src.rglob("*.rs")):
        text = candidate.read_text(encoding="utf-8")
        if f"pub enum {type_name}" not in text:
            continue
        enum_match = re.search(rf"pub enum {re.escape(type_name)}\s*\{{", text)
        if not enum_match:
            continue
        start = text.rfind("\n", 0, enum_match.start()) + 1
        block_lines = text[start:].splitlines()
        block, _ = collect_braced(block_lines, 0, "{", "}")
        pre_text = text[: enum_match.start()]
        source_attrs: list[str] = []
        source_docs: list[str] = []
        for pre_line in reversed(pre_text.splitlines()[-20:]):
            stripped = pre_line.strip()
            if stripped.startswith("#["):
                source_attrs.insert(0, stripped)
            elif stripped.startswith("///"):
                source_docs.insert(0, stripped[3:].lstrip())
            elif stripped and not stripped.startswith("//"):
                break
        item = parse_enum(
            block,
            list(docs) if docs else source_docs,
            source_attrs or list(attrs),
        )
        if item:
            item.name = alias_name
        return item
    return None


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
            fields=[Field("_0", "OpenPitParamDecimal")],
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
            f"failed to extract define_decimal_param_wrapper docs for {missing_list}"
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
    rhs = rhs.strip()
    # A nullable callback is declared as `Option<extern "C" fn(..)>`; in C this
    # is just an ordinary (null-able) function pointer, so unwrap the `Option`
    # before parsing the underlying signature.
    option_match = re.fullmatch(r"Option<\s*(.+?)\s*,?\s*>", rhs)
    if option_match:
        rhs = option_match.group(1).strip()
    match = re.match(r"(?:unsafe )?extern \"C\" fn\((.*)\)\s*(?:-> (.+))?", rhs)
    if not match:
        return [], None
    ret = match.group(2)
    if ret is not None:
        ret = ret.strip().rstrip(",").strip()
    return parse_args(match.group(1)), (ret if ret else None)


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
    m = re.fullmatch(r"Option<(OpenPit\w+Fn)>", rust_type)
    if m:
        return m.group(1)
    if rust_type in RUST_TO_C:
        return RUST_TO_C[rust_type]
    c_name = rust_type_name(rust_type)
    if not c_name.startswith("OpenPit"):
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


def format_field_decl_lines(field: Field, indent: int = 4) -> list[str]:
    """Render a field declaration as indented C lines.

    A declaration whose indented single-line form exceeds 80 columns wraps the
    field name (and any trailing array dimension) onto the next line at twice
    the base indent, matching the body style used for long struct/union fields.
    """
    pad = " " * indent
    array_match = re.match(r"\[(.+);\s*([^\]]+)\]", field.rust_type)
    if array_match:
        c_type = map_type(array_match.group(1))
        suffix = f"[{array_match.group(2).strip()}]"
    else:
        c_type = map_type(field.rust_type)
        suffix = ""
    single = f"{pad}{c_type} {field.name}{suffix};"
    if len(single) <= 80:
        return [single]
    return [f"{pad}{c_type}", f"{pad}{pad}{field.name}{suffix};"]


def format_aggregate_typedef(item: Item) -> str:
    """Render a struct/union as a named `typedef ... {` block for the docs."""
    keyword = "union" if item.kind == "union" else "struct"
    head = f"typedef {keyword} {item.name} {{"
    if len(head) > 80:
        head = f"typedef {keyword}\n    {item.name} {{"
    body_lines: list[str] = []
    for field_item in item.fields:
        body_lines.extend(format_field_decl_lines(field_item))
    body = "\n".join(body_lines)
    return f"{head}\n{body}\n}} {item.name};"


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
    return {base_name} if base_name.startswith("OpenPit") else set()


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
    lines = normalize_doc_lines(lines)
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


def format_forward_decl(name: str, kind: str = "struct") -> str:
    keyword = "union" if kind == "union" else "struct"
    line = f"typedef {keyword} {name} {name};"
    if len(line) <= 80:
        return line
    head = f"typedef {keyword} {name}"
    if len(head) <= 80:
        return f"{head}\n    {name};"
    return f"typedef {keyword}\n    {name}\n    {name};"


def format_define(name: str, ctype: str, value: str) -> str:
    line = f"#define {name} (({ctype}) {value})"
    if len(line) <= 80:
        return line
    return f"#define {name} \\\n    (({ctype}) {value})"


def render_header(items: list[Item]) -> str:
    # Maps a forward-declared aggregate name to its C kind ("struct" or
    # "union") so the forward decl emits the matching `typedef struct`/
    # `typedef union`. Opaque handles are always structs.
    forward_kinds: dict[str, str] = {}
    alias_items = []
    const_items = []
    enum_items = []
    struct_items = []
    typedef_items = []
    fn_items = []
    for item in items:
        if item.kind == "opaque":
            forward_kinds[item.name] = "struct"
        elif item.kind == "alias":
            alias_items.append(item)
        elif item.kind == "const":
            const_items.append(item)
        elif item.kind == "enum":
            enum_items.append(item)
        elif item.kind in {"struct", "union"}:
            forward_kinds[item.name] = item.kind
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
        " * Please see https://openpit.dev and the OWNERS file for details.",
        " *",
        " * Generated file. Do not edit manually.",
        " */",
        "",
        "#ifndef OPENPIT_H",
        "#define OPENPIT_H",
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

    for name in sorted(forward_kinds):
        parts.append(format_forward_decl(name, forward_kinds[name]))
    if forward_kinds:
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
        keyword = "union" if item.kind == "union" else "struct"
        parts.append(format_doc_comment(item.docs) + f"{keyword} {item.name} {{")
        for field_item in item.fields:
            field_doc = format_doc_comment(field_item.docs)
            if field_doc:
                for doc_line in field_doc.rstrip().splitlines():
                    parts.append(f"    {doc_line}" if doc_line else "")
            parts.extend(format_field_decl_lines(field_item))
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


# The site has no build step of its own: the shared look comes from
# docs/assets/styles.css, and these rules cover only the content elements that
# the landing page does not have. Every content page needs them, so they are
# emitted from one template scoped to the page kind (".api" for the generated
# reference, ".md" for the rendered README) instead of being kept twice.
CONTENT_CSS_TEMPLATE = """\
    {scope} {
      flex: 1;
    }

    /* Every page kind opens with one page title, so the rule lives here rather
       than with the per-kind chrome: without it the browser default applies. */
    {scope} h1 {
      font-size: 1.6rem;
      font-weight: 700;
      letter-spacing: -0.02em;
    }

    {scope} p,
    {scope} li {
      font-size: 0.8125rem;
      color: var(--muted-lt);
      line-height: 1.65;
    }

    {scope} p {
      margin-bottom: 10px;
    }

    {scope} ul,
    {scope} ol {
      margin: 0 0 10px 1.2rem;
    }

    {scope} code {
      color: var(--accent);
    }

    {scope} a {
      color: var(--accent);
    }

    {scope} a:hover {
      text-decoration: underline;
    }

    {scope} pre {
      background: var(--surface);
      border: 1px solid var(--border);
      border-radius: 6px;
      padding: 14px 16px;
      margin-bottom: 14px;
      overflow-x: auto;
      font-size: 0.75rem;
      line-height: 1.55;
    }

    {scope} pre code {
      color: var(--text);
    }

    {scope} blockquote {
      border-left: 2px solid var(--border);
      padding-left: 12px;
      margin-bottom: 10px;
    }

    {scope} table {
      border-collapse: collapse;
      margin-bottom: 14px;
    }

    {scope} th,
    {scope} td {
      border: 1px solid var(--border);
      padding: 6px 10px;
      font-size: 0.8125rem;
      line-height: 1.65;
      text-align: left;
    }

    {scope} th {
      color: var(--text);
    }

    {scope} td {
      color: var(--muted-lt);
    }"""

# Reference-page chrome: the page title, the per-symbol headings, and the
# declaration block. The content rules above cover everything else.
API_PAGE_CSS = """\
    .api h2 {
      font-size: 0.9375rem;
      font-weight: 700;
      letter-spacing: -0.01em;
      margin-bottom: 10px;
    }

    .api h3,
    .api h4,
    .api h5,
    .api h6 {
      font-weight: 700;
      letter-spacing: 0.07em;
      text-transform: uppercase;
      color: var(--muted);
      margin: 14px 0 6px;
    }

    /* Doc-comment headings nest under the symbol heading, so the levels are
       sized apart: shared styling alone would flatten the hierarchy. */
    .api h3 {
      font-size: 0.8125rem;
    }

    .api h4 {
      font-size: 0.75rem;
    }

    .api h5 {
      font-size: 0.6875rem;
    }

    .api h6 {
      font-size: 0.625rem;
    }

    /* The symbol heading is its own permalink. The marker stays out of the way
       until the heading is hovered or reached by keyboard, which would
       otherwise repeat on every symbol of the page. */
    .api .api-anchor::after {
      content: " #";
      color: var(--muted);
      opacity: 0;
    }

    .api .api-anchor:hover::after,
    .api .api-anchor:focus-visible::after {
      opacity: 1;
    }

    .api .api-anchor:focus-visible {
      outline: 2px solid var(--accent);
      outline-offset: 3px;
      border-radius: 3px;
    }

    .api-nav {
      font-size: 0.75rem;
      color: var(--muted);
      margin: 8px 0 28px;
    }

    .api-sections {
      list-style: none;
    }

    .api-item {
      padding: 24px 0;
      border-top: 1px solid var(--border);
    }

    /* The declaration closes its section, so it drops the shared bottom
       margin; the selector must outweigh the shared ".api pre" rule. */
    .api pre.api-decl {
      margin-bottom: 0;
    }"""


def render_content_css(scope: str) -> str:
    """Return the shared content rules scoped to one page kind."""
    return CONTENT_CSS_TEMPLATE.replace("{scope}", scope)


DOC_PAGE_CSS = f"{render_content_css('.api')}\n\n{API_PAGE_CSS}"

PAGE_CONTAINER_STYLE = (
    "max-width: 860px; margin: 0 auto; padding: 48px 24px 48px;"
    " display: flex; flex-direction: column; flex: 1;"
)

FONTS_HREF = (
    "https://fonts.googleapis.com/css2?family=JetBrains+Mono"
    ":wght@400;500;700&amp;display=swap"
)


def escape_text(value: str) -> str:
    """Escape a value for an HTML text node.

    Quotes stay literal: no generated text is inlined into an attribute, and
    escaped quotes only make the doc prose harder to read in the source.
    """
    return escape(value, quote=False)


def walk_tokens(tokens: list[Token]) -> Iterator[Token]:
    for token in tokens:
        yield token
        if token.children:
            yield from walk_tokens(list(token.children))


def build_doc_markdown() -> MarkdownIt:
    """Return the parser the FFI doc comments are written against.

    The comments are CommonMark plus tables. Raw HTML stays disabled: a doc
    comment that mentions a tag means the text, so it is escaped rather than
    injected into the page.
    """
    return MarkdownIt("commonmark", {"html": False}).enable("table")


# A doc-comment heading is a subsection of the per-symbol "<h2>", so "# Safety"
# has to land on "<h3>" rather than on a second page title.
DOC_HEADING_OFFSET = 2

# Everything the reference pages know how to present. Anything else would be
# published without styling or, worse, as literal Markdown source, so it is
# rejected instead. Strikethrough is deliberately absent: the parser enables
# only tables on top of CommonMark, so "~~text~~" is prose, and listing tokens
# the parser cannot emit would advertise support that does not exist.
SUPPORTED_DOC_TOKENS: frozenset[str] = frozenset(
    {
        "blockquote_close",
        "blockquote_open",
        "bullet_list_close",
        "bullet_list_open",
        "code_inline",
        "em_close",
        "em_open",
        "fence",
        "heading_close",
        "heading_open",
        "hr",
        "inline",
        "link_close",
        "link_open",
        "list_item_close",
        "list_item_open",
        "ordered_list_close",
        "ordered_list_open",
        "paragraph_close",
        "paragraph_open",
        "softbreak",
        "strong_close",
        "strong_open",
        "table_close",
        "table_open",
        "tbody_close",
        "tbody_open",
        "td_close",
        "td_open",
        "text",
        "th_close",
        "th_open",
        "thead_close",
        "thead_open",
        "tr_close",
        "tr_open",
    }
)

# Link targets that are real URLs. Everything else in a doc comment is a Rust
# intra-doc path such as "openpit::Foo" or "OpenPitErrorKind::Variant", which
# no browser can follow.
DOC_LINK_URL_PREFIXES = ("https://", "http://", "mailto:", "/", "#")

# The shortcut intra-doc form rustdoc resolves without a target, "[`Foo`]" or
# "[Foo]". CommonMark leaves it as literal text because no link reference
# definition backs it, so the brackets are matched here instead.
DOC_SHORTCUT_LINK_RE = re.compile(r"\[([A-Za-z_]\w*(?:::\w+)*)\]")


def resolve_doc_link(href: str, symbol_hrefs: Mapping[str, str]) -> str | None:
    """Return the published href for a doc-comment link target.

    URLs pass through. A Rust intra-doc path resolves through the leftmost of
    its segments that names a documented C API symbol, so a path to a member
    lands on the page of the type that owns it. When nothing matches there is
    no page to point at, and ``None`` tells the caller to keep the link text
    alone rather than publish a broken href.
    """
    if href.startswith(DOC_LINK_URL_PREFIXES):
        return href
    for segment in href.split("::"):
        target = symbol_hrefs.get(segment.strip())
        if target is not None:
            return target
    return None


def _drop_token_markup(token: Token) -> None:
    """Turn a token into markup-free text, keeping the surrounding content."""
    token.type = "text"
    token.tag = ""
    token.attrs = {}
    token.content = ""


def _apply_doc_links(children: list[Token], symbol_hrefs: Mapping[str, str]) -> None:
    dropped: list[bool] = []
    for token in children:
        if token.type == "link_open":
            target = resolve_doc_link(token.attrGet("href") or "", symbol_hrefs)
            if target is None:
                _drop_token_markup(token)
                dropped.append(True)
                continue
            token.attrSet("href", target)
            dropped.append(False)
        elif token.type == "link_close" and dropped and dropped.pop():
            _drop_token_markup(token)


def _text_token(content: str) -> Token:
    token = Token("text", "", 0)
    token.content = content
    return token


def _shortcut_link_tokens(
    label: list[Token], path: str, symbol_hrefs: Mapping[str, str]
) -> list[Token]:
    """Wrap a shortcut link label in an anchor, or return it unwrapped.

    An unresolved shortcut keeps its label and loses the brackets, the same way
    an unresolved inline link does: the reference has no page to point at, and
    the brackets are Rust syntax rather than something the reader should see.
    """
    target = resolve_doc_link(path, symbol_hrefs)
    if target is None:
        return label
    link_open = Token("link_open", "a", 1)
    link_open.attrSet("href", target)
    return [link_open, *label, Token("link_close", "a", -1)]


def _expand_text_shortcut_links(
    token: Token, symbol_hrefs: Mapping[str, str]
) -> list[Token]:
    """Resolve the bracket-only shortcut form inside one text token."""
    out: list[Token] = []
    position = 0
    for match in DOC_SHORTCUT_LINK_RE.finditer(token.content):
        if match.start() > position:
            out.append(_text_token(token.content[position : match.start()]))
        path = match.group(1)
        out.extend(
            _shortcut_link_tokens([_text_token(path)], path, symbol_hrefs),
        )
        position = match.end()
    if not out:
        return [token]
    if position < len(token.content):
        out.append(_text_token(token.content[position:]))
    return out


def _expand_doc_shortcut_links(
    children: list[Token], symbol_hrefs: Mapping[str, str]
) -> list[Token]:
    """Resolve shortcut intra-doc links across one inline token stream.

    It runs after the inline links are resolved, so the hrefs written here are
    never fed back through the intra-doc resolver. Only text tokens are
    rewritten: a bracketed name inside a code span is sample text, not a link.
    """
    out: list[Token] = []
    pending = list(children)
    while pending:
        token = pending.pop(0)
        if token.type != "text":
            out.append(token)
            continue
        # "[`Foo`]" reaches the stream as a text token ending in "[", the code
        # span, and a text token starting with "]".
        if (
            token.content.endswith("[")
            and len(pending) >= 2
            and pending[0].type == "code_inline"
            and pending[1].type == "text"
            and pending[1].content.startswith("]")
        ):
            code = pending.pop(0)
            tail = pending.pop(0)
            if len(token.content) > 1:
                out.append(_text_token(token.content[:-1]))
            out.extend(_shortcut_link_tokens([code], code.content, symbol_hrefs))
            tail.content = tail.content[1:]
            if tail.content:
                pending.insert(0, tail)
            continue
        out.extend(_expand_text_shortcut_links(token, symbol_hrefs))
    return out


def _shift_doc_headings(tokens: list[Token]) -> None:
    for token in tokens:
        if token.type in {"heading_open", "heading_close"}:
            level = min(int(token.tag[1:]) + DOC_HEADING_OFFSET, 6)
            token.tag = f"h{level}"


def _doc_owner(symbol: str | None, source_file: str | None) -> str:
    """Name the doc comment a failure comes from.

    The generator's own traceback says nothing about which comment has to be
    rewritten, so the symbol and the Rust file that declares it are part of the
    message whenever the caller knows them.
    """
    if symbol is None and source_file is None:
        return "a doc comment"
    if source_file is None:
        return f"the doc comment on {symbol}"
    if symbol is None:
        return f"a doc comment in {source_file}"
    return f"the doc comment on {symbol} ({source_file})"


def _reject_unsupported_doc_markup(
    tokens: list[Token],
    source: str,
    symbol: str | None,
    source_file: str | None,
) -> None:
    for token in walk_tokens(tokens):
        if token.type not in SUPPORTED_DOC_TOKENS:
            raise UnsupportedDocMarkupError(
                f"{_doc_owner(symbol, source_file)} uses {token.type!r}, which"
                f" the reference pages cannot render; rewrite it:\n{source}"
            )


def render_doc_html(
    lines: list[str],
    symbol_hrefs: Mapping[str, str] | None = None,
    *,
    symbol: str | None = None,
    source_file: str | None = None,
) -> list[str]:
    """Render doc-comment Markdown as the HTML lines of a reference page.

    ``symbol_hrefs`` maps a documented C API symbol to its published location
    and resolves the Rust intra-doc links the comments carry. ``symbol`` and
    ``source_file`` name the comment being rendered so a rejected markup error
    points at it. Markup is emitted unindented: whitespace added inside a
    ``<pre>`` block would show up in the published code samples.
    """
    source = "\n".join(normalize_doc_lines(lines)).strip()
    if not source:
        return []
    parser = build_doc_markdown()
    env: dict[str, object] = {}
    tokens = parser.parse(source, env)
    _reject_unsupported_doc_markup(tokens, source, symbol, source_file)
    _shift_doc_headings(tokens)
    hrefs = symbol_hrefs or {}
    for token in tokens:
        if token.type == "inline" and token.children:
            _apply_doc_links(token.children, hrefs)
            token.children = _expand_doc_shortcut_links(token.children, hrefs)
    return parser.renderer.render(tokens, parser.options, env).splitlines()


def read_site_partial(name: str, indent: str) -> list[str]:
    """Inline a shared ``docs/partials`` fragment.

    The site is served as static files with no include mechanism, so the
    fragments are copied in at generation time and remain the single source
    for the shared chrome.
    """
    path = PARTIALS_DIR / name
    if not path.is_file():
        raise MissingSitePartialError(
            f"{path} is missing; the generated pages inline it as shared chrome"
        )
    text = path.read_text(encoding="utf-8")
    return [f"{indent}{line}" if line.strip() else "" for line in text.splitlines()]


def render_doc_page(
    title: str,
    canonical_url: str | None,
    body: list[str],
    *,
    description: str | None = None,
    robots: str | None = None,
    json_ld: str | None = None,
    css: str = DOC_PAGE_CSS,
) -> str:
    """Wrap page body markup in the shared documentation-site page shell.

    Every page of the site - the generated C API reference and the rendered
    README index alike - goes through here, so the head, the inlined header and
    footer partials, and the asset paths stay identical across the site. Only
    the page-local stylesheet differs per page kind.

    Each head metadatum is optional and carries no default: a page that has
    nothing truthful to say about itself says nothing. ``canonical_url`` is left
    out by a page that must not claim one, such as the 404 page; ``description``
    also gates the social-card tags, which would otherwise advertise an invented
    summary; ``robots`` is emitted only to restrict indexing; and ``json_ld`` is
    the structured-data block the calling module attaches to its index pages.
    """
    escaped_title = escape(title, quote=True)
    metadata: list[str] = []
    if description is not None:
        escaped_description = escape(description, quote=True)
        metadata.extend(
            [
                f'  <meta name="description" content="{escaped_description}" />',
                f'  <meta property="og:title" content="{escaped_title}" />',
                f'  <meta property="og:description"'
                f' content="{escaped_description}" />',
                '  <meta property="og:type" content="website" />',
                '  <meta property="og:site_name" content="OpenPit" />',
                '  <meta property="og:locale" content="en_US" />',
                f'  <meta property="og:image" content="{SOCIAL_PREVIEW_URL}" />',
                f'  <meta property="og:image:width"'
                f' content="{SOCIAL_PREVIEW_WIDTH}" />',
                f'  <meta property="og:image:height"'
                f' content="{SOCIAL_PREVIEW_HEIGHT}" />',
                f'  <meta property="og:image:alt" content="{SOCIAL_PREVIEW_ALT}" />',
                '  <meta name="twitter:card" content="summary_large_image" />',
                f'  <meta name="twitter:title" content="{escaped_title}" />',
                f'  <meta name="twitter:description"'
                f' content="{escaped_description}" />',
                f'  <meta name="twitter:image" content="{SOCIAL_PREVIEW_URL}" />',
                f'  <meta name="twitter:image:alt" content="{SOCIAL_PREVIEW_ALT}" />',
            ]
        )
    if canonical_url is not None:
        escaped_canonical = escape(canonical_url, quote=True)
        metadata.append(f'  <link rel="canonical" href="{escaped_canonical}" />')
        if description is not None:
            metadata.append(
                f'  <meta property="og:url" content="{escaped_canonical}" />'
            )
    if robots is not None:
        metadata.append(
            f'  <meta name="robots" content="{escape(robots, quote=True)}" />'
        )
    structured_data: list[str] = []
    if json_ld is not None:
        structured_data = [
            '  <script type="application/ld+json">',
            json_ld,
            "  </script>",
        ]

    lines = [
        "<!doctype html>",
        '<html lang="en">',
        "",
        "<head>",
        '  <meta charset="utf-8" />',
        '  <meta name="viewport" content="width=device-width, initial-scale=1.0" />',
        f"  <title>{escape_text(title)}</title>",
        *metadata,
        '  <link rel="preconnect" href="https://fonts.googleapis.com" />',
        '  <link rel="preconnect" href="https://fonts.gstatic.com" crossorigin />',
        f'  <link href="{FONTS_HREF}" rel="stylesheet" />',
        '  <link rel="stylesheet" href="/assets/styles.css" />',
        '  <link rel="icon" href="/favicon.ico" sizes="any" />',
        '  <link rel="icon" type="image/svg+xml" href="/favicon-light.svg"'
        ' media="(prefers-color-scheme: light)" />',
        '  <link rel="icon" type="image/svg+xml" href="/favicon-dark.svg"'
        ' media="(prefers-color-scheme: dark)" />',
        '  <meta name="theme-color" content="#f6f3ec"'
        ' media="(prefers-color-scheme: light)" />',
        '  <meta name="theme-color" content="#0a0d12"'
        ' media="(prefers-color-scheme: dark)" />',
        *structured_data,
        "  <style>",
        css,
        "  </style>",
        "</head>",
        "",
        "<body>",
        f'  <div class="container-tight" style="{PAGE_CONTAINER_STYLE}">',
        "",
        '    <header style="text-align: center; margin-bottom: 28px;">',
        *read_site_partial("header.html", "      "),
        "    </header>",
        "",
        *body,
        "",
        *read_site_partial("footer.html", "    "),
        "",
        "  </div>",
        "</body>",
        "",
        "</html>",
        "",
    ]
    return "\n".join(lines)


def format_item_declaration(item: Item) -> str:
    if item.kind == "opaque" or item.opaque:
        return format_forward_decl(item.name)
    if item.kind == "const":
        return format_define(
            item.name,
            map_type(item.alias or ""),
            map_const_value(item.value or "0"),
        )
    if item.kind == "alias":
        return f"typedef {map_type(item.alias or '')} {item.name};"
    if item.kind == "enum":
        enum_lines = [f"typedef {map_type(item.repr_name or 'int32_t')} {item.name};"]
        for variant, value, variant_docs in item.variants:
            if variant_docs:
                enum_lines.append(format_doc_comment(variant_docs).rstrip())
            enum_lines.append(
                format_define(f"{item.name}_{variant}", item.name, str(value))
            )
        return "\n".join(enum_lines)
    if item.kind in {"struct", "union"}:
        return format_aggregate_typedef(item)
    if item.kind == "typedef_fn":
        return format_multiline_typedef(item.name, item.args, item.ret)
    return format_multiline_function(item.name, item.args, item.ret)


def _section_page_items(
    slug: str, sections_by_slug: dict[str, tuple[str, list[Item]]]
) -> list[Item]:
    """Return the items one section page documents.

    The parameter page repeats the few runtime symbols its functions report
    errors through, so a reader never has to leave the page to look them up.
    """
    _title, section_items = sections_by_slug[slug]
    if slug != "params":
        return list(section_items)
    runtime_section = sections_by_slug.get("runtime")
    if runtime_section is None:
        return list(section_items)
    _runtime_title, runtime_items = runtime_section
    extra = [item for item in runtime_items if item.name in PARAMS_RUNTIME_DUPLICATES]
    return [*section_items, *extra]


def symbol_anchor(name: str, used: set[str]) -> str:
    """Return the heading id a symbol is deep-linked by.

    The symbol name is the anchor, which keeps the fragments the Markdown
    reference used to publish. Characters an HTML id cannot carry are folded to
    "-", and a collision after that folding gets a numeric suffix, so every
    heading on a page stays addressable by exactly one fragment.
    """
    base = re.sub(r"[^A-Za-z0-9_.-]", "-", name) or "symbol"
    anchor = base
    suffix = 2
    while anchor in used:
        anchor = f"{base}-{suffix}"
        suffix += 1
    used.add(anchor)
    return anchor


def legacy_symbol_anchor(anchor: str, used: set[str]) -> str | None:
    """Return the former Kramdown fragment when it does not collide.

    The Markdown reference lowercased heading fragments. Exact symbol-case IDs
    remain canonical, while this hidden alias keeps old incoming links alive.
    A real symbol whose exact ID is already that lowercase value wins because
    HTML cannot carry duplicate IDs.
    """
    legacy = anchor.lower()
    if legacy == anchor or legacy in used:
        return None
    used.add(legacy)
    return legacy


def render_docs(items: list[Item], source_files: list[str]) -> dict[str, str]:
    grouped: dict[str, list[Item]] = {}
    for item in items:
        grouped.setdefault(item.section, []).append(item)
    sections_by_slug: dict[str, tuple[str, list[Item]]] = {}
    section_order: list[str] = []
    for source in source_files:
        section_items = grouped.get(source)
        if not section_items:
            continue
        slug, title = section_info(source)
        if slug not in sections_by_slug:
            sections_by_slug[slug] = (title, [])
            section_order.append(slug)
        sections_by_slug[slug][1].extend(section_items)

    page_items = {
        slug: _section_page_items(slug, sections_by_slug) for slug in section_order
    }
    # Anchors are assigned for the whole reference before any page is rendered:
    # a doc comment on one page routinely links to a symbol documented on
    # another.
    anchors: dict[str, dict[str, str]] = {}
    legacy_anchors: dict[str, dict[str, str]] = {}
    symbol_hrefs: dict[str, str] = {}
    for slug in section_order:
        used: set[str] = set()
        page_anchors = {
            item.name: symbol_anchor(item.name, used) for item in page_items[slug]
        }
        page_legacy_anchors = {
            name: legacy
            for name, anchor in page_anchors.items()
            if (legacy := legacy_symbol_anchor(anchor, used)) is not None
        }
        anchors[slug] = page_anchors
        legacy_anchors[slug] = page_legacy_anchors
        for name, anchor in page_anchors.items():
            symbol_hrefs.setdefault(name, f"{slug}#{anchor}")

    outputs: dict[str, str] = {}
    index_body = [
        '    <main class="api">',
        "      <h1>OpenPit C API</h1>",
        f'      <p class="api-nav"><a href="{SITE_BASE_URL}/">'
        "Back to the documentation index</a></p>",
        f"      <p>Header: <code>{escape_text(HEADER_PATH.name)}</code></p>",
        "      <h2>Sections</h2>",
        '      <ul class="api-sections">',
    ]
    for slug in section_order:
        title, _section_items = sections_by_slug[slug]
        index_body.append(f'        <li><a href="{slug}">{escape_text(title)}</a></li>')
    index_body.extend(["      </ul>", "    </main>"])
    outputs["index.html"] = render_doc_page(
        "OpenPit C API",
        C_API_BASE_URL,
        index_body,
        description="C header reference for the OpenPit C ABI, split by API section.",
    )

    for slug in section_order:
        title, _section_items = sections_by_slug[slug]
        lines = [
            '    <main class="api">',
            f"      <h1>{escape_text(title)}</h1>",
            f'      <p class="api-nav"><a href="{C_API_SECTION.site_path}">'
            "Back to the C API index</a></p>",
        ]
        for item in page_items[slug]:
            declaration = format_item_declaration(item)
            anchor = anchors[slug][item.name]
            lines.append('      <section class="api-item">')
            legacy_anchor = legacy_anchors[slug].get(item.name)
            if legacy_anchor is not None:
                lines.append(
                    f'        <a id="{escape(legacy_anchor)}"'
                    ' class="legacy-anchor" aria-hidden="true"></a>'
                )
            # The heading links to itself: a page carries hundreds of symbols,
            # so every one of them has to be addressable on its own.
            lines.append(
                f'        <h2 id="{escape(anchor)}">'
                f'<a class="api-anchor" href="#{escape(anchor)}">'
                f"<code>{escape_text(item.name)}</code></a></h2>"
            )
            lines.extend(
                render_doc_html(
                    item.docs,
                    symbol_hrefs,
                    symbol=item.name,
                    source_file=item.section or None,
                )
            )
            lines.append(
                f'        <pre class="api-decl"><code>{escape_text(declaration)}'
                "</code></pre>"
            )
            lines.append("      </section>")
        lines.append("    </main>")
        outputs[f"{slug}.html"] = render_doc_page(
            f"{title} - OpenPit C API",
            f"{C_API_BASE_URL}{slug}",
            lines,
            description=f"{title} reference for the OpenPit C API.",
        )
    return outputs
