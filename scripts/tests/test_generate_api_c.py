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

import importlib.util
import sys
from pathlib import Path

C_API_SCRIPT_PATH = Path(__file__).resolve().parents[1] / "_generate_api_c_h.py"
DLSYM_SCRIPT_PATH = Path(__file__).resolve().parents[1] / "_generate_api_c_dlsym.py"
PARAM_RS_PATH = (
    Path(__file__).resolve().parents[2] / "crates" / "openpit-ffi" / "src" / "param.rs"
)
LAST_ERROR_RS_PATH = (
    Path(__file__).resolve().parents[2]
    / "crates"
    / "openpit-ffi"
    / "src"
    / "last_error.rs"
)


def load_module(path: Path, name: str):
    spec = importlib.util.spec_from_file_location(name, path)
    assert spec is not None
    assert spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


def load_c_api_module():
    return load_module(C_API_SCRIPT_PATH, "_generate_api_c_h")


def load_dlsym_module():
    return load_module(DLSYM_SCRIPT_PATH, "_generate_api_c_dlsym")


def collect_named_block(module, lines: list[str], prefix: str) -> str:
    start = next(
        index for index, line in enumerate(lines) if line.strip().startswith(prefix)
    )
    if prefix.startswith("macro_rules!"):
        block, _ = module.collect_braced(lines, start, "{", "}")
        return block
    block, _ = module.collect_macro_invocation(lines, start)
    return block


def test_decimal_wrapper_docs_expand_from_macro_source() -> None:
    module = load_c_api_module()
    lines = PARAM_RS_PATH.read_text(encoding="utf-8").splitlines()
    macro_block = collect_named_block(
        module, lines, "macro_rules! define_decimal_param_wrapper"
    )
    invocation_block = collect_named_block(
        module, lines, "define_decimal_param_wrapper!("
    )

    template = module.parse_decimal_wrapper_template(macro_block)
    wrapper_item, create_item, get_decimal_item = module.parse_decimal_wrapper(
        invocation_block, template
    )

    assert wrapper_item.docs == ["Validated `Pnl` value wrapper."]
    assert create_item.docs == [
        "Validates a decimal and returns a `Pnl` wrapper.",
        "",
        "Meaning: Profit and loss value; positive means profit, negative means loss.",
        "",
        "Success:",
        "- returns `true` and writes a validated wrapper to `out`.",
        "",
        "Error:",
        "- returns `false` when `out` is null or when the decimal does not satisfy the"
        " rules of this type;",
        "- on error read `out_error` for the message.",
    ]
    assert get_decimal_item.docs == ["Returns the decimal stored in `Pnl`."]


def test_parse_file_uses_macro_docs_for_decimal_wrappers() -> None:
    module = load_c_api_module()
    items = module.parse_file(PARAM_RS_PATH)
    docs_by_name = {item.name: item.docs for item in items}

    assert docs_by_name["OpenPitParamPnl"] == ["Validated `Pnl` value wrapper."]
    assert docs_by_name["openpit_create_param_pnl"][0] == (
        "Validates a decimal and returns a `Pnl` wrapper."
    )
    assert docs_by_name["openpit_create_param_pnl"][2] == (
        "Meaning: Profit and loss value; positive means profit, negative means loss."
    )
    assert docs_by_name["openpit_param_pnl_get_decimal"] == [
        "Returns the decimal stored in `Pnl`."
    ]


def test_parse_decimal_macro_ffi_uses_hardcoded_signatures() -> None:
    module = load_c_api_module()
    lines = PARAM_RS_PATH.read_text(encoding="utf-8").splitlines()
    macro_block = collect_named_block(
        module, lines, "macro_rules! define_decimal_param_ffi_common"
    )
    invocation_block = collect_named_block(
        module, lines, "define_decimal_param_ffi_common!("
    )
    specs = module.parse_macro_fn_specs(macro_block)
    items = module.parse_decimal_ffi_common(invocation_block, specs)
    by_name = {item.name: item for item in items}

    from_str_item = by_name["openpit_create_param_pnl_from_str"]
    assert from_str_item.ret == "bool"
    assert from_str_item.args == [
        ("value", "OpenPitStringView"),
        ("out", "*mut OpenPitParamPnl"),
        ("out_error", "OpenPitOutParamError"),
    ]

    checked_mul_f64_item = by_name["openpit_param_pnl_checked_mul_f64"]
    assert checked_mul_f64_item.ret == "bool"
    assert checked_mul_f64_item.args == [
        ("value", "OpenPitParamPnl"),
        ("multiplier", "f64"),
        ("out", "*mut OpenPitParamPnl"),
        ("out_error", "OpenPitOutParamError"),
    ]

    to_string_item = by_name["openpit_param_pnl_to_string"]
    assert to_string_item.ret == "*mut OpenPitSharedString"
    assert to_string_item.args == [
        ("value", "OpenPitParamPnl"),
        ("out_error", "OpenPitOutParamError"),
    ]


