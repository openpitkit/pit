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

"""Assemble the publishable documentation site under ``target/docs-site``.

The site root is the repository ``README.md`` rendered into the same page
shell the generated API references use, with generated navigation to every
reference the site hosts; the references themselves come from ``docs/`` where
``gen-docs-site`` leaves them. Alongside them the subdomain gets its own
``robots.txt``, ``llms.txt``, and ``404.html``. What ships is an explicit list
rather than the whole of ``docs/``: the shared ``partials/`` are inlined at
generation time instead of being served, and anything belonging to the
``openpit.dev`` landing page - which lives in its own repository - stays out
of ``docs.openpit.dev`` even when a copy is left behind under ``docs/``.
"""

from __future__ import annotations

import posixpath
import re
import shutil
from html import escape, unescape
from pathlib import Path
from urllib.parse import unquote, urljoin, urlsplit, urlunsplit

import _generate_api_c_h as header
from markdown_it import MarkdownIt
from markdown_it.token import Token

ROOT = Path(__file__).resolve().parents[1]
README_PATH = ROOT / "README.md"
SITE_DIR = ROOT / "docs"
OUTPUT_DIR = ROOT / "target" / "docs-site"
DOXYGEN_MAIN_PAGE_SOURCE = ROOT / "bindings" / "cpp" / "DoxygenMainPage.md"
DOXYGEN_MAIN_PAGE_OUTPUT = f"{DOXYGEN_MAIN_PAGE_SOURCE.stem}_8md.html"

INDEX_CANONICAL_URL = f"{header.SITE_BASE_URL}/"
INDEX_DESCRIPTION = (
    "Generated C, C++, and JavaScript/TypeScript API references for OpenPit."
)
NOT_FOUND_DESCRIPTION = "The requested OpenPit documentation page was not found."

# Generated reference sections, as produced by the gen-docs-* recipes. The list
# lives with the page shell so the copy step, site-root navigation, robots.txt,
# and llms.txt cannot disagree. The sitemap is derived from the final tree.
API_SECTIONS: tuple[header.ApiSection, ...] = header.API_SECTIONS

# Site-root files the page shell links to.
ROOT_FILES: tuple[str, ...] = (
    "favicon.ico",
    "favicon-light.svg",
    "favicon-dark.svg",
)

# docs/assets also holds landing-page-only art, so the published subset is
# listed explicitly: the shared stylesheet, the logo the header partial embeds,
# and the banner the README opens with.
ASSET_FILES: tuple[str, ...] = (
    "styles.css",
    "pit-logo-ex.png",
    "pit-readme-banner.png",
)

# Repository-relative README targets resolve to the public repository, except
# for files under docs/assets that ship with the site itself.
REPO_BLOB_BASE = "https://github.com/openpitkit/pit/blob/main"
REPO_TREE_BASE = "https://github.com/openpitkit/pit/tree/main"
SITE_ASSET_PREFIX = "docs/assets/"

# AI crawlers the landing page allows by name. The documentation subdomain
# mirrors the list: a crawler that honours the landing page must not be left
# guessing about the references it links to.
AI_CRAWLER_GROUPS: tuple[tuple[str, ...], ...] = (
    (
        "OAI-SearchBot",
        "ChatGPT-User",
        "PerplexityBot",
        "Perplexity-User",
        "Claude-SearchBot",
        "Claude-User",
        "Google-Extended",
        "Applebot",
        "Applebot-Extended",
    ),
    (
        "GPTBot",
        "ClaudeBot",
        "anthropic-ai",
        "CCBot",
        "Amazonbot",
        "Meta-ExternalAgent",
    ),
)

