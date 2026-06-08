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

# Workspace build and test shortcuts.

# Rust build.
build:
    cargo build --workspace

# Build Go.
build-go:
    cd bindings/go && go build

# Format, generate, and lint and test the result.
check: fmt-all gen-api-c check-dry
# Lint and test the result (non-mutating).
check-dry: lint-all test-all run-examples
# Format, generate, and lint and test Rust.
check-rust: fmt-all gen-api-c lint-rust test-rust
# Lint and test Rust (non-mutating).
check-rust-dry: lint-rust test-rust
# Format, generate, and lint and test Go.
check-go: fmt-all gen-api-c lint-go test-go
# Lint and test Go (non-mutating).
check-go-dry: lint-go test-go test-go-race
# Format, generate, and lint and test Python.
check-python: fmt-all gen-api-c lint-python test-python
# Lint and test Python (non-mutating).
check-python-dry: lint-python test-python
    cargo nextest run -p openpit-python --locked --status-level fail --final-status-level fail

# Run all examples.
[parallel]
run-examples: run-examples-go run-examples-python

# Lint all.
[parallel]
lint-all: lint-rust lint-python lint-go
# Lint Rus.
lint-rust:
    cargo fmt --all -- --check --quiet
    cargo clippy --workspace --all-targets --no-default-features --locked -q -- -D warnings
    cargo clippy -p openpit --all-targets --all-features --locked -q -- -D warnings
    RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --all-features --locked -q
# Lint Python.
lint-python:
    python -m ruff check --quiet .
    python -m black . --check --quiet
# Lint Go.
lint-go:
    cd bindings/go && gofmt -l . | (! grep .)
    cd bindings/go && go vet -all ./... > /dev/null
    cd bindings/go && golangci-lint run --timeout=5m ./...
    gofmt -l examples/go | (! grep .)
    just _go-examples "go vet -all ./..."
    just _go-examples "golangci-lint run --timeout=5m ./..."

# Run all tests.
[parallel]
test-all: test-rust test-python test-go test-go-race test-c-examples

# Rust tests.
test-rust:
    #!/usr/bin/env bash
    set -euo pipefail
    _run_nextest() {
        if ! cargo nextest "$@"; then
            if ! command -v cargo-nextest &>/dev/null; then
                printf '\n\033[31m  error: cargo-nextest is required to run Rust tests.\033[0m\n'
                printf '\033[33m  install: brew install cargo-nextest\033[0m\n\n'
            fi
            exit 1
        fi
    }
    _run_nextest run --workspace --locked --status-level fail --final-status-level fail
    _run_nextest run -p openpit --all-features --locked --status-level fail --final-status-level fail
    # nextest does not run doctests; cover them via cargo test.
    cargo test --workspace --doc --locked
    cargo test -p openpit --all-features --doc --locked
# Rust tests with actionable coverage summary.
test-rust-cov:
    mkdir -p target/llvm-cov
    cargo llvm-cov test --workspace --exclude openpit-python --all-features --json --output-path target/llvm-cov/workspace.json
    python3 scripts/summarize_llvm_cov.py target/llvm-cov/workspace.json --output target/llvm-cov/workspace-summary.json --text
# Raw cargo-llvm-cov console report.
test-rust-cov-raw:
    cargo llvm-cov --workspace --exclude openpit-python --all-features

# Run docker-based release e2e checks against published artifacts.
test-release-e2e version:
    ./e2e/run.sh {{ version }}

# Shared pytest runner helper.
_pytest args:
    # shellcheck disable=SC1083
    python -m pytest -q --no-header {{ args }}