def test_parse_file_includes_pointer_alias_for_out_error() -> None:
    module = load_c_api_module()
    items = module.parse_file(LAST_ERROR_RS_PATH)
    by_name = {item.name: item for item in items}

    out_error_alias = by_name["OpenPitOutError"]
    assert out_error_alias.kind == "alias"
    assert out_error_alias.alias == "*mut *mut OpenPitSharedString"

    out_param_error_alias = by_name["OpenPitOutParamError"]
    assert out_param_error_alias.kind == "alias"
    assert out_param_error_alias.alias == "*mut *mut OpenPitParamError"


def test_collect_declarations_handles_multiline_and_split_return_type() -> None:
    module = load_dlsym_module()
    header = """
typedef bool (*OpenPitCallback)(void);
struct OpenPitIgnored;
#define openpit_ignored 1
const OpenPitSharedString *
openpit_return_shared_string(void);
bool openpit_create_param_pnl(
    OpenPitParamDecimal value,
    OpenPitParamPnl * out,
    OpenPitOutParamError out_error
);
"""

    declarations = module.collect_declarations(header)

    assert declarations == [
        "const OpenPitSharedString * openpit_return_shared_string(void)",
        (
            "bool openpit_create_param_pnl( OpenPitParamDecimal value, "
            "OpenPitParamPnl * out, OpenPitOutParamError out_error )"
        ),
    ]


def test_parse_dlsym_functions_splits_return_types_and_params() -> None:
    module = load_dlsym_module()
    header = """
OpenPitStringView openpit_get_runtime_version(void);
void openpit_destroy_shared_string(OpenPitSharedString * value);
bool openpit_write_value(OpenPitValue value, OpenPitValue * out);
"""

    functions = module.parse_dlsym_functions(header)

    assert functions == [
        ("OpenPitStringView", "openpit_get_runtime_version", [("void", None)]),
        (
            "void",
            "openpit_destroy_shared_string",
            [("OpenPitSharedString *", "value")],
        ),
        (
            "bool",
            "openpit_write_value",
            [("OpenPitValue", "value"), ("OpenPitValue *", "out")],
        ),
    ]


def test_render_dlsym_source_generates_init_and_forwarding_wrappers() -> None:
    module = load_dlsym_module()
    functions = [
        ("OpenPitStringView", "openpit_get_runtime_version", [("void", None)]),
        (
            "void",
            "openpit_destroy_shared_string",
            [("OpenPitSharedString *", "value")],
        ),
        (
            "bool",
            "openpit_write_value",
            [("OpenPitValue", "value"), ("OpenPitValue *", "out")],
        ),
    ]

    source = module.render_dlsym_source(functions)

    assert (
        "static OpenPitStringView (*_fn_openpit_get_runtime_version)(void) = NULL;"
        in source
    )
    assert (
        "_fn_openpit_write_value = "
        "(bool (*)(OpenPitValue, OpenPitValue *))"
        'openpit_dlsym(handle, "openpit_write_value");'
    ) in source
    assert (
        'if (_fn_openpit_write_value == NULL) return "openpit_write_value";' in source
    )
    assert "OpenPitStringView openpit_get_runtime_version(void) {" in source
    assert "    return _fn_openpit_get_runtime_version();" in source
    assert "void openpit_destroy_shared_string(OpenPitSharedString * value) {" in source
    assert "    _fn_openpit_destroy_shared_string(value);" in source
    assert (
        "bool openpit_write_value(OpenPitValue value, OpenPitValue * out) {" in source
    )
    assert "    return _fn_openpit_write_value(value, out);" in source


def test_generate_dlsym_writes_output(tmp_path: Path) -> None:
    module = load_dlsym_module()
    header_path = tmp_path / "openpit.h"
    output_path = tmp_path / "openpit_dlsym.c"
    header_path.write_text(
        "OpenPitStringView openpit_get_runtime_version(void);\n",
        encoding="utf-8",
    )

    module.generate(header_path, output_path)

    assert output_path.read_text(encoding="utf-8").endswith(
        "OpenPitStringView openpit_get_runtime_version(void) {\n"
        "    return _fn_openpit_get_runtime_version();\n"
        "}\n"
    )