# Markdown content styling. The shared stylesheet carries only the site
# variables and the landing-page components; the content rules come from the
# page shell, and only the README elements the reference pages do not have are
# defined here.
MARKDOWN_PAGE_CSS = """\
    .md-nav {
      font-size: 0.75rem;
      color: var(--muted);
      margin: 0 0 20px;
    }

    .md h2 {
      font-size: 1.0625rem;
      font-weight: 700;
      letter-spacing: -0.01em;
      margin: 32px 0 12px;
      padding-top: 20px;
      border-top: 1px solid var(--border);
    }

    .md h3 {
      font-size: 0.9375rem;
      font-weight: 700;
      letter-spacing: -0.01em;
      margin: 24px 0 10px;
    }

    .md h4 {
      font-size: 0.6875rem;
      font-weight: 700;
      letter-spacing: 0.07em;
      text-transform: uppercase;
      color: var(--muted);
      margin: 18px 0 8px;
    }

    .md img {
      max-width: 100%;
      height: auto;
      vertical-align: middle;
    }

    .md hr {
      border: none;
      border-top: 1px solid var(--border);
      margin: 24px 0;
    }

    .md details {
      margin-bottom: 14px;
    }

    .md summary {
      font-size: 0.8125rem;
      color: var(--muted-lt);
      cursor: pointer;
    }"""

INDEX_PAGE_CSS = f"{header.render_content_css('.md')}\n\n{MARKDOWN_PAGE_CSS}"


class UnknownRepositoryLinkError(Exception):
    """A repository-relative README target has no file behind it."""


class InvalidReadmeError(Exception):
    """The README does not have the shape the site root is rendered from."""


class MissingSiteSourceError(Exception):
    """A file the published site needs is missing from ``docs/``."""


def build_markdown() -> MarkdownIt:
    """Return the parser the README is written against.

    CommonMark defaults cover the whole document; ``html`` is kept enabled so
    the ``<details>``/``<summary>`` blocks survive instead of being escaped.
    """
    return MarkdownIt("commonmark", {"html": True})


def rewrite_relative_url(url: str, *, root: Path) -> str:
    """Point a repository-relative README target at its published location.

    Absolute URLs, protocol-relative URLs, bare anchors, and site-absolute
    paths are returned unchanged. Everything else is a repository path: assets
    that ship with the site keep serving from the site, the rest resolve to the
    public repository, as a blob or a tree depending on what the path is.
    """
    parts = urlsplit(url)
    if parts.scheme or parts.netloc or not parts.path:
        return url
    path = parts.path
    if path.startswith("/"):
        return url
    # urlsplit keeps the query and fragment verbatim, and for a relative URL
    # the path is always its prefix, so the remainder can be re-attached as is.
    tail = url[len(path) :]

    if path.startswith(SITE_ASSET_PREFIX):
        return f"/assets/{path[len(SITE_ASSET_PREFIX) :]}{tail}"

    # The link carries a URL path, so percent-escapes have to come off before
    # it names a file: "a%20b.png" is the file "a b.png".
    target = root / unquote(path)
    if target.is_dir():
        return f"{REPO_TREE_BASE}/{path.rstrip('/')}{tail}"
    if target.is_file():
        return f"{REPO_BLOB_BASE}/{path}{tail}"
    raise UnknownRepositoryLinkError(
        f"README links to {url!r}, which is not a repository path"
    )


def rewrite_links(tokens: list[Token], *, root: Path) -> None:
    """Rewrite link and image targets on the parsed document, in place.

    Rewriting the token tree rather than the source text is what keeps the
    relative-looking paths inside fenced code blocks untouched.
    """
    for token in header.walk_tokens(tokens):
        if token.type == "link_open":
            attribute = "href"
        elif token.type == "image":
            attribute = "src"
        else:
            continue
        url = token.attrGet(attribute)
        if isinstance(url, str):
            token.attrSet(attribute, rewrite_relative_url(url, root=root))


def split_leading_title(tokens: list[Token]) -> tuple[str, list[Token]]:
    """Split the document title off its body.

    The README opens with a level-one heading that repeats the project name
    and tagline the shared header partial already renders, so it must not stay
    in the body as a second visible title.
    """
    opening = tokens[:3]
    shape = [token.type for token in opening]
    if shape != ["heading_open", "inline", "heading_close"] or opening[0].tag != "h1":
        raise InvalidReadmeError(
            "README must start with a level-one heading, but it starts with" f" {shape}"
        )
    return tokens[1].content.strip(), tokens[3:]


