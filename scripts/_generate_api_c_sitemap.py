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

import re
from pathlib import Path

import _generate_api_c_h as header

ROOT = Path(__file__).resolve().parents[1]
DOCS_DIR = ROOT / "docs"
C_API_DIR = DOCS_DIR / "c-api"
CPP_API_DIR = DOCS_DIR / "cpp-api"
JS_API_DIR = DOCS_DIR / "js-api"
SITEMAP_PATH = DOCS_DIR / "sitemap.xml"
SITE_BASE = "https://openpit.dev"

# Suffix appended to per-section URLs (e.g. "/c-api/orders" + SUFFIX).
# Default ".html" matches GitHub Pages with Jekyll rendering markdown without
# a custom permalink config. Set to "" or "/" if the site is reconfigured to
# serve pretty URLs.
PAGE_SUFFIX = ".html"

STATIC_URLS: tuple[str, ...] = (
    f"{SITE_BASE}/",
    f"{SITE_BASE}/c-api/",
    f"{SITE_BASE}/cpp-api/",
    f"{SITE_BASE}/js-api/",
)


def discover_section_slugs() -> list[str]:
    if not C_API_DIR.is_dir():
        return []
    sources = header.list_source_files()
    slug_to_source: dict[str, str] = {}
    for source in sources:
        slug, _title = header.section_info(source)
        if slug not in slug_to_source:
            slug_to_source[slug] = source
    source_order = {source: idx for idx, source in enumerate(sources)}

    def sort_key(slug: str) -> tuple[int, str]:
        source = slug_to_source.get(slug)
        if source is None:
            return (len(source_order), slug)
        return (source_order[source], slug)

    slugs = [
        path.stem
        for path in C_API_DIR.glob("*.md")
        if path.is_file() and path.stem != "index"
    ]
    return sorted(set(slugs), key=sort_key)


# Doxygen navigation/index pages that aggregate other pages rather than
# documenting a single entity. These must never enter the sitemap even when
# their name happens to match a content prefix (e.g. "namespaces.html" vs the
# "namespace*" allowlist, or "classes.html" vs "class*").
_DOXYGEN_NAV_PAGES: frozenset[str] = frozenset(
    {
        "annotated.html",
        "classes.html",
        "namespaces.html",
        "files.html",
        "hierarchy.html",
        "index.html",
    }
)

# Prefixes of Doxygen letter-indexed member listings (one page per letter) and
# global-symbol indexes. Excluded as navigation, not content.
_DOXYGEN_NAV_PREFIXES: tuple[str, ...] = (
    "namespacemembers",
    "functions",
    "globals",
)


def _is_cpp_content_page(name: str) -> bool:
    """Return True only for canonical Doxygen content pages.

    A content page documents one entity: a class, struct, union, namespace, or
    file. Everything Doxygen emits for navigation (index/listing pages, the
    per-class member rosters, and directory pages) is rejected.
    """
    if not name.endswith(".html"):
        return False
    # Per-class member roster, never a standalone content page.
    if name.endswith("-members.html"):
        return False
    # Directory pages ("dir_<hash>.html").
    if name.startswith("dir_"):
        return False
    if name in _DOXYGEN_NAV_PAGES:
        return False
    if name.startswith(_DOXYGEN_NAV_PREFIXES):
        return False
    # Class / struct / union / namespace documentation pages. The nav pages
    # that share these prefixes (classes.html, namespaces.html,
    # namespacemembers*.html) are already rejected above.
    if name.startswith(("class", "struct", "union", "namespace")):
        return True
    # File-documentation pages: Doxygen encodes the source name and mangles the
    # extension dot to "_8" (".hpp" -> "_8hpp", ".h" -> "_8h", ".md" -> "_8md").
    return name.endswith(("_8hpp.html", "_8h.html", "_8md.html"))


def discover_cpp_api_paths() -> list[str]:
    if not CPP_API_DIR.is_dir():
        return []
    # Filesystem-driven and top-level only: subdirectories (e.g. "search/")
    # hold Doxygen assets, never content pages, so we deliberately skip them.
    paths = [
        path.name
        for path in CPP_API_DIR.glob("*.html")
        if path.is_file() and _is_cpp_content_page(path.name)
    ]
    return sorted(paths)


def discover_js_api_paths() -> list[str]:
    """Return every generated TypeDoc HTML page below ``docs/js-api``.

    TypeDoc's nested class/function/interface pages are canonical content, not
    alternate navigation URLs, so unlike Doxygen no allowlist is needed. The
    root ``index.html`` is already represented by the static ``/js-api/`` URL.
    """
    if not JS_API_DIR.is_dir():
        return []
    return sorted(
        path.relative_to(JS_API_DIR).as_posix()
        for path in JS_API_DIR.rglob("*.html")
        if path.is_file() and path != JS_API_DIR / "index.html"
    )


def build_canonical_urls() -> list[str]:
    urls = list(STATIC_URLS)
    seen: set[str] = set(urls)
    for slug in discover_section_slugs():
        url = f"{SITE_BASE}/c-api/{slug}{PAGE_SUFFIX}"
        if url in seen:
            continue
        seen.add(url)
        urls.append(url)
    for rel in discover_cpp_api_paths():
        url = f"{SITE_BASE}/cpp-api/{rel}"
        if url in seen:
            continue
        seen.add(url)
        urls.append(url)
    for rel in discover_js_api_paths():
        url = f"{SITE_BASE}/js-api/{rel}"
        if url in seen:
            continue
        seen.add(url)
        urls.append(url)
    return urls


def parse_existing_urls(text: str) -> list[str]:
    return re.findall(r"<loc>\s*([^<\s]+)\s*</loc>", text)


def render_sitemap(urls: list[str]) -> str:
    lines = [
        '<?xml version="1.0" encoding="UTF-8"?>',
        '<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">',
    ]
    for url in urls:
        lines.append(f"  <url><loc>{url}</loc></url>")
    lines.append("</urlset>")
    lines.append("")
    return "\n".join(lines)


def generate() -> None:
    canonical = build_canonical_urls()
    if SITEMAP_PATH.exists():
        existing = parse_existing_urls(SITEMAP_PATH.read_text(encoding="utf-8"))
        if existing == canonical:
            return
    SITEMAP_PATH.parent.mkdir(parents=True, exist_ok=True)
    SITEMAP_PATH.write_text(render_sitemap(canonical), encoding="utf-8", newline="\n")
    print(f"Generated {SITEMAP_PATH.relative_to(ROOT)}")


if __name__ == "__main__":
    generate()
