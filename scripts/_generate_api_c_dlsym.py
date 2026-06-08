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

import contextlib
import re
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
DLSYM_HEADER_PATH = ROOT / "bindings" / "go" / "internal" / "native" / "openpit.h"
DLSYM_OUTPUT_PATH = ROOT / "bindings" / "go" / "internal" / "native" / "openpit_dlsym.c"

DLSYM_LICENSE = """\
/*
 * Copyright The Pit Project Owners. All rights reserved.
 * SPDX-License-Identifier: Apache-2.0
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *     http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 *
 * Please see https://openpit.dev and the OWNERS file for details.
 *
 * Generated file. Do not edit manually.
 */"""

DLSYM_PLATFORM_PROLOGUE = """\
#include "openpit.h"

#ifdef _WIN32
#include <windows.h>
static void *openpit_dlsym(void *handle, const char *name) {
    return (void *)(uintptr_t)GetProcAddress((HMODULE)handle, name);
}
#else
#include <dlfcn.h>
static void *openpit_dlsym(void *handle, const char *name) {
    return dlsym(handle, name);
}
#endif"""


def previous_code_line(lines: list[str], before: int) -> str:
    for index in range(before - 1, -1, -1):
        line = lines[index].strip()
        if line and not line.startswith("*") and not line.startswith("/"):
            return line
    return ""


def collect_declarations(text: str) -> list[str]:
    lines = text.splitlines()
    declarations = []
    index = 0
    while index < len(lines):
        line = lines[index]
        if (
            re.match(r"^[A-Za-z]", line)
            and "openpit_" in line
            and not line.startswith("typedef")
            and not line.startswith("struct")
            and not line.startswith("#")
        ):
            prefix = ""
            if re.match(r"^openpit_", line):
                previous = previous_code_line(lines, index)
                if (
                    previous
                    and not previous.endswith("{")
                    and not previous.endswith("}")
                ):
                    prefix = previous + " "

            start = prefix + line
            if start.rstrip().endswith(");"):
                declarations.append(start.rstrip().rstrip(";"))
            else:
                parts = [start]
                index += 1
                while index < len(lines):
                    parts.append(lines[index])
                    if lines[index].rstrip() == ");":
                        break
                    index += 1
                joined = " ".join(part.strip() for part in parts).rstrip()
                if joined.endswith(";"):
                    joined = joined[:-1]
                declarations.append(joined)
        index += 1
    return declarations


def parse_decl(declaration: str) -> tuple[str, str, list[str]]:
    paren = declaration.index("(")
    before = declaration[:paren].strip()
    params_raw = declaration[paren + 1 :].rstrip().rstrip(")")
    parts = before.rsplit(None, 1)
    ret_type = parts[0].strip() if len(parts) == 2 else before
    name = parts[1].strip() if len(parts) == 2 else ""
    params = [param.strip() for param in params_raw.split(",") if param.strip()]
    return ret_type, name, params


def split_type_name(param: str) -> tuple[str, str | None]:
    param = param.strip()
    if not param or param == "void":
        return "void", None
    parts = param.rsplit(None, 1)
    if len(parts) == 1:
        return parts[0], None
    return parts[0].strip(), parts[1].strip()


def parse_dlsym_functions(
    header_text: str,
) -> list[tuple[str, str, list[tuple[str, str | None]]]]:
    functions = []
    for declaration in collect_declarations(header_text):
        ret_type, name, raw_params = parse_decl(declaration)
        if not name.startswith("openpit_"):
            continue
        params = [split_type_name(param) for param in raw_params]
        functions.append((ret_type, name, params))
    return functions


def dlsym_param_types(params: list[tuple[str, str | None]]) -> str:
    if not params or (len(params) == 1 and params[0] == ("void", None)):
        return "void"
    return ", ".join(param_type for param_type, _ in params)


def dlsym_signature_params(params: list[tuple[str, str | None]]) -> tuple[str, str]:
    if not params or (len(params) == 1 and params[0] == ("void", None)):
        return "void", ""

    sig_parts = []
    call_parts = []
    for param_type, param_name in params:
        sig_parts.append(f"{param_type} {param_name}" if param_name else param_type)
        if param_name:
            call_parts.append(param_name)
    return ", ".join(sig_parts), ", ".join(call_parts)


def render_dlsym_source(
    functions: list[tuple[str, str, list[tuple[str, str | None]]]],
) -> str:
    out = [DLSYM_LICENSE, "", DLSYM_PLATFORM_PROLOGUE, ""]
    out += [
        "/* Function pointers resolved via openpit_dlsym after the runtime"
        " is loaded. */",
    ]

    for ret_type, name, params in functions:
        ptr_types = dlsym_param_types(params)
        out.append(f"static {ret_type} (*_fn_{name})({ptr_types}) = NULL;")

    out += [
        "",
        "/*",
        " * Resolves every function pointer by name from the given runtime handle.",
        " * Returns NULL on success; on failure returns the name of the first symbol",
        " * that could not be resolved (the pointer references a static string",
        " * literal that lives for the lifetime of the process).",
        " */",
        "const char *openpit_native_init(void *handle) {",
    ]
    for ret_type, name, params in functions:
        ptr_types = dlsym_param_types(params)
        cast = f"({ret_type} (*)({ptr_types}))"
        out += [
            f'    _fn_{name} = {cast}openpit_dlsym(handle, "{name}");',
            f'    if (_fn_{name} == NULL) return "{name}";',
        ]
    out += [
        "    return NULL;",
        "}",
    ]

    for ret_type, name, params in functions:
        sig_params, call_args = dlsym_signature_params(params)
        ret_stmt = "" if ret_type.strip() == "void" else "return "
        out += [
            "",
            f"{ret_type} {name}({sig_params}) {{",
            f"    {ret_stmt}_fn_{name}({call_args});",
            "}",
        ]
    return "\n".join(out) + "\n"


def generate(
    header_path: Path = DLSYM_HEADER_PATH, output_path: Path = DLSYM_OUTPUT_PATH
) -> None:
    functions = parse_dlsym_functions(header_path.read_text(encoding="utf-8"))
    output_path.write_text(render_dlsym_source(functions), encoding="utf-8")
    with contextlib.suppress(ValueError):
        output_path = output_path.relative_to(ROOT)
    print(f"Generated {len(functions)} wrappers -> {output_path}")