def render_api_nav(indent: str) -> list[str]:
    """Render the navigation to every reference the site publishes.

    The root page is rendered from the README, which is written for the
    repository rather than for this site, so the links to what the site hosts
    are generated from the section list instead of being authored.
    """
    lines = [f'{indent}<nav class="md-nav" aria-label="API reference">']
    lines.append(f"{indent}  API reference:")
    for index, section in enumerate(API_SECTIONS):
        separator = "" if index == len(API_SECTIONS) - 1 else ","
        lines.append(
            f'{indent}  <a href="{section.site_path}">'
            f"{header.escape_text(section.title)}</a>{separator}"
        )
    lines.append(f"{indent}</nav>")
    return lines


def render_index_body(markdown_text: str, *, root: Path) -> tuple[str, list[str]]:
    """Render README markdown into a page title and the page body markup."""
    parser = build_markdown()
    env: dict[str, object] = {}
    tokens = parser.parse(markdown_text, env)
    rewrite_links(tokens, root=root)
    title, body_tokens = split_leading_title(tokens)
    # Rendered markup is emitted unindented: whitespace added inside a <pre>
    # block would show up in the published code samples.
    content = parser.renderer.render(body_tokens, parser.options, env)
    body = [
        '    <main class="md">',
        # Kept for the document outline and search engines only, hidden so the
        # header partial stays the single visible title, the way the C API
        # index pairs the shared header with one page heading.
        f'      <h1 class="visually-hidden">{header.escape_text(title)}</h1>',
        *render_api_nav("      "),
        *content.splitlines(),
        "    </main>",
    ]
    return title, body


def render_index_page(readme_path: Path | None = None) -> str:
    """Render the README into the documentation-site root page."""
    source = README_PATH if readme_path is None else readme_path
    title, body = render_index_body(
        source.read_text(encoding="utf-8"), root=source.parent
    )
    return header.render_doc_page(
        title,
        INDEX_CANONICAL_URL,
        body,
        description=INDEX_DESCRIPTION,
        css=INDEX_PAGE_CSS,
    )


_HREF_RE = re.compile(
    r"(?P<prefix>\bhref\s*=\s*)(?P<quote>['\"])(?P<url>.*?)(?P=quote)",
    re.IGNORECASE,
)
_LINK_CANONICAL_RE = re.compile(
    r"<link\b(?=[^>]*\brel\s*=\s*['\"]canonical['\"])[^>]*>\s*",
    re.IGNORECASE,
)
_LINK_TAG_RE = re.compile(r"<link\b[^>]*>", re.IGNORECASE)
_META_TAG_RE = re.compile(r"<meta\b[^>]*>", re.IGNORECASE)
_ATTRIBUTE_RE = re.compile(
    r"(?P<name>[A-Za-z_:][-A-Za-z0-9_:.]*)\s*=\s*"
    r"(?P<quote>['\"])(?P<value>.*?)(?P=quote)",
    re.DOTALL,
)
_TITLE_RE = re.compile(
    r"<title\b[^>]*>(?P<title>.*?)</title>", re.IGNORECASE | re.DOTALL
)
_HEAD_END_RE = re.compile(r"</head\s*>", re.IGNORECASE)
_DATA_URL_RE = re.compile(
    r"(?P<quote>['\"])(?P<url>[A-Za-z0-9_./:@%+~=&?;-]+\.html"
    r"(?:[?#][A-Za-z0-9_./:@%+~=&?;-]*)?)(?P=quote)"
)
_EMPTY_DOXYGEN_CONTENTS_RE = re.compile(
    r'<div\s+class=["\']contents["\']\s*>\s*</div>' r"(?:\s*<!--\s*contents\s*-->)?",
    re.IGNORECASE,
)

