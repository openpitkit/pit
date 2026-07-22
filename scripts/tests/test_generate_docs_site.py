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
import shutil
import sys
from pathlib import Path

import pytest

SCRIPTS_DIR = Path(__file__).resolve().parents[1]
C_API_SCRIPT_PATH = SCRIPTS_DIR / "_generate_api_c_h.py"
DOCS_SITE_SCRIPT_PATH = SCRIPTS_DIR / "_generate_docs_site.py"

README_SAMPLE = """\
# Sample Title

[![Banner](docs/assets/pit-readme-banner.png)](https://openpit.dev/)

Read [the license](LICENSE) and browse [examples/](examples/).
See the [website](https://openpit.dev/) and the [scope](#current-scope).

```sh
cat LICENSE
cp docs/assets/pit-readme-banner.png /tmp/
```

<details>
<summary>Details block</summary>

Body text.

</details>
"""


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


def load_docs_site_module():
    load_c_api_module()
    return load_module(DOCS_SITE_SCRIPT_PATH, "_generate_docs_site")


def write_sample_readme(root: Path) -> Path:
    """Lay out the repository paths the sample README links to."""
    root.mkdir(parents=True, exist_ok=True)
    (root / "LICENSE").write_text("Apache-2.0\n", encoding="utf-8")
    (root / "examples").mkdir(exist_ok=True)
    assets = root / "docs" / "assets"
    assets.mkdir(parents=True, exist_ok=True)
    (assets / "pit-readme-banner.png").write_bytes(b"png")
    readme = root / "README.md"
    readme.write_text(README_SAMPLE, encoding="utf-8")
    return readme


