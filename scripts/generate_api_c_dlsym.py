#!/usr/bin/env python3
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
#
# Generates openpit_dlsym.c from openpit.h.
# Run via: go generate ./internal/native/

import re
import sys
from pathlib import Path

LICENSE = """\
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
 * Please see https://github.com/openpitkit and the OWNERS file for details.
 *
 * Generated file. Do not edit manually.
 * Regenerate with: go generate ./internal/native/
 */"""


PLATFORM_PROLOGUE = """\
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


def _prev_code_line(lines: list, before: int) -> str:
    """Return the nearest non-blank, non-comment line before index `before`."""
    for j in range(before - 1, -1, -1):
        s = lines[j].strip()
        if s and not s.startswith("*") and not s.startswith("/"):
            return s
    return ""


def collect_declarations(text: str) -> list:
    lines = text.splitlines()
    decls = []
    i = 0
    while i < len(lines):
        line = lines[i]
        if (
            re.match(r"^[A-Za-z]", line)
            and "openpit_" in line
            and not line.startswith("typedef")
            and not line.startswith("struct")
            and not line.startswith("#")
        ):
            # When the function name starts at column 0, the return type may
            # be on the previous line (e.g. "const Foo *\nopenpit_bar(...)").
            prefix = ""
            if re.match(r"^openpit_", line):
                prev = _prev_code_line(lines, i)
                if prev and not prev.endswith("{") and not prev.endswith("}"):
                    prefix = prev + " "

            start = prefix + line
            if start.rstrip().endswith(");"):
                decls.append(start.rstrip().rstrip(";"))
            else:
                parts = [start]
                i += 1
                while i < len(lines):
                    parts.append(lines[i])
                    if lines[i].rstrip() == ");":
                        break
                    i += 1
                joined = " ".join(p.strip() for p in parts).rstrip()
                if joined.endswith(";"):
                    joined = joined[:-1]
                decls.append(joined)
        i += 1
    return decls


def parse_decl(decl: str):
    paren = decl.index("(")
    before = decl[:paren].strip()
    params_raw = decl[paren + 1 :].rstrip().rstrip(")")
    parts = before.rsplit(None, 1)
    ret_type = parts[0].strip() if len(parts) == 2 else before
    name = parts[1].strip() if len(parts) == 2 else ""
    params = [p.strip() for p in params_raw.split(",") if p.strip()]
    return ret_type, name, params


def split_type_name(param: str):
    param = param.strip()
    if not param or param == "void":
        return "void", None
    parts = param.rsplit(None, 1)
    if len(parts) == 1:
        return parts[0], None
    return parts[0].strip(), parts[1].strip()


def generate(header_path: Path, output_path: Path) -> None:
    text = header_path.read_text()
    decls = collect_declarations(text)

    functions = []
    for decl in decls:
        ret_type, name, raw_params = parse_decl(decl)
        if not name.startswith("openpit_"):
            continue
        params = [split_type_name(p) for p in raw_params]
        functions.append((ret_type, name, params))

    out = [LICENSE, "", PLATFORM_PROLOGUE, ""]
    out += [
        "/* Function pointers resolved via openpit_dlsym after the runtime"
        " is loaded. */",
    ]

    for ret_type, name, params in functions:
        void_only = not params or (len(params) == 1 and params[0] == ("void", None))
        ptr_types = "void" if void_only else ", ".join(t for t, _ in params)
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
        void_only = not params or (len(params) == 1 and params[0] == ("void", None))
        ptr_types = "void" if void_only else ", ".join(t for t, _ in params)
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
        void_only = not params or (len(params) == 1 and params[0] == ("void", None))
        if void_only:
            sig_params = "void"
            call_args = ""
        else:
            sig_parts, call_parts = [], []
            for t, n in params:
                sig_parts.append(f"{t} {n}" if n else t)
                if n:
                    call_parts.append(n)
            sig_params = ", ".join(sig_parts)
            call_args = ", ".join(call_parts)

        ret_stmt = "" if ret_type.strip() == "void" else "return "
        out += [
            "",
            f"{ret_type} {name}({sig_params}) {{",
            f"    {ret_stmt}_fn_{name}({call_args});",
            "}",
        ]

    output_path.write_text("\n".join(out) + "\n")
    print(f"Generated {len(functions)} wrappers -> {output_path}", file=sys.stderr)


if __name__ == "__main__":
    if len(sys.argv) == 3:
        generate(Path(sys.argv[1]), Path(sys.argv[2]))
    else:
        native = (
            Path(__file__).parent.parent / "bindings" / "go" / "internal" / "native"
        )
        generate(native / "openpit.h", native / "openpit_dlsym.c")