# Doxygen navigation/index pages aggregate other pages rather than document a
# single entity. They stay publishable for browsing but do not belong in the
# sitemap.
_DOXYGEN_NAV_PAGES: frozenset[str] = frozenset(
    {
        "annotated.html",
        "classes.html",
        "namespaces.html",
        "files.html",
        "hierarchy.html",
    }
)
_DOXYGEN_NAV_PREFIXES: tuple[str, ...] = (
    "namespacemembers",
    "functions",
    "globals",
)


def _attributes(tag: str) -> dict[str, str]:
    return {
        match.group("name").lower(): unescape(match.group("value"))
        for match in _ATTRIBUTE_RE.finditer(tag)
    }


def _is_doxygen_file_reference(text: str, path: Path) -> bool:
    generator = any(
        attributes.get("name", "").lower() == "generator"
        and attributes.get("content", "").lower().startswith("doxygen ")
        for attributes in (
            _attributes(match.group(0)) for match in _META_TAG_RE.finditer(text)
        )
    )
    return generator and _page_title(text, path).endswith(" File Reference")


def discover_page_aliases(section_root: Path) -> dict[str, Path]:
    """Map the configured Doxygen main-page backing file to the section index."""
    path = section_root / DOXYGEN_MAIN_PAGE_OUTPUT
    if not path.is_file():
        return {}
    text = path.read_text(encoding="utf-8")
    if not (
        _is_doxygen_file_reference(text, path)
        and _EMPTY_DOXYGEN_CONTENTS_RE.search(text)
    ):
        return {}
    return {DOXYGEN_MAIN_PAGE_OUTPUT: Path("index.html")}


def _local_section_path(
    url_path: str, section: header.ApiSection, source_path: Path
) -> str | None:
    if url_path.startswith("/"):
        prefix = f"/{section.slug}/"
        if not url_path.startswith(prefix):
            return None
        return posixpath.normpath(unquote(url_path.removeprefix(prefix)))
    source_parent = source_path.parent.as_posix()
    return posixpath.normpath(posixpath.join(source_parent, unquote(url_path)))


def _canonical_url(
    section: header.ApiSection,
    relative_path: Path,
    aliases: dict[str, Path],
) -> str:
    target = aliases.get(relative_path.as_posix(), relative_path)
    clean_path = header.clean_html_url(target.as_posix())
    return urljoin(section.url, clean_path)


def _clean_generated_url(
    url: str,
    section: header.ApiSection,
    aliases: dict[str, Path],
    source_path: Path,
) -> str:
    """Rewrite a local generated HTML target to its public clean URL."""
    parts = urlsplit(url)
    if parts.scheme not in {"", "http", "https"}:
        return url
    if parts.netloc and parts.netloc != urlsplit(header.SITE_BASE_URL).netloc:
        return url
    path = parts.path
    if not path.endswith(".html"):
        return url
    local_path = _local_section_path(path, section, source_path)
    if local_path in aliases:
        target = _canonical_url(section, aliases[local_path], aliases)
        target_parts = urlsplit(target)
        return urlunsplit(
            (
                target_parts.scheme,
                target_parts.netloc,
                target_parts.path,
                parts.query,
                parts.fragment,
            )
        )
    return header.clean_html_url(
        urlunsplit((parts.scheme, parts.netloc, path, parts.query, parts.fragment))
    )


def _page_title(text: str, path: Path) -> str:
    match = _TITLE_RE.search(text)
    if match is None:
        raise MissingSiteSourceError(f"{path} has no HTML title")
    title = re.sub(r"<[^>]+>", "", match.group("title"))
    return unescape(title).strip()