def make_site_dir(root: Path) -> Path:
    """Build a docs/ tree shaped like the one gen-docs-site leaves behind."""
    site = root / "docs"
    c_api = site / "c-api"
    c_api.mkdir(parents=True, exist_ok=True)
    (c_api / "index.html").write_text(
        "<!doctype html><html><head><title>C API</title>"
        '<meta name="description" content="C reference." />'
        '<link rel="canonical" href="https://docs.openpit.dev/c-api/index.html" />'
        "</head><body>"
        '<a href="orders.html?view=all#OpenPitOrder">Orders</a>'
        '<a href="https://example.com/guide.html">External</a>'
        "</body></html>\n",
        encoding="utf-8",
    )
    (c_api / "orders.html").write_text(
        "<!doctype html><html><head><title>Orders</title>"
        '<meta name="description" content="Orders reference." />'
        "</head><body></body></html>\n",
        encoding="utf-8",
    )

    cpp_api = site / "cpp-api"
    cpp_api.mkdir(parents=True, exist_ok=True)
    (cpp_api / "index.html").write_text(
        "<!doctype html><html><head><title>C++ API</title></head><body>"
        '<a href="classEngine.html#run">Engine</a>'
        '<a href="DoxygenMainPage_8md.html">Main page</a>'
        "</body></html>\n",
        encoding="utf-8",
    )
    for name, title in (
        ("classEngine.html", "Engine Class Reference"),
        ("engine_8hpp.html", "engine.hpp File Reference"),
        ("asyncengine_2engine_8hpp.html", "engine.hpp File Reference"),
    ):
        (cpp_api / name).write_text(
            f"<!doctype html><html><head><title>{title}</title></head>"
            "<body></body></html>\n",
            encoding="utf-8",
        )
    for name, title, contents in (
        ("DoxygenMainPage_8md.html", "DoxygenMainPage.md", ""),
        ("pretrade_8hpp.html", "pretrade.hpp", ""),
        (
            "documented_manual_8md.html",
            "Manual.md",
            "<p>Substantive file documentation.</p>",
        ),
    ):
        (cpp_api / name).write_text(
            "<!doctype html><html><head>"
            '<meta name="generator" content="Doxygen 1.17.0" />'
            f"<title>OpenPit: {title} File Reference</title></head><body>"
            f'<div class="contents">{contents}</div><!-- contents -->'
            "</body></html>\n",
            encoding="utf-8",
        )
    (cpp_api / "navtree.js").write_text(
        'const nav = ["classEngine.html#run", "DoxygenMainPage_8md.html"];\n',
        encoding="utf-8",
    )

    js_api = site / "js-api"
    js_api.mkdir(parents=True, exist_ok=True)
    (js_api / "index.html").write_text(
        "<!doctype html><html><head><title>JS API</title>"
        '<meta name="description" content="Documentation for @openpit/engine" />'
        '</head><body><a href="modules.html">Modules</a></body></html>\n',
        encoding="utf-8",
    )
    (js_api / "modules.html").write_text(
        "<!doctype html><html><head><title>Modules</title></head>"
        "<body></body></html>\n",
        encoding="utf-8",
    )
    (js_api / "hidden.html").write_text(
        "<!doctype html><html><head><title>Hidden</title>"
        '<meta name="robots" content="noindex, follow" />'
        "</head><body></body></html>\n",
        encoding="utf-8",
    )
    # A nested page and the marker that keeps a static host from filtering
    # underscore-prefixed paths: both have to survive the copy.
    (js_api / "classes").mkdir(exist_ok=True)
    (js_api / "classes" / "index.Engine.html").write_text(
        "<!doctype html><html><head><title>Engine</title></head>"
        '<body><a href="../modules.html?q=1#engine">Modules</a></body></html>\n',
        encoding="utf-8",
    )
    (js_api / "guides").mkdir(exist_ok=True)
    (js_api / "guides" / "index.html").write_text(
        "<!doctype html><html><head><title>Guides</title></head>"
        "<body></body></html>\n",
        encoding="utf-8",
    )
    for directory, name in (
        ("types", "index.AccountBlockErrorKind.html"),
        ("variables", "index.AccountBlockErrorKind-1.html"),
    ):
        page_dir = js_api / directory
        page_dir.mkdir(exist_ok=True)
        (page_dir / name).write_text(
            "<!doctype html><html><head><title>AccountBlockErrorKind</title>"
            '<meta name="description" content="Documentation for @openpit/engine" />'
            "</head><body></body></html>\n",
            encoding="utf-8",
        )
    (js_api / ".nojekyll").write_text("", encoding="utf-8")
    (js_api / "sitemap.xml").write_text("typedoc sitemap\n", encoding="utf-8")
    (site / "assets").mkdir(exist_ok=True)
    for name in ("styles.css", "pit-logo-ex.png", "pit-readme-banner.png"):
        (site / "assets" / name).write_text(f"/* {name} */\n", encoding="utf-8")
    (site / "assets" / "pit-logo.svg").write_text("<svg/>\n", encoding="utf-8")
    for name in ("favicon.ico", "favicon-light.svg", "favicon-dark.svg"):
        (site / name).write_text("icon\n", encoding="utf-8")
    # Landing-page files that belong to the other domain.
    (site / "index.html").write_text("<!-- landing page -->\n", encoding="utf-8")
    (site / "llms.txt").write_text("landing\n", encoding="utf-8")
    (site / "CNAME").write_text("openpit.dev\n", encoding="utf-8")
    (site / "robots.txt").write_text("Sitemap: https://openpit.dev\n", encoding="utf-8")
    (site / "partials").mkdir(exist_ok=True)
    (site / "partials" / "header.html").write_text("<div></div>\n", encoding="utf-8")
    return site


def make_repo(module, root: Path, monkeypatch) -> Path:
    """Point the generator at a self-contained repository fixture.

    The README is part of the fixture: reading the real one would tie these
    tests to an unrelated documentation edit.
    """
    readme = write_sample_readme(root)
    monkeypatch.setattr(module, "SITE_DIR", make_site_dir(root))
    monkeypatch.setattr(module, "README_PATH", readme)
    return root


def test_readme_links_are_rewritten_per_target_kind(tmp_path: Path) -> None:
    module = load_docs_site_module()
    readme = write_sample_readme(tmp_path)

    _title, body = module.render_index_body(
        readme.read_text(encoding="utf-8"), root=tmp_path
    )
    html = "\n".join(body)

    # A relative file becomes a repository blob, a directory a repository tree.
    assert 'href="https://github.com/openpitkit/pit/blob/main/LICENSE"' in html
    assert 'href="https://github.com/openpitkit/pit/tree/main/examples"' in html
    # The banner ships with the site, so it keeps serving from the site.
    assert 'src="/assets/pit-readme-banner.png"' in html
    assert 'src="docs/assets/pit-readme-banner.png"' not in html
    # Absolute links and anchors pass through untouched.
    assert 'href="https://openpit.dev/"' in html
    assert 'href="#current-scope"' in html
    # Relative-looking paths inside a fenced block are text, never links.
    assert "cat LICENSE" in html
    assert "cp docs/assets/pit-readme-banner.png /tmp/" in html


