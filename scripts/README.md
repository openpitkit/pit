# scripts

Helper scripts used by the build and release tooling.

- `generate_api_c.py` (with its `_generate_api_c_*.py` helpers) — generates the
  C FFI header and its Markdown docs.
- `summarize_llvm_cov.py` — condenses a `cargo-llvm-cov` JSON export into a
  compact coverage summary.

## How to run

They are normally invoked through `just` ([just.systems](https://just.systems/)):

```sh
just gen-api-c   # runs generate_api_c.py
```

`summarize_llvm_cov.py` is run as part of the coverage flow. Both can also be
run directly with `python3 scripts/<name>.py` (pass `--help` for arguments).

## How to run unit tests

```sh
python -m pytest scripts/tests
```