def _description_subject(section: header.ApiSection, relative_path: Path) -> str:
    """Return a readable, path-specific subject for fallback metadata."""
    stem = relative_path.stem
    if stem == "index" and relative_path.parent == Path("."):
        return "API index"

    if section.slug == "js-api" and relative_path.parent != Path("."):
        page_kind = {
            "classes": "class",
            "enums": "enum",
            "functions": "function",
            "interfaces": "interface",
            "modules": "module",
            "types": "type",
            "variables": "variable",
        }.get(relative_path.parts[0], relative_path.parts[0])
        return f"{page_kind} {stem}"

    if section.slug == "cpp-api":
        file_match = re.search(r"_8([A-Za-z0-9]+)$", stem)
        if file_match is not None:
            source = stem[: file_match.start()].replace("_2", "/")
            return f"source file {source}.{file_match.group(1)}"
        for prefix, page_kind in (
            ("class", "class"),
            ("struct", "struct"),
            ("union", "union"),
            ("namespace", "namespace"),
        ):
            if stem.startswith(prefix):
                name = stem.removeprefix(prefix).replace("_1_1", "::")
                return f"{page_kind} {name}"

    return f"page {header.clean_html_url(relative_path.as_posix())}"


def _page_description(
    text: str,
    section: header.ApiSection,
    title: str,
    relative_path: Path,
) -> str:
    for match in _META_TAG_RE.finditer(text):
        attributes = _attributes(match.group(0))
        if attributes.get("name", "").lower() == "description":
            description = attributes.get("content", "").strip()
            if description and description != "Documentation for @openpit/engine":
                return description
    subject = _description_subject(section, relative_path)
    return f"OpenPit {section.title} reference for {subject}: {title}."


def _strip_social_metadata(text: str) -> str:
    def replace(match: re.Match[str]) -> str:
        attributes = _attributes(match.group(0))
        name = attributes.get("name", "").lower()
        prop = attributes.get("property", "").lower()
        if name.startswith("twitter:") or prop.startswith("og:"):
            return ""
        return match.group(0)

    return _META_TAG_RE.sub(replace, text)


def _social_metadata(title: str, description: str, canonical: str) -> str:
    escaped_title = escape(title, quote=True)
    escaped_description = escape(description, quote=True)
    escaped_canonical = escape(canonical, quote=True)
    return "\n".join(
        [
            f'<meta name="description" content="{escaped_description}" />',
            f'<link rel="canonical" href="{escaped_canonical}" />',
            f'<meta property="og:title" content="{escaped_title}" />',
            f'<meta property="og:description" content="{escaped_description}" />',
            '<meta property="og:type" content="website" />',
            '<meta property="og:site_name" content="OpenPit" />',
            '<meta property="og:locale" content="en_US" />',
            f'<meta property="og:url" content="{escaped_canonical}" />',
            f'<meta property="og:image" content="{header.SOCIAL_PREVIEW_URL}" />',
            f'<meta property="og:image:width"'
            f' content="{header.SOCIAL_PREVIEW_WIDTH}" />',
            f'<meta property="og:image:height"'
            f' content="{header.SOCIAL_PREVIEW_HEIGHT}" />',
            f'<meta property="og:image:alt" content="{header.SOCIAL_PREVIEW_ALT}" />',
            '<meta name="twitter:card" content="summary_large_image" />',
            f'<meta name="twitter:title" content="{escaped_title}" />',
            f'<meta name="twitter:description" content="{escaped_description}" />',
            f'<meta name="twitter:image" content="{header.SOCIAL_PREVIEW_URL}" />',
            f'<meta name="twitter:image:alt" content="{header.SOCIAL_PREVIEW_ALT}" />',
        ]
    )