def test_readme_raw_html_survives_rendering(tmp_path: Path) -> None:
    module = load_docs_site_module()
    readme = write_sample_readme(tmp_path)

    _title, body = module.render_index_body(
        readme.read_text(encoding="utf-8"), root=tmp_path
    )
    html = "\n".join(body)

    assert "<details>" in html
    assert "<summary>Details block</summary>" in html


def test_readme_link_to_percent_encoded_asset_is_resolved(tmp_path: Path) -> None:
    module = load_docs_site_module()
    write_sample_readme(tmp_path)
    (tmp_path / "release notes.md").write_text("notes\n", encoding="utf-8")

    encoded = module.rewrite_relative_url("release%20notes.md", root=tmp_path)

    # The link carries a URL path, so the file behind it is the decoded name.
    assert encoded == ("https://github.com/openpitkit/pit/blob/main/release%20notes.md")


def test_readme_link_to_missing_path_is_rejected(tmp_path: Path) -> None:
    module = load_docs_site_module()
    readme = write_sample_readme(tmp_path)
    readme.write_text(
        f"{README_SAMPLE}\nSee [the plan](ROADMAP.md).\n", encoding="utf-8"
    )

    with pytest.raises(module.UnknownRepositoryLinkError):
        module.render_index_body(readme.read_text(encoding="utf-8"), root=tmp_path)


def test_index_page_uses_the_shared_shell_and_docs_canonical(tmp_path: Path) -> None:
    module = load_docs_site_module()
    readme = write_sample_readme(tmp_path)

    page = module.render_index_page(readme)

    assert page.startswith("<!doctype html>\n")
    assert '<link rel="canonical" href="https://docs.openpit.dev/" />' in page
    assert f'<meta name="description" content="{module.INDEX_DESCRIPTION}" />' in page
    assert '<meta property="og:url" content="https://docs.openpit.dev/" />' in page
    assert '<meta property="og:site_name" content="OpenPit" />' in page
    assert '<meta property="og:locale" content="en_US" />' in page
    assert (
        f'<meta property="og:image:width"'
        f' content="{module.header.SOCIAL_PREVIEW_WIDTH}" />' in page
    )
    assert (
        f'<meta property="og:image:height"'
        f' content="{module.header.SOCIAL_PREVIEW_HEIGHT}" />' in page
    )
    assert '<meta property="og:image:alt"' in page
    assert '<meta name="twitter:image:alt"' in page
    assert "<title>Sample Title</title>" in page
    assert '<link rel="stylesheet" href="/assets/styles.css" />' in page
    assert '<link rel="icon" href="/favicon.ico" sizes="any" />' in page
    # The header and footer partials are inlined, not linked.
    assert "Pre-trade Integrity Toolkit" in page
    assert "https://github.com/openpitkit/pit/blob/main/LICENSE" in page


def test_index_page_keeps_one_visible_title(tmp_path: Path) -> None:
    module = load_docs_site_module()
    readme = write_sample_readme(tmp_path)

    page = module.render_index_page(readme)

    assert '<h1 class="visually-hidden">Sample Title</h1>' in page
    assert page.count("<h1") == 1


def test_readme_without_leading_title_is_rejected(tmp_path: Path) -> None:
    module = load_docs_site_module()

    with pytest.raises(module.InvalidReadmeError):
        module.render_index_body("Body only.\n", root=tmp_path)


def test_readme_with_an_unexpected_opening_token_is_rejected(tmp_path: Path) -> None:
    module = load_docs_site_module()

    # A setext-looking fragment parses to a heading without an inline child.
    with pytest.raises(module.InvalidReadmeError):
        module.render_index_body("## Second level\n\nBody.\n", root=tmp_path)


