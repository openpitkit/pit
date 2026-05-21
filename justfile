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
check: fmt-all gen-api-c lint-all test-all run-examples

# Run all examples.
run-examples: run-examples-go run-examples-python

# Lint all.
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
    cd bindings/go && golangci-lint run ./...
    cd examples/go/rate_pnl_killswitch && gofmt -l . | (! grep .)
    just _go-in examples/go/rate_pnl_killswitch "go vet -all ./..."
    just _go-in examples/go/rate_pnl_killswitch "golangci-lint run ./..."

# Run all tests.
test-all: test-rust test-python test-go test-go-race test-c-examples

# Rust tests.
test-rust:
    cargo test --workspace --locked -q
    cargo test -p openpit --all-features --locked -q
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
    python -m pytest -q {{ args }}
# Full Python test suite
test-python: python-develop
    just _pytest bindings/python/tests
    just _pytest examples/python/rate_pnl_killswitch
# Python unit tests only.
test-python-unit: python-develop
    just _pytest bindings/python/tests/unit
# Python integration test only.
test-python-integration: python-develop
    just _pytest bindings/python/tests/integration
# Run a workspace Python example from examples/python against local sources.
run-examples-python: python-develop
    python examples/python/rate_pnl_killswitch/main.py

# Full Go test suite (bindings + workspace examples).
test-go:
    just _go "go test ./..."
    just _go-in examples/go/rate_pnl_killswitch "go test ./..."
# Go race test suite.
test-go-race:
    just _go "go test -race ./..."
    just _go-in examples/go/rate_pnl_killswitch "go test -race ./..."
# Go tests with actionable coverage summary.
test-go-cov:
    just _go "go test -coverprofile=coverage.out -coverpkg=./... ./..."
    cd bindings/go && go tool cover -func=coverage.out | grep -v '100.0%'
# Run a workspace Go examples from examples/go against local sources.
run-examples-go:
    just _go-in examples/go/rate_pnl_killswitch "go run ."

# Compile C examples embedded in public README files.
test-c-examples:
    awk 'BEGIN { in_block = 0; first_block = 1 } /^```c$/ { in_block = 1; if (!first_block) print ""; first_block = 0; next } /^```$/ && in_block { in_block = 0; next } in_block { print }' bindings/c/README.md > /tmp/openpit_readme_example.c
    cc -std=c11 -fsyntax-only -I bindings/c /tmp/openpit_readme_example.c

# Format all.
fmt-all: fmt-rust fmt-python fmt-go
# Format Rust.
fmt-rust:
    cargo fmt --all
# Format Python.
fmt-python:
    python -m black .
# Format Go.
fmt-go:
    cd bindings/go && gofmt -w .

# Prepare new release (kind is patch, minor or major).
release kind: check
    cargo release {{ kind }} --execute --no-confirm
# Push the current HEAD to the dry-run branch and start the staging release workflow.
release-dry:
    git push --force-with-lease origin HEAD:release-dry-run
    gh workflow run release.yml --ref release-dry-run -f dry_run=true

# Install Python bindings into the current Python environment (debug build).
python-develop:
    maturin develop --manifest-path bindings/python/Cargo.toml
# Install Python bindings into the current Python environment (release build).
python-develop-release:
    maturin develop --release --manifest-path bindings/python/Cargo.toml

# Generate the C header and Markdown docs for the FFI crate.
gen-api-c:
    python3 scripts/generate_api_c.py
    python3 scripts/generate_api_c_dlsym.py

# Build FFI.
_build-ffi:
    cargo build -p openpit-ffi --release --locked

# Run a Go command in the bindings/go module with FFI runtime path configured.
_go args:
    just _go-in bindings/go "{{ args }}"

# Run a Go command in a workspace-level subdirectory with FFI runtime path configured.
_go-in dir args: _build-ffi
    OPENPIT_RUNTIME_LIBRARY_PATH="$(if [ "$(uname -s)" = "Darwin" ]; then \
      echo "$(pwd)/target/release/libopenpit_ffi.dylib"; \
    elif [ "$(uname -s)" = "Linux" ]; then \
      echo "$(pwd)/target/release/libopenpit_ffi.so"; \
    else \
      echo "unsupported OS for pit-ffi runtime lookup" >&2; \
      exit 1; \
    fi)" && cd {{ dir }} && OPENPIT_RUNTIME_LIBRARY_PATH="$OPENPIT_RUNTIME_LIBRARY_PATH" {{ args }}