def normalize_html_page(
    path: Path,
    *,
    section: header.ApiSection,
    section_root: Path,
    aliases: dict[str, Path],
) -> None:
    """Normalize one generated page for clean URLs and shared metadata."""
    text = path.read_text(encoding="utf-8")
    relative_path = path.relative_to(section_root)

    def rewrite_href(match: re.Match[str]) -> str:
        clean = _clean_generated_url(
            match.group("url"), section, aliases, relative_path
        )
        prefix = match.group("prefix")
        quote = match.group("quote")
        return f"{prefix}{quote}{clean}{quote}"

    text = _HREF_RE.sub(rewrite_href, text)
    title = _page_title(text, path)
    description = _page_description(text, section, title, relative_path)
    canonical = _canonical_url(section, relative_path, aliases)
    text = _LINK_CANONICAL_RE.sub("", text)
    text = _strip_social_metadata(text)
    # Replace an existing description so generated themes cannot leave a
    # duplicate after the shared metadata is inserted.
    text = _META_TAG_RE.sub(
        lambda match: (
            ""
            if _attributes(match.group(0)).get("name", "").lower() == "description"
            else match.group(0)
        ),
        text,
    )
    metadata = _social_metadata(title, description, canonical)
    if _HEAD_END_RE.search(text) is None:
        raise MissingSiteSourceError(f"{path} has no closing HTML head")
    text = _HEAD_END_RE.sub(f"{metadata}\n</head>", text, count=1)
    path.write_text(text, encoding="utf-8", newline="\n")


def normalize_navigation_data(
    root: Path, section: header.ApiSection, aliases: dict[str, Path]
) -> None:
    """Normalize URL literals in generated search and navigation data."""
    for path in sorted((*root.rglob("*.js"), *root.rglob("*.json"))):
        text = path.read_text(encoding="utf-8")
        relative_path = path.relative_to(root)

        def rewrite_url(match: re.Match[str], source_path: Path = relative_path) -> str:
            clean = _clean_generated_url(
                match.group("url"), section, aliases, source_path
            )
            return f'{match.group("quote")}{clean}{match.group("quote")}'

        normalized = _DATA_URL_RE.sub(rewrite_url, text)
        if normalized != text:
            path.write_text(normalized, encoding="utf-8", newline="\n")


def normalize_reference(section_root: Path, section: header.ApiSection) -> None:
    aliases = discover_page_aliases(section_root)
    for path in sorted(section_root.rglob("*.html")):
        normalize_html_page(
            path,
            section=section,
            section_root=section_root,
            aliases=aliases,
        )
    normalize_navigation_data(section_root, section, aliases)


def render_robots_txt() -> str:
    """Render the robots.txt for the documentation subdomain.

    Generated rather than authored so the sitemap location cannot drift from
    the canonical links. ``Allow: /`` already covers every section, so the
    only other groups are the AI crawlers the landing page names explicitly:
    the references are meant to be quotable by them too.
    """
    lines = ["User-agent: *", "Allow: /"]
    for group in AI_CRAWLER_GROUPS:
        lines.append("")
        lines.extend(f"User-agent: {agent}" for agent in group)
        lines.append("Allow: /")
    lines.extend(["", f"Sitemap: {header.SITE_BASE_URL}/sitemap.xml", ""])
    return "\n".join(lines)


def render_llms_txt() -> str:
    """Render the llms.txt for the documentation subdomain.

    The landing page describes the project; this one describes what is
    published here, so a model that lands on the subdomain can tell the
    references apart without crawling them.
    """
    lines = [
        "# OpenPit API References",
        "",
        f"> {header.SITE_BASE_URL} publishes the generated API references of"
        " OpenPit, an open-source, embeddable pre-trade risk engine and SDK."
        " Every page here is generated from the sources of the release it"
        " documents; the project overview lives on the landing page and the"
        " conceptual documentation in the wiki.",
        "",
        "## References",
        "",
    ]
    lines.extend(
        f"- [{section.title}]({section.url}): {section.summary}"
        for section in API_SECTIONS
    )
    lines.extend(
        [
            "",
            "## Project",
            "",
            "- [Website](https://openpit.dev/): Overview, installation, and"
            " documentation links.",
            "- [Wiki](https://wiki.openpit.dev/): Architecture, domain types,"
            " pipeline design, and policies.",
            "- [Source repository](https://github.com/openpitkit/pit): Rust"
            " workspace with the Go, Python, JavaScript/TypeScript, C++, Rust,"
            " and C SDK surfaces.",
            "",
            "## License",
            "",
            "[Apache License 2.0]"
            "(https://github.com/openpitkit/pit/blob/main/LICENSE).",
            "",
        ]
    )
    return "\n".join(lines)