def test_index_page_links_to_every_published_reference(tmp_path: Path) -> None:
    module = load_docs_site_module()
    readme = write_sample_readme(tmp_path)

    page = module.render_index_page(readme)

    assert '<nav class="md-nav" aria-label="API reference">' in page
    for section in module.API_SECTIONS:
        assert f'<a href="/{section.slug}/">{section.title}</a>' in page


def test_canonical_urls_share_the_site_base() -> None:
    module = load_docs_site_module()

    assert module.INDEX_CANONICAL_URL == "https://docs.openpit.dev/"
    assert module.header.C_API_BASE_URL == "https://docs.openpit.dev/c-api/"
    for section in module.API_SECTIONS:
        assert section.url == f"{module.header.SITE_BASE_URL}/{section.slug}/"


def test_robots_txt_allows_ai_crawlers_and_points_at_the_docs_sitemap() -> None:
    module = load_docs_site_module()

    robots = module.render_robots_txt()

    assert robots.startswith("User-agent: *\nAllow: /\n")
    # "Allow: /" already covers every section; per-section lines would only
    # repeat it.
    for section in module.API_SECTIONS:
        assert f"Allow: /{section.slug}/" not in robots
    for group in module.AI_CRAWLER_GROUPS:
        for agent in group:
            assert f"User-agent: {agent}\n" in robots
    assert "Sitemap: https://docs.openpit.dev/sitemap.xml\n" in robots
    assert robots.endswith("\n")


def test_sitemap_scans_final_html_excludes_noindex_and_dedupes(
    tmp_path: Path,
) -> None:
    module = load_docs_site_module()
    root = tmp_path / "site"
    (root / "nested").mkdir(parents=True)

    def write_page(relative: str, canonical: str, *, noindex: bool = False) -> None:
        robots = '<meta name="robots" content="noindex, follow" />' if noindex else ""
        (root / relative).write_text(
            "<!doctype html><html><head><title>Page</title>"
            f'<link rel="canonical" href="{canonical}" />{robots}'
            "</head><body></body></html>\n",
            encoding="utf-8",
        )

    write_page("z.html", "https://docs.openpit.dev/z")
    write_page("nested/a.html", "https://docs.openpit.dev/a")
    write_page("nested/duplicate.html", "https://docs.openpit.dev/a")
    write_page("hidden.html", "https://docs.openpit.dev/hidden", noindex=True)

    urls = module.discover_sitemap_urls(root)

    assert urls == ["https://docs.openpit.dev/a", "https://docs.openpit.dev/z"]
    sitemap = module.render_sitemap(urls)
    assert re.findall(r"<loc>([^<]+)</loc>", sitemap) == urls


def test_sitemap_includes_only_canonical_doxygen_content_pages(
    tmp_path: Path,
) -> None:
    module = load_docs_site_module()
    root = tmp_path / "site"
    cpp_api = root / "cpp-api"
    (cpp_api / "search").mkdir(parents=True)

    included = (
        "index.html",
        "classEngine.html",
        "structOrder.html",
        "unionPayload.html",
        "namespaceopenpit.html",
        "engine_8hpp.html",
        "documented_manual_8md.html",
    )
    excluded = (
        "annotated.html",
        "classes.html",
        "namespaces.html",
        "files.html",
        "hierarchy.html",
        "namespacemembers.html",
        "namespacemembers_a.html",
        "functions.html",
        "functions_a.html",
        "globals.html",
        "globals_a.html",
        "classEngine-members.html",
        "dir_deadbeef.html",
        "search/search.html",
    )
    for relative in (*included, *excluded):
        path = cpp_api / relative
        path.parent.mkdir(parents=True, exist_ok=True)
        canonical = "" if relative == "index.html" else relative.removesuffix(".html")
        path.write_text(
            "<!doctype html><html><head><title>Page</title>"
            f'<link rel="canonical" href="https://docs.openpit.dev/cpp-api/'
            f'{canonical}" /></head><body></body></html>\n',
            encoding="utf-8",
        )

    urls = set(module.discover_sitemap_urls(root))

    for relative in included:
        canonical = "" if relative == "index.html" else relative.removesuffix(".html")
        assert f"https://docs.openpit.dev/cpp-api/{canonical}" in urls
    for relative in excluded:
        canonical = relative.removesuffix(".html")
        assert f"https://docs.openpit.dev/cpp-api/{canonical}" not in urls


