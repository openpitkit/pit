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

import importlib.util
import re
import runpy
import subprocess
import sys
from pathlib import Path

import pytest

CLI_SCRIPT_PATH = Path(__file__).resolve().parents[1] / "generate_api_c.py"
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


def load_cli_module():
    load_c_api_module()
    load_dlsym_module()
    return load_module(CLI_SCRIPT_PATH, "generate_api_c")


def record_generator_calls(module, monkeypatch) -> list[str]:
    """Replace every generator entry point with a call recorder."""
    calls: list[str] = []
    monkeypatch.setattr(
        module.header, "generate_headers", lambda: calls.append("headers")
    )
    monkeypatch.setattr(module.header, "generate_docs", lambda: calls.append("docs"))
    monkeypatch.setattr(
        module.dlsym, "generate", lambda *args, **kwargs: calls.append("dlsym")
    )
    return calls


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

    from_string_item = by_name["openpit_create_param_pnl_from_string"]
    assert from_string_item.ret == "bool"
    assert from_string_item.args == [
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


def test_parse_fn_pointer_unwraps_nullable_option_callback() -> None:
    module = load_c_api_module()

    bare = 'extern "C" fn(user_data: *mut c_void) -> bool'
    nullable = (
        "Option<\n"
        '    extern "C" fn(\n'
        "        user_data: *mut c_void,\n"
        "        out_account_group_id: *mut OpenPitParamAccountGroupId,\n"
        "    ) -> bool,\n"
        ">"
    )

    bare_args, bare_ret = module.parse_fn_pointer(" ".join(bare.split()))
    assert bare_args == [("user_data", "*mut c_void")]
    assert bare_ret == "bool"

    nullable_args, nullable_ret = module.parse_fn_pointer(" ".join(nullable.split()))
    assert nullable_args == [
        ("user_data", "*mut c_void"),
        ("out_account_group_id", "*mut OpenPitParamAccountGroupId"),
    ]
    assert nullable_ret == "bool"


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


def test_generate_headers_writes_ffi_artifacts_without_docs(
    tmp_path: Path, monkeypatch
) -> None:
    module = load_c_api_module()
    header_path = tmp_path / "openpit.h"
    go_copy = tmp_path / "go" / "openpit.h"
    docs_dir = tmp_path / "c-api"
    monkeypatch.setattr(module, "HEADER_PATH", header_path)
    monkeypatch.setattr(module, "HEADER_COPIES", [go_copy])
    monkeypatch.setattr(module, "DOCS_DIR", docs_dir)

    module.generate_headers()

    assert header_path.read_text(encoding="utf-8").startswith("/*")
    assert go_copy.read_text(encoding="utf-8") == header_path.read_text(
        encoding="utf-8"
    )
    # The FFI-artifact path must never touch the documentation tree.
    assert not docs_dir.exists()


def test_generate_docs_writes_c_api_html_without_header(
    tmp_path: Path, monkeypatch
) -> None:
    module = load_c_api_module()
    header_path = tmp_path / "openpit.h"
    docs_dir = tmp_path / "c-api"
    monkeypatch.setattr(module, "HEADER_PATH", header_path)
    monkeypatch.setattr(module, "DOCS_DIR", docs_dir)

    module.generate_docs()

    index = (docs_dir / "index.html").read_text(encoding="utf-8")
    section_pages = [path for path in docs_dir.glob("*.html") if path.stem != "index"]
    assert section_pages, "expected at least one C API section page"
    assert not list(docs_dir.glob("*.md"))
    assert index.startswith("<!doctype html>\n")
    assert '<link rel="canonical" href="https://docs.openpit.dev/c-api/" />' in index
    assert '<link rel="stylesheet" href="/assets/styles.css" />' in index
    # The shared chrome is inlined from docs/partials, not linked.
    assert "Pre-trade Integrity Toolkit" in index
    for page in section_pages:
        text = page.read_text(encoding="utf-8")
        canonical = f"https://docs.openpit.dev/c-api/{page.stem}"
        assert f'<link rel="canonical" href="{canonical}" />' in text
        assert (
            f'<a href="{module.C_API_SECTION.site_path}">'
            "Back to the C API index</a>" in text
        )
        assert f'<li><a href="{page.stem}">' in index
    # The documentation path must never write the FFI header.
    assert not header_path.exists()


def test_generate_docs_removes_stale_markdown_pages(
    tmp_path: Path, monkeypatch
) -> None:
    module = load_c_api_module()
    docs_dir = tmp_path / "c-api"
    docs_dir.mkdir()
    stale_markdown = docs_dir / "orders.md"
    stale_markdown.write_text("# Orders\n", encoding="utf-8")
    stale_html = docs_dir / "removed-section.html"
    stale_html.write_text("<!doctype html>\n", encoding="utf-8")
    monkeypatch.setattr(module, "HEADER_PATH", tmp_path / "openpit.h")
    monkeypatch.setattr(module, "DOCS_DIR", docs_dir)

    module.generate_docs()

    assert not stale_markdown.exists()
    assert not stale_html.exists()
    assert (docs_dir / "orders.html").is_file()


def test_render_doc_html_maps_markdown_subset_and_escapes_text() -> None:
    module = load_c_api_module()
    lines = [
        "Compares <a> & <b> markers.",
        "",
        "# Safety",
        "",
        "The `ptr` field must be non-null.",
        "",
        "- first bullet with `code`;",
        "- second <b> bullet.",
        "",
        "Trailing `unbalanced paragraph.",
    ]

    rendered = module.render_doc_html(lines)

    assert rendered == [
        "<p>Compares &lt;a&gt; &amp; &lt;b&gt; markers.</p>",
        # A doc-comment heading is a subsection of the per-symbol heading.
        "<h3>Safety</h3>",
        "<p>The <code>ptr</code> field must be non-null.</p>",
        "<ul>",
        "<li>first bullet with <code>code</code>;</li>",
        "<li>second &lt;b&gt; bullet.</li>",
        "</ul>",
        "<p>Trailing `unbalanced paragraph.</p>",
    ]


def test_render_doc_html_escapes_inline_code_content() -> None:
    module = load_c_api_module()

    assert module.render_doc_html(["Pass `a < b && c` here."]) == [
        "<p>Pass <code>a &lt; b &amp;&amp; c</code> here.</p>"
    ]
    assert module.render_doc_html([]) == []


def test_render_doc_html_renders_fenced_code_verbatim() -> None:
    module = load_c_api_module()
    lines = ["Example:", "", "```c", "if (a < b) {", "    call();", "}", "```"]

    rendered = module.render_doc_html(lines)

    assert rendered == [
        "<p>Example:</p>",
        '<pre><code class="language-c">if (a &lt; b) {',
        "    call();",
        "}",
        "</code></pre>",
    ]


def test_render_doc_html_renders_tables_quotes_and_ordered_lists() -> None:
    module = load_c_api_module()
    lines = [
        "| kind | meaning |",
        "| ---- | ------- |",
        "| `0`  | ok      |",
        "",
        "> Reserved for future use.",
        "",
        "1. first step;",
        "2. second step.",
        "",
        "* starred bullet.",
    ]

    rendered = "\n".join(module.render_doc_html(lines))

    assert "<table>" in rendered
    assert "<th>kind</th>" in rendered
    assert "<td><code>0</code></td>" in rendered
    assert "<blockquote>" in rendered
    assert "<ol>\n<li>first step;</li>" in rendered
    assert "<ul>\n<li>starred bullet.</li>" in rendered
    # Nothing may reach the page as literal Markdown source.
    assert "| kind |" not in rendered
    assert "1. first step" not in rendered


def test_render_doc_html_resolves_intra_doc_links_to_published_pages() -> None:
    module = load_c_api_module()
    symbol_hrefs = {"OpenPitConfigureErrorKind": "runtime#OpenPitConfigureErrorKind"}
    lines = [
        "Reported with the [`Validation`](OpenPitConfigureErrorKind::Validation)"
        " kind, see [the site](https://openpit.dev/).",
    ]

    rendered = "\n".join(module.render_doc_html(lines, symbol_hrefs))

    assert (
        '<a href="runtime#OpenPitConfigureErrorKind"><code>Validation</code></a>'
        in rendered
    )
    assert '<a href="https://openpit.dev/">the site</a>' in rendered


def test_render_doc_html_drops_unresolvable_intra_doc_links() -> None:
    module = load_c_api_module()
    lines = ["Mirrors [`SpotFundsOverrideTarget`](openpit::SpotFundsOverrideTarget)."]

    rendered = "\n".join(module.render_doc_html(lines, {}))

    # No page documents the Rust item, so the link text stays but the broken
    # href never reaches the page.
    assert rendered == "<p>Mirrors <code>SpotFundsOverrideTarget</code>.</p>"
    assert "href" not in rendered
    assert "openpit::" not in rendered


def test_render_doc_html_rejects_markup_it_cannot_publish() -> None:
    module = load_c_api_module()

    with pytest.raises(module.UnsupportedDocMarkupError, match="image"):
        module.render_doc_html(["See ![diagram](diagram.png)."])


def test_generate_docs_anchors_every_symbol_heading(
    tmp_path: Path, monkeypatch
) -> None:
    module = load_c_api_module()
    docs_dir = tmp_path / "c-api"
    monkeypatch.setattr(module, "HEADER_PATH", tmp_path / "openpit.h")
    monkeypatch.setattr(module, "DOCS_DIR", docs_dir)

    module.generate_docs()

    orders = (docs_dir / "orders.html").read_text(encoding="utf-8")
    assert (
        '<a id="openpitorder" class="legacy-anchor" aria-hidden="true"></a>' in orders
    )
    assert (
        '<h2 id="OpenPitOrder"><a class="api-anchor" href="#OpenPitOrder">'
        "<code>OpenPitOrder</code></a></h2>" in orders
    )
    ids = re.findall(r'\bid="([^"]+)"', orders)
    assert ids, "expected per-symbol headings"
    assert len(ids) == len(set(ids))


def test_symbol_anchor_is_unique_and_attribute_safe() -> None:
    module = load_c_api_module()
    used: set[str] = set()

    assert module.symbol_anchor("OpenPitOrder", used) == "OpenPitOrder"
    assert module.symbol_anchor("OpenPitOrder", used) == "OpenPitOrder-2"
    assert module.symbol_anchor('bad" name', used) == "bad--name"


def test_clean_html_url_handles_indexes_queries_and_fragments() -> None:
    module = load_c_api_module()

    assert module.clean_html_url("orders.html") == "orders"
    assert module.clean_html_url("orders.html?q=1#OpenPitOrder") == (
        "orders?q=1#OpenPitOrder"
    )
    assert module.clean_html_url("index.html") == "./"
    assert module.clean_html_url("nested/index.html#top") == "nested/#top"
    assert module.clean_html_url("../index.html?view=all") == "../?view=all"
    assert module.clean_html_url("/index.html") == "/"


def test_cli_docs_mode_generates_docs_only(monkeypatch) -> None:
    module = load_cli_module()
    calls = record_generator_calls(module, monkeypatch)

    module.main(mode="docs")

    # The dlsym stub is an FFI artifact and must stay out of the docs run.
    assert calls == ["docs"]


def test_cli_reports_missing_site_partial_without_traceback(monkeypatch) -> None:
    header_module = load_c_api_module()
    load_dlsym_module()

    def fail_docs() -> None:
        raise header_module.MissingSitePartialError("missing header.html")

    monkeypatch.setattr(header_module, "generate_docs", fail_docs)
    monkeypatch.setattr(sys, "argv", [str(CLI_SCRIPT_PATH), "--docs"])

    with pytest.raises(SystemExit, match=r"^[^\n]+: missing header\.html$") as error:
        runpy.run_path(str(CLI_SCRIPT_PATH), run_name="__main__")

    assert "Traceback" not in str(error.value)


def test_cli_default_and_headers_mode_generate_ffi_artifacts_only(monkeypatch) -> None:
    module = load_cli_module()
    calls = record_generator_calls(module, monkeypatch)

    module.main()
    module.main(mode="headers")

    assert calls == ["headers", "dlsym", "headers", "dlsym"]


def test_cli_dlsym_mode_generates_stub_only(monkeypatch) -> None:
    module = load_cli_module()
    calls = record_generator_calls(module, monkeypatch)

    module.main(mode="dlsym")

    assert calls == ["dlsym"]


def test_cli_rejects_combined_mode_flags() -> None:
    # argparse rejects the combination before any generator runs, so this
    # never touches the repository tree.
    result = subprocess.run(
        [sys.executable, str(CLI_SCRIPT_PATH), "--docs", "--headers-only"],
        capture_output=True,
        text=True,
        check=False,
    )

    assert result.returncode == 2
    assert "not allowed with argument" in result.stderr


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