def _is_noindex(text: str) -> bool:
    for match in _META_TAG_RE.finditer(text):
        attributes = _attributes(match.group(0))
        if attributes.get("name", "").lower() != "robots":
            continue
        directives = re.split(r"[\s,]+", attributes.get("content", "").lower())
        if "noindex" in directives:
            return True
    return False


def _page_canonical(text: str, path: Path) -> str | None:
    canonicals: list[str] = []
    for match in _LINK_TAG_RE.finditer(text):
        attributes = _attributes(match.group(0))
        if "canonical" not in attributes.get("rel", "").lower().split():
            continue
        href = attributes.get("href", "").strip()
        if href:
            canonicals.append(href)
    if not canonicals:
        return None
    if len(canonicals) != 1:
        raise MissingSiteSourceError(
            f"{path} has {len(canonicals)} canonical links; expected exactly one"
        )
    canonical = canonicals[0]
    parts = urlsplit(canonical)
    if (
        parts.scheme != "https"
        or parts.netloc != urlsplit(header.SITE_BASE_URL).netloc
        or parts.query
        or parts.fragment
    ):
        raise MissingSiteSourceError(
            f"{path} has invalid documentation canonical {canonical!r}"
        )
    return canonical


def _is_cpp_sitemap_page(relative: Path) -> bool:
    """Return whether a final-tree C++ page is canonical API content."""
    if relative == Path("index.html"):
        return True
    if relative.parent != Path("."):
        return False
    name = relative.name
    if name.endswith("-members.html") or name.startswith("dir_"):
        return False
    if name in _DOXYGEN_NAV_PAGES or name.startswith(_DOXYGEN_NAV_PREFIXES):
        return False
    if name.startswith(("class", "struct", "union", "namespace")):
        return True
    return name.endswith(("_8hpp.html", "_8h.html", "_8md.html"))


def discover_sitemap_urls(root: Path) -> list[str]:
    """Read indexable canonical URLs from the final publishable HTML tree."""
    urls: set[str] = set()
    for path in sorted(root.rglob("*.html")):
        relative = path.relative_to(root)
        if relative.parts[0] == "cpp-api" and not _is_cpp_sitemap_page(
            Path(*relative.parts[1:])
        ):
            continue
        text = path.read_text(encoding="utf-8")
        if _is_noindex(text):
            continue
        canonical = _page_canonical(text, path)
        if canonical is None:
            raise MissingSiteSourceError(
                f"{path} is indexable but has no canonical link"
            )
        urls.add(canonical)
    return sorted(urls)


def render_sitemap(urls: list[str]) -> str:
    lines = [
        '<?xml version="1.0" encoding="UTF-8"?>',
        '<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">',
    ]
    lines.extend(f"  <url><loc>{escape(url)}</loc></url>" for url in urls)
    lines.extend(["</urlset>", ""])
    return "\n".join(lines)


def render_redirects(root: Path) -> str:
    """Derive clean-URL redirects from the final page canonicals."""
    redirects: dict[str, str] = {}
    for path in sorted(root.rglob("*.html")):
        text = path.read_text(encoding="utf-8")
        canonical = _page_canonical(text, path)
        if canonical is None:
            continue
        target = urlsplit(canonical).path
        relative = path.relative_to(root).as_posix()
        aliases = {f"/{relative}", f"/{relative.removesuffix('.html')}"}
        for alias in aliases:
            if alias == target:
                continue
            previous = redirects.setdefault(alias, target)
            if previous != target:
                raise MissingSiteSourceError(
                    f"redirect alias {alias!r} targets both {previous!r} and {target!r}"
                )
    lines = [f"{source} {redirects[source]} 301" for source in sorted(redirects)]
    return "\n".join([*lines, ""])