def test_assembled_sitemap_matches_the_final_tree(tmp_path: Path, monkeypatch) -> None:
    module = load_docs_site_module()
    make_repo(module, tmp_path / "repo", monkeypatch)
    target = module.assemble(tmp_path / "docs-site")

    sitemap = (target / "sitemap.xml").read_text(encoding="utf-8")
    urls = re.findall(r"<loc>([^<]+)</loc>", sitemap)

    assert urls == module.discover_sitemap_urls(target)
    assert "https://docs.openpit.dev/" in urls
    assert "https://docs.openpit.dev/c-api/orders" in urls
    assert "https://docs.openpit.dev/cpp-api/documented_manual_8md" in urls
    assert "https://docs.openpit.dev/js-api/hidden" not in urls
    assert urls.count("https://docs.openpit.dev/cpp-api/") == 1
    assert not any(url.endswith(".html") for url in urls)


def test_llms_txt_describes_every_published_reference() -> None:
    module = load_docs_site_module()

    llms = module.render_llms_txt()

    assert llms.startswith("# OpenPit API References\n")
    for section in module.API_SECTIONS:
        assert f"- [{section.title}]({section.url}): {section.summary}" in llms
    assert llms.endswith("\n")


def test_not_found_page_uses_the_shared_shell_and_the_reference_nav() -> None:
    module = load_docs_site_module()

    page = module.render_not_found_page()

    assert page.startswith("<!doctype html>\n")
    assert "<title>Page not found - OpenPit</title>" in page
    assert '<meta name="robots" content="noindex, follow" />' in page
    assert 'rel="canonical"' not in page
    assert 'property="og:url"' not in page
    for section in module.API_SECTIONS:
        assert f'<a href="/{section.slug}/">{section.title}</a>' in page


def test_assemble_publishes_only_the_documentation_tree(
    tmp_path: Path, monkeypatch
) -> None:
    module = load_docs_site_module()
    make_repo(module, tmp_path / "repo", monkeypatch)
    target = tmp_path / "docs-site"

    module.assemble(target)

    assert sorted(path.name for path in target.iterdir()) == [
        "404.html",
        "_redirects",
        "assets",
        "c-api",
        "cpp-api",
        "favicon-dark.svg",
        "favicon-light.svg",
        "favicon.ico",
        "index.html",
        "js-api",
        "llms.txt",
        "robots.txt",
        "sitemap.xml",
    ]
    assert sorted(path.name for path in (target / "assets").iterdir()) == [
        "pit-logo-ex.png",
        "pit-readme-banner.png",
        "styles.css",
    ]
    assert (target / "c-api" / "index.html").is_file()
    # The landing page and its files stay on the other domain, and the shared
    # partials are inlined at generation time rather than published.
    assert not (target / "partials").exists()
    assert not (target / "CNAME").exists()
    index = (target / "index.html").read_text(encoding="utf-8")
    assert "landing page" not in index
    assert "<title>Sample Title</title>" in index
    assert '<link rel="canonical" href="https://docs.openpit.dev/" />' in index
    assert (target / "robots.txt").read_text(encoding="utf-8") == (
        module.render_robots_txt()
    )
    # The subdomain gets its own llms.txt, never the landing page's.
    assert (target / "llms.txt").read_text(encoding="utf-8") == module.render_llms_txt()
    assert (target / "404.html").read_text(encoding="utf-8") == (
        module.render_not_found_page()
    )
    assert (target / "_redirects").read_text(encoding="utf-8") == (
        module.render_redirects(target)
    )


def test_redirects_are_derived_from_final_page_canonicals(
    tmp_path: Path, monkeypatch
) -> None:
    module = load_docs_site_module()
    make_repo(module, tmp_path / "repo", monkeypatch)
    target = module.assemble(tmp_path / "docs-site")

    redirects = module.render_redirects(target).splitlines()

    assert "/index / 301" in redirects
    assert "/index.html / 301" in redirects
    for section in module.API_SECTIONS:
        assert f"/{section.slug}/index /{section.slug}/ 301" in redirects
        assert f"/{section.slug}/index.html /{section.slug}/ 301" in redirects
    assert "/cpp-api/DoxygenMainPage_8md /cpp-api/ 301" in redirects
    assert "/cpp-api/DoxygenMainPage_8md.html /cpp-api/ 301" in redirects
    assert (
        "/cpp-api/documented_manual_8md.html "
        "/cpp-api/documented_manual_8md 301" in redirects
    )
    assert "/cpp-api/documented_manual_8md /cpp-api/ 301" not in redirects