# Full Python test suite
test-python: python-develop
    #!/usr/bin/env bash
    set -euo pipefail
    just _pytest bindings/python/tests
    for d in examples/python/*/; do
      [ -f "${d}main.py" ] || continue
      just _pytest "$d"
    done
# Python unit tests only.
test-python-unit: python-develop
    just _pytest bindings/python/tests/unit
# Python integration test only.
test-python-integration: python-develop
    just _pytest bindings/python/tests/integration
# Run a workspace Python example from examples/python against local sources.
run-examples-python: python-develop
    python examples/python/rate_pnl_killswitch/main.py
    python examples/python/spot_funds/main.py
    just run-examples-python-table
# Run a spot-policy scenario table through the Python spot_table example.
run-examples-python-table test_file="examples/tables/spot/coverage.md": python-develop
    python examples/python/spot_table/main.py --table $(pwd)/{{ test_file }}
# Repeat-run a scenario table through the Python example for `dur` (default 3m).
run-examples-python-table-repeat test_file="examples/tables/spot/coverage.md" dur="3m": python-develop
    python examples/python/spot_table/main.py --table $(pwd)/{{ test_file }} --min-duration {{ dur }}

# Full Go test suite.
test-go:
    just _go "go test ./..."
    just _go-examples "go test ./..."
# Go race test suite.
test-go-race:
    just _go "go test -race ./..."
    just _go-examples "go test -race ./..."
# Go tests with actionable coverage summary.
test-go-cov:
    just _go "go test -coverprofile=coverage.out -coverpkg=./... ./..."
    cd bindings/go && go tool cover -func=coverage.out | grep -v '100.0%'
# Run a workspace Go examples from examples/go against local sources.
run-examples-go:
    just _go-in examples/go/rate_pnl_killswitch "go run ."
    just _go-in examples/go/spot_funds "go run ."
    just run-examples-go-table
# Run a spot-policy scenario table through the spot_table example.
run-examples-go-table test_file="examples/tables/spot/coverage.md":
    just _go-in examples/go/spot_table "go run . -table $(pwd)/{{ test_file }}"
# Repeat-run a scenario table: re-run it for `dur` (a Go duration, default 3m).
run-examples-go-table-repeat test_file="examples/tables/spot/coverage.md" dur="3m":
    just _go-in examples/go/spot_table "go run . -table $(pwd)/{{ test_file }} -min-duration {{ dur }}"

# Compile C examples embedded in public README files.
test-c-examples:
    awk 'BEGIN { in_block = 0; first_block = 1 } /^```c$/ { in_block = 1; if (!first_block) print ""; first_block = 0; next } /^```$/ && in_block { in_block = 0; next } in_block { print }' bindings/c/README.md > /tmp/openpit_readme_example.c
    cc -std=c11 -fsyntax-only -I bindings/c /tmp/openpit_readme_example.c

# Format all.
[parallel]
fmt-all: fmt-rust fmt-python fmt-go
# Format Rust.
fmt-rust:
    cargo fmt --all
# Format Python.
fmt-python:
    python -m black . -q
# Format Go.
fmt-go:
    cd bindings/go && gofmt -w .
    gofmt -w examples/go

# Prepare new release (kind is patch, minor or major).
release kind: check
    cargo release {{ kind }} --execute --no-confirm
# Push the current HEAD to the dry-run branch and start the staging release workflow.
release-dry:
    git push --force-with-lease origin HEAD:release-dry-run
    gh workflow run release.yml --ref release-dry-run -f dry_run=true

# Install Python bindings into the current Python environment (debug build).
python-develop:
    maturin develop -q --manifest-path bindings/python/Cargo.toml
# Install Python bindings into the current Python environment (release build).
python-develop-release:
    maturin develop -q --release --manifest-path bindings/python/Cargo.toml

# Generate the C header and Markdown docs for the FFI crate.
gen-api-c:
    python3 scripts/generate_api_c.py > /dev/null

# Build FFI.
_build-ffi:
    cargo build -p openpit-ffi --release --locked -q

# Run a Go command in the bindings/go module with FFI runtime path configured.
_go args:
    just _go-in bindings/go "{{ args }}"

# Run a Go command in every examples/go module, with the FFI runtime configured.
_go-examples args: _go-embed-runtime
    #!/usr/bin/env bash
    set -euo pipefail
    case "$(uname -s)" in
      Darwin) lib="$(pwd)/target/release/libopenpit_ffi.dylib" ;;
      Linux)  lib="$(pwd)/target/release/libopenpit_ffi.so" ;;
      *) echo "unsupported OS for pit-ffi runtime lookup" >&2; exit 1 ;;
    esac
    export OPENPIT_RUNTIME_LIBRARY_PATH="$lib"
    for d in examples/go/*/; do
      [ -f "${d}go.mod" ] || continue
      echo ">> ${d}" >&2
      ( cd "$d" && {{ args }} )
    done

# Place the freshly built FFI runtime into the Go embed tree so the
# embedded-runtime loader tests run against a locally built library
# (no release artifact required). The copied lib is gitignored.
_go-embed-runtime: _build-ffi
    #!/usr/bin/env bash
    set -euo pipefail
    os="$(uname -s)"; arch="$(uname -m)"
    case "$arch" in aarch64|arm64) arch=arm64 ;; x86_64|amd64) arch=amd64 ;; esac
    case "$os" in
      Darwin) plat="darwin-$arch"; lib="libopenpit_ffi.dylib" ;;
      Linux)  plat="linux-$arch";  lib="libopenpit_ffi.so" ;;
      *) echo "unsupported OS for go runtime embed: $os" >&2; exit 1 ;;
    esac
    cp "target/release/$lib" "bindings/go/internal/runtime/$plat/$lib"

# Run a Go command in a workspace-level subdirectory with FFI runtime path configured.
_go-in dir args: _go-embed-runtime
    OPENPIT_RUNTIME_LIBRARY_PATH="$(if [ "$(uname -s)" = "Darwin" ]; then \
      echo "$(pwd)/target/release/libopenpit_ffi.dylib"; \
    elif [ "$(uname -s)" = "Linux" ]; then \
      echo "$(pwd)/target/release/libopenpit_ffi.so"; \
    else \
      echo "unsupported OS for pit-ffi runtime lookup" >&2; \
      exit 1; \
    fi)" && cd {{ dir }} && OPENPIT_RUNTIME_LIBRARY_PATH="$OPENPIT_RUNTIME_LIBRARY_PATH" {{ args }}