def render_not_found_page() -> str:
    """Render the 404 page the documentation Pages project serves.

    It uses the same shell as every generated page, so an unknown URL keeps
    the site chrome and offers the references instead of a bare host error.
    """
    body = [
        '    <main class="md">',
        "      <h1>Page not found</h1>",
        "      <p>This URL is not part of the published documentation. It may"
        " have been renamed by a newer release, or the reference it belonged"
        " to may no longer exist.</p>",
        *render_api_nav("      "),
        f'      <p><a href="{INDEX_CANONICAL_URL}">Back to the documentation'
        " index</a></p>",
        "    </main>",
    ]
    return header.render_doc_page(
        "Page not found - OpenPit",
        None,
        body,
        description=NOT_FOUND_DESCRIPTION,
        robots="noindex, follow",
        css=INDEX_PAGE_CSS,
    )


def _require(path: Path) -> Path:
    if not path.exists():
        raise MissingSiteSourceError(
            f"{path} is missing; run 'just --justfile pipeline.just gen-docs-site'"
        )
    return path


def _require_section(path: Path) -> Path:
    """Return a generated reference directory, or explain why it is unusable.

    An absent directory means the section was never generated; an empty one
    means it was generated somewhere else. Both would publish a section URL
    that serves nothing, so neither may pass silently.
    """
    if not path.is_dir():
        raise MissingSiteSourceError(
            f"{path} is missing; run 'just --justfile pipeline.just gen-docs-site'"
        )
    if not any(entry.is_file() for entry in path.rglob("*")):
        raise MissingSiteSourceError(
            f"{path} holds no generated pages; run"
            " 'just --justfile pipeline.just gen-docs-site'"
        )
    return path


def assemble(dest: Path | None = None) -> Path:
    """Build the publishable tree, replacing any previous one.

    The output directory is rebuilt from scratch so a page that disappeared
    from a regenerated reference cannot linger in the published site.
    """
    target = OUTPUT_DIR if dest is None else dest
    index = render_index_page()
    not_found = render_not_found_page()

    if target.exists():
        shutil.rmtree(target)
    target.mkdir(parents=True)
    (target / "index.html").write_text(index, encoding="utf-8", newline="\n")
    (target / "404.html").write_text(not_found, encoding="utf-8", newline="\n")

    for section in API_SECTIONS:
        section_target = target / section.slug
        shutil.copytree(
            _require_section(SITE_DIR / section.slug),
            section_target,
            ignore=shutil.ignore_patterns(".DS_Store", "sitemap.xml"),
        )
        normalize_reference(section_target, section)

    (target / "robots.txt").write_text(
        render_robots_txt(), encoding="utf-8", newline="\n"
    )
    (target / "llms.txt").write_text(render_llms_txt(), encoding="utf-8", newline="\n")

    assets = target / "assets"
    assets.mkdir()
    for name in ASSET_FILES:
        shutil.copy2(_require(SITE_DIR / "assets" / name), assets / name)
    for name in ROOT_FILES:
        shutil.copy2(_require(SITE_DIR / name), target / name)

    (target / "_redirects").write_text(
        render_redirects(target), encoding="utf-8", newline="\n"
    )
    (target / "sitemap.xml").write_text(
        render_sitemap(discover_sitemap_urls(target)),
        encoding="utf-8",
        newline="\n",
    )

    return target


def generate() -> None:
    target = assemble()
    print(f"Assembled {target.relative_to(ROOT).as_posix()}")


if __name__ == "__main__":
    try:
        generate()
    except (
        MissingSiteSourceError,
        UnknownRepositoryLinkError,
        InvalidReadmeError,
        header.MissingSitePartialError,
        header.UnsupportedDocMarkupError,
    ) as error:
        raise SystemExit(f"error: {error}") from None