def test_only_doxygen_main_page_backing_file_aliases_to_index(tmp_path: Path) -> None:
    module = load_docs_site_module()
    cpp_api = make_site_dir(tmp_path) / "cpp-api"

    aliases = module.discover_page_aliases(cpp_api)

    assert aliases["DoxygenMainPage_8md.html"] == Path("index.html")
    assert "pretrade_8hpp.html" not in aliases
    assert "documented_manual_8md.html" not in aliases


def test_assemble_copies_nested_and_hidden_section_files(
    tmp_path: Path, monkeypatch
) -> None:
    module = load_docs_site_module()
    make_repo(module, tmp_path / "repo", monkeypatch)
    target = tmp_path / "docs-site"

    module.assemble(target)

    assert (target / "js-api" / "classes" / "index.Engine.html").is_file()
    assert (target / "js-api" / ".nojekyll").is_file()
    assert not (target / "js-api" / "sitemap.xml").exists()


def test_assemble_normalizes_clean_urls_and_page_metadata(
    tmp_path: Path, monkeypatch
) -> None:
    module = load_docs_site_module()
    make_repo(module, tmp_path / "repo", monkeypatch)
    target = tmp_path / "docs-site"

    module.assemble(target)

    expected_canonicals = {
        "c-api/index.html": "https://docs.openpit.dev/c-api/",
        "c-api/orders.html": "https://docs.openpit.dev/c-api/orders",
        "cpp-api/index.html": "https://docs.openpit.dev/cpp-api/",
        "cpp-api/classEngine.html": "https://docs.openpit.dev/cpp-api/classEngine",
        "cpp-api/DoxygenMainPage_8md.html": "https://docs.openpit.dev/cpp-api/",
        "cpp-api/pretrade_8hpp.html": (
            "https://docs.openpit.dev/cpp-api/pretrade_8hpp"
        ),
        "cpp-api/documented_manual_8md.html": (
            "https://docs.openpit.dev/cpp-api/documented_manual_8md"
        ),
        "cpp-api/engine_8hpp.html": "https://docs.openpit.dev/cpp-api/engine_8hpp",
        "cpp-api/asyncengine_2engine_8hpp.html": (
            "https://docs.openpit.dev/cpp-api/asyncengine_2engine_8hpp"
        ),
        "js-api/index.html": "https://docs.openpit.dev/js-api/",
        "js-api/hidden.html": "https://docs.openpit.dev/js-api/hidden",
        "js-api/modules.html": "https://docs.openpit.dev/js-api/modules",
        "js-api/classes/index.Engine.html": (
            "https://docs.openpit.dev/js-api/classes/index.Engine"
        ),
        "js-api/guides/index.html": "https://docs.openpit.dev/js-api/guides/",
        "js-api/types/index.AccountBlockErrorKind.html": (
            "https://docs.openpit.dev/js-api/types/index.AccountBlockErrorKind"
        ),
        "js-api/variables/index.AccountBlockErrorKind-1.html": (
            "https://docs.openpit.dev/js-api/variables/index.AccountBlockErrorKind-1"
        ),
    }
    descriptions: set[str] = set()
    for relative, canonical in expected_canonicals.items():
        page = (target / relative).read_text(encoding="utf-8")
        assert page.count('rel="canonical"') == 1
        assert page.count('name="description"') == 1
        assert f'<link rel="canonical" href="{canonical}" />' in page
        assert page.count(f'<meta property="og:url" content="{canonical}" />') == 1
        assert '<meta property="og:site_name" content="OpenPit" />' in page
        assert '<meta property="og:locale" content="en_US" />' in page
        assert (
            f'<meta property="og:image:width"'
            f' content="{module.header.SOCIAL_PREVIEW_WIDTH}" />' in page
        )
        assert (
            f'<meta property="og:image:height"'
            f' content="{module.header.SOCIAL_PREVIEW_HEIGHT}" />' in page
        )
        assert '<meta property="og:image:alt"' in page
        assert '<meta name="twitter:image:alt"' in page
        description = re.search(r'name="description" content="([^"]+)"', page)
        assert description is not None
        assert description.group(1) not in descriptions
        descriptions.add(description.group(1))

    c_index = (target / "c-api" / "index.html").read_text(encoding="utf-8")
    assert 'href="orders?view=all#OpenPitOrder"' in c_index
    assert 'href="https://example.com/guide.html"' in c_index

    cpp_index = (target / "cpp-api" / "index.html").read_text(encoding="utf-8")
    assert 'href="classEngine#run"' in cpp_index
    assert 'href="https://docs.openpit.dev/cpp-api/"' in cpp_index
    cpp_nav = (target / "cpp-api" / "navtree.js").read_text(encoding="utf-8")
    assert '"classEngine#run"' in cpp_nav
    assert '"https://docs.openpit.dev/cpp-api/"' in cpp_nav

    js_index = (target / "js-api" / "index.html").read_text(encoding="utf-8")
    assert 'href="modules"' in js_index
    assert "Documentation for @openpit/engine" not in js_index
    assert (
        '<meta name="description" content="OpenPit JavaScript/TypeScript API '
        'reference for API index: JS API." />' in js_index
    )
    js_nested = (target / "js-api" / "classes" / "index.Engine.html").read_text(
        encoding="utf-8"
    )
    assert 'href="../modules?q=1#engine"' in js_nested

    def description(relative: str) -> str:
        page = (target / relative).read_text(encoding="utf-8")
        match = re.search(r'name="description" content="([^"]+)"', page)
        assert match is not None
        return match.group(1)

    cpp_root_file = description("cpp-api/engine_8hpp.html")
    cpp_nested_file = description("cpp-api/asyncengine_2engine_8hpp.html")
    assert "source file engine.hpp" in cpp_root_file
    assert "source file asyncengine/engine.hpp" in cpp_nested_file
    assert cpp_root_file != cpp_nested_file

    js_type = description("js-api/types/index.AccountBlockErrorKind.html")
    js_variable = description("js-api/variables/index.AccountBlockErrorKind-1.html")
    assert "type index.AccountBlockErrorKind" in js_type
    assert "variable index.AccountBlockErrorKind-1" in js_variable
    assert js_type != js_variable


