# scripts

Helper scripts used by the build and release tooling.

- `generate_api_c.py` (with its `_generate_api_c_*.py` helpers) - generates the
  C FFI header, its Go copy, and the dlsym stub. Pass `--docs` to generate the
  C API HTML reference under `docs/c-api` instead; that reference is built and
  published only from the CI pipeline, never by the local delivery gate. Every
  documented symbol gets its own heading anchor, and doc comments are rendered
  as Markdown, with Rust intra-doc links resolved to the published page of the
  C API symbol they name. `_generate_api_c_h.py` also owns the shared page
  shell, the shared content styling, the list of reference sections the site
  publishes, and the `SITE_BASE_URL` of the documentation site
  (`https://docs.openpit.dev`), which every generated canonical link, sitemap
  location, and `robots.txt` reads.
- `_generate_docs_site.py` - renders `README.md` into the documentation-site
  root page, prepends generated navigation to every published reference, and
  assembles the publishable tree under `target/docs-site`: the generated
  references, a generated `robots.txt`, `llms.txt`, and `404.html`, and the
  assets the published pages reference. It derives the sitemap and clean-URL
  redirects from canonical and robots metadata in the final normalized HTML
  tree.
  Repository-relative README links are rewritten to the public repository;
  assets that ship with the site keep their on-site path. The `openpit.dev`
  landing page is a different site, kept in its own repository, and nothing
  belonging to it is ever copied.
- `summarize_llvm_cov.py` - condenses a `cargo-llvm-cov` JSON export into a
  compact coverage summary.

## How to run

They are normally invoked through `just` ([just.systems](https://just.systems/)):

```sh
just gen-api-c   # runs generate_api_c.py
```

The documentation site is a pipeline-only flow:

```sh
just --justfile pipeline.just assemble-docs-site   # runs _generate_docs_site.py
```

`gen-docs-cpp` needs `doxygen` and `graphviz` on the machine that runs it and
fails when either is missing.

`summarize_llvm_cov.py` is run as part of the coverage flow. All of them can
also be run directly with `python3 scripts/<name>.py` (pass `--help` for
arguments).

## How to run unit tests

```sh
python -m pytest scripts/tests
```