def test_assemble_starts_from_a_clean_directory(tmp_path: Path, monkeypatch) -> None:
    module = load_docs_site_module()
    make_repo(module, tmp_path / "repo", monkeypatch)
    target = tmp_path / "docs-site"
    (target / "c-api").mkdir(parents=True)
    stale = target / "c-api" / "removed-section.html"
    stale.write_text("<!doctype html>\n", encoding="utf-8")
    stale_root = target / "removed-page.html"
    stale_root.write_text("<!doctype html>\n", encoding="utf-8")

    module.assemble(target)

    assert not stale.exists()
    assert not stale_root.exists()
    assert (target / "c-api" / "index.html").is_file()


def test_assemble_reports_a_missing_generated_reference(
    tmp_path: Path, monkeypatch
) -> None:
    module = load_docs_site_module()
    repo = make_repo(module, tmp_path / "repo", monkeypatch)
    shutil.rmtree(repo / "docs" / "js-api")

    with pytest.raises(module.MissingSiteSourceError, match="js-api"):
        module.assemble(tmp_path / "docs-site")


def test_assemble_reports_an_empty_generated_reference(
    tmp_path: Path, monkeypatch
) -> None:
    module = load_docs_site_module()
    repo = make_repo(module, tmp_path / "repo", monkeypatch)
    section = repo / "docs" / "cpp-api"
    shutil.rmtree(section)
    section.mkdir()

    with pytest.raises(module.MissingSiteSourceError, match="cpp-api"):
        module.assemble(tmp_path / "docs-site")


def test_page_shell_reports_a_missing_partial(tmp_path: Path, monkeypatch) -> None:
    module = load_docs_site_module()
    partials = tmp_path / "partials"
    partials.mkdir()
    monkeypatch.setattr(module.header, "PARTIALS_DIR", partials)

    with pytest.raises(module.header.MissingSitePartialError, match="header.html"):
        module.render_not_found_page()
