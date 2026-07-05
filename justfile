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

# Workspace build and test shortcuts.

set windows-shell := ["cmd.exe", "/c"]

venv_dir := env_var_or_default("PIT_VIRTUAL_ENV", justfile_directory() / ".venv")
venv_bin := if os_family() == "windows" { venv_dir / "Scripts" } else { venv_dir / "bin" }
default_python_path := if os_family() == "windows" { venv_bin / "python.exe" } else { venv_bin / "python" }
path_separator := if os_family() == "windows" { ";" } else { ":" }
python_path := env_var_or_default("PYTHON_PATH", default_python_path)
bootstrap_python := env_var_or_default("PIT_BOOTSTRAP_PYTHON", if os_family() == "windows" { "python" } else { "python3" })
windows_target := env_var_or_default("PIT_WINDOWS_TARGET", "x86_64-pc-windows-msvc")
just_helper := "scripts/just_helpers.py"
python_config := "bindings/python/pyproject.toml"
export VIRTUAL_ENV := venv_dir
export PYO3_PYTHON := python_path
export PIT_WINDOWS_TARGET := windows_target
export PATH := venv_bin + path_separator + env_var("PATH")

# Create/update the local Python environment used by just recipes.
[unix]
_ensure-python-env:
    test -x "{{ python_path }}" || {{ bootstrap_python }} -m venv "{{ venv_dir }}"
    "{{ python_path }}" -m pip install -r requirements.txt
[windows]
_ensure-python-env:
    {{ bootstrap_python }} {{ just_helper }} ensure-python-env "{{ venv_dir }}" "{{ python_path }}" requirements.txt

# Rust build (debug).
[unix]
build-debug:
    cargo build --workspace
[windows]
build-debug:
    cargo build --workspace --target {{ windows_target }}

# Rust build (release).
[unix]
build-release:
    cargo build --workspace --release
[windows]
build-release:
    cargo build --workspace --release --target {{ windows_target }}

# Build Go against the debug FFI runtime.
build-go-debug:
    just _go-in debug bindings/go go build

# Build Go against the release FFI runtime.
build-go-release:
    just _go-in release bindings/go go build

# Format, generate, lint, test, and run examples in debug mode.
check-debug: _ensure-python-env fmt-all gen-all check-dry-debug

# Format, generate, lint, test, and run examples in release mode.
check-release: _ensure-python-env fmt-all gen-all check-dry-release

# Format, generate, lint, test, and run examples in debug and release modes.
check-full: _ensure-python-env fmt-all gen-all check-dry-full

# Lint, test, and run examples in debug mode (non-mutating).
check-dry-debug: lint-all test-all-debug run-examples-debug

# Lint, test, and run examples in release mode (non-mutating).
check-dry-release: lint-all test-all-release run-examples-release

# Lint, test, and run examples in debug and release modes (non-mutating).
check-dry-full: lint-all test-all-full run-examples-debug run-examples-release

# Format, generate, lint, and test Rust in debug mode.
check-rust-debug: _ensure-python-env fmt-rust gen-api-c check-rust-dry-debug

# Format, generate, lint, and test Rust in release mode.
check-rust-release: _ensure-python-env fmt-rust gen-api-c check-rust-dry-release

# Format, generate, lint, and test Rust in debug and release modes.
check-rust-full: _ensure-python-env fmt-rust gen-api-c check-rust-dry-full

# Lint and test Rust in debug mode (non-mutating).
[parallel]
check-rust-dry-debug: lint-rust test-rust-debug

# Lint and test Rust in release mode (non-mutating).
[parallel]
check-rust-dry-release: lint-rust test-rust-release

# Lint and test Rust in debug and release modes (non-mutating).
check-rust-dry-full: lint-rust test-rust-full

# Format, generate, lint, and test Go in debug mode.
check-go-debug: _ensure-python-env fmt-go gen-api-c check-go-dry-debug

# Format, generate, lint, and test Go in release mode.
check-go-release: _ensure-python-env fmt-go gen-api-c check-go-dry-release

# Format, generate, lint, and test Go in debug and release modes.
check-go-full: _ensure-python-env fmt-go gen-api-c check-go-dry-full

# Lint and test Go in debug mode (non-mutating).
check-go-dry-debug: lint-go test-go-debug test-go-race

# Lint and test Go in release mode (non-mutating).
[parallel]
check-go-dry-release: lint-go test-go-release

# Lint and test Go in debug and release modes (non-mutating).
check-go-dry-full: lint-go test-go-full

# Format, generate, lint, and test Python in debug mode.
check-python-debug: _ensure-python-env fmt-python gen-api-c check-python-dry-debug

# Format, generate, lint, and test Python in release mode.
check-python-release: _ensure-python-env fmt-python gen-api-c check-python-dry-release

# Format, generate, lint, and test Python in debug and release modes.
check-python-full: _ensure-python-env fmt-python gen-api-c check-python-dry-full

# Lint and test Python in debug mode (non-mutating).
[parallel]
[unix]
check-python-dry-debug: lint-python test-python-debug
    cargo nextest run -p openpit-python --locked --status-level fail --final-status-level fail
[parallel]
[windows]
check-python-dry-debug: lint-python test-python-debug
    cargo nextest run -p openpit-python --target {{ windows_target }} --locked --status-level fail --final-status-level fail

# Lint and test Python in release mode (non-mutating).
[parallel]
[unix]
check-python-dry-release: lint-python test-python-release
    cargo nextest run --release -p openpit-python --locked --status-level fail --final-status-level fail
[parallel]
[windows]
check-python-dry-release: lint-python test-python-release
    cargo nextest run --release -p openpit-python --target {{ windows_target }} --locked --status-level fail --final-status-level fail

# Lint and test Python in debug and release modes (non-mutating).
[unix]
check-python-dry-full: lint-python test-python-full
    cargo nextest run -p openpit-python --locked --status-level fail --final-status-level fail
    cargo nextest run --release -p openpit-python --locked --status-level fail --final-status-level fail
[windows]
check-python-dry-full: lint-python test-python-full
    cargo nextest run -p openpit-python --target {{ windows_target }} --locked --status-level fail --final-status-level fail
    cargo nextest run --release -p openpit-python --target {{ windows_target }} --locked --status-level fail --final-status-level fail

# Format, generate, lint, and test C++ in debug mode.
check-cpp-debug: _ensure-python-env fmt-cpp gen-docs-cpp check-cpp-dry-debug

# Format, generate, lint, and test C++ in release mode.
check-cpp-release: _ensure-python-env fmt-cpp gen-docs-cpp check-cpp-dry-release

# Format, generate, lint, and test C++ in debug and release modes.
check-cpp-full: _ensure-python-env fmt-cpp gen-docs-cpp check-cpp-dry-full

# Lint and test C++ in debug mode (non-mutating).
check-cpp-dry-debug: lint-cpp test-cpp-debug test-examples-cpp-debug

# Lint and test C++ in release mode (non-mutating).
check-cpp-dry-release: lint-cpp test-cpp-release test-examples-cpp-release

# Lint and test C++ in debug and release modes (non-mutating).
check-cpp-dry-full: lint-cpp test-cpp-full test-examples-cpp-full

# Run all examples in debug mode.
run-examples-debug: run-examples-go-debug run-examples-python-debug run-examples-cpp-debug

# Run all examples in release mode.
run-examples-release: run-examples-go-release run-examples-python-release run-examples-cpp-release

# Lint all.
[parallel]
lint-all: lint-rust lint-python lint-go lint-cpp

# Lint Rust.
[unix]
lint-rust:
    cargo fmt --all -- --check --quiet
    cargo clippy --workspace --all-targets --no-default-features --locked -q -- -D warnings
    cargo clippy -p openpit --all-targets --all-features --locked -q -- -D warnings
    RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --all-features --locked -q
[windows]
lint-rust: _ensure-python-env
    cargo fmt --all -- --check --quiet
    cargo clippy --workspace --exclude openpit-python --all-targets --no-default-features --locked -q -- -D warnings
    cargo clippy -p openpit --all-targets --all-features --locked -q -- -D warnings
    {{ python_path }} {{ just_helper }} cargo-doc-warnings

# Lint Python.
lint-python: _ensure-python-env
    {{ python_path }} -m ruff check --config {{ python_config }} --quiet .
    {{ python_path }} -m black --config {{ python_config }} . --check --quiet

# Lint Go.
[unix]
lint-go:
    cd bindings/go && gofmt -l . | (! grep .)
    cd bindings/go && go vet -all ./... > /dev/null
    cd bindings/go && golangci-lint run --timeout=5m ./...
    gofmt -l examples/go | (! grep .)
    just _go-examples debug go vet -all ./...
    just _go-examples debug golangci-lint run --timeout=5m ./...
[windows]
lint-go: _ensure-python-env
    {{ python_path }} {{ just_helper }} check-gofmt bindings/go
    {{ python_path }} {{ just_helper }} check-golangci-lint
    {{ python_path }} {{ just_helper }} go-in debug bindings/go go vet -all ./...
    {{ python_path }} {{ just_helper }} go-in debug bindings/go golangci-lint run --timeout=5m ./...
    {{ python_path }} {{ just_helper }} check-gofmt examples/go
    {{ python_path }} {{ just_helper }} go-examples debug go vet -all ./...
    {{ python_path }} {{ just_helper }} go-examples debug golangci-lint run --timeout=5m ./...

# Run all tests in debug mode.
test-all-debug: test-rust-debug test-python-debug test-go-debug test-go-race test-c-examples test-cpp-debug test-examples-cpp-debug

# Run all tests in release mode.
test-all-release: test-rust-release test-python-release test-go-release test-c-examples test-cpp-release test-examples-cpp-release

# Run all tests in debug and release modes.
test-all-full: test-rust-full test-python-full test-go-full test-c-examples test-cpp-full test-examples-cpp-full

# Rust tests (debug).
[unix]
test-rust-debug:
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
    _run_nextest run --workspace --exclude openpit-python --locked --status-level fail --final-status-level fail
    _run_nextest run -p openpit --all-features --locked --status-level fail --final-status-level fail
    # nextest does not run doctests; cover them via cargo test.
    cargo test --workspace --doc --locked
    cargo test -p openpit --all-features --doc --locked
[windows]
test-rust-debug: _ensure-python-env
    {{ python_path }} {{ just_helper }} test-rust debug

# Rust tests (release).
[unix]
test-rust-release:
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
    _run_nextest run --release --workspace --exclude openpit-python --locked --status-level fail --final-status-level fail
    _run_nextest run --release -p openpit --all-features --locked --status-level fail --final-status-level fail
    # nextest does not run doctests; cover them via cargo test.
    cargo test --release --workspace --doc --locked
    cargo test --release -p openpit --all-features --doc --locked
[windows]
test-rust-release: _ensure-python-env
    {{ python_path }} {{ just_helper }} test-rust release

# Rust tests (debug and release).
test-rust-full: test-rust-debug test-rust-release

# Rust tests with actionable coverage summary.
test-rust-cov: _ensure-python-env
    {{ python_path }} {{ just_helper }} mkdir target/llvm-cov
    cargo llvm-cov test --workspace --exclude openpit-python --all-features --json --output-path target/llvm-cov/workspace.json
    {{ python_path }} scripts/summarize_llvm_cov.py target/llvm-cov/workspace.json --output target/llvm-cov/workspace-summary.json --text

# Raw cargo-llvm-cov console report.
test-rust-cov-raw:
    cargo llvm-cov --workspace --exclude openpit-python --all-features

# Run docker-based release e2e checks against published artifacts.
[unix]
test-release-e2e version:
    ./e2e/run.sh {{ version }}
[windows]
test-release-e2e version:
    echo test-release-e2e uses e2e/run.sh and is Unix-only
    exit /b 1

# Shared pytest runner helper.
[unix]
_pytest args: _ensure-python-env
    # shellcheck disable=SC1083
    {{ python_path }} -m pytest -q --no-header -c {{ python_config }} {{ args }}
[windows]
_pytest args: _ensure-python-env
    {{ python_path }} -m pytest -q --no-header -c {{ python_config }} {{ args }}

# Full Python test suite (debug).
[unix]
test-python-debug: python-develop-debug
    #!/usr/bin/env bash
    set -euo pipefail
    just _pytest bindings/python/tests
    for d in examples/python/*/; do
      [ -f "${d}main.py" ] || continue
      if [[ -f "${d}requirements.txt" ]]; then
        {{ python_path }} -m pip install -r "${d}requirements.txt"
      fi
      just _pytest "$d"
    done
[windows]
test-python-debug: python-develop-debug
    {{ python_path }} {{ just_helper }} test-python

# Full Python test suite (release).
[unix]
test-python-release: python-develop-release
    #!/usr/bin/env bash
    set -euo pipefail
    just _pytest bindings/python/tests
    for d in examples/python/*/; do
      [ -f "${d}main.py" ] || continue
      if [[ -f "${d}requirements.txt" ]]; then
        {{ python_path }} -m pip install -r "${d}requirements.txt"
      fi
      just _pytest "$d"
    done
[windows]
test-python-release: python-develop-release
    {{ python_path }} {{ just_helper }} test-python

# Full Python test suite (debug and release).
test-python-full: test-python-debug test-python-release

# Python unit tests only (debug).
test-python-unit-debug: python-develop-debug
    just _pytest bindings/python/tests/unit

# Python unit tests only (release).
test-python-unit-release: python-develop-release
    just _pytest bindings/python/tests/unit

# Python unit tests only (debug and release).
test-python-unit-full: test-python-unit-debug test-python-unit-release

# Python integration test only (debug).
test-python-integration-debug: python-develop-debug
    just _pytest bindings/python/tests/integration

# Python integration test only (release).
test-python-integration-release: python-develop-release
    just _pytest bindings/python/tests/integration

# Python integration test only (debug and release).
test-python-integration-full: test-python-integration-debug test-python-integration-release

# Run a workspace Python example from examples/python against local sources (debug).
run-examples-python-debug: python-develop-debug
    {{ python_path }} examples/python/rate_pnl_killswitch/main.py
    {{ python_path }} examples/python/spot_funds/main.py
    just run-examples-python-table-debug examples/tables/spot/coverage.md

# Run a workspace Python example from examples/python against local sources (release).
run-examples-python-release: python-develop-release
    {{ python_path }} examples/python/rate_pnl_killswitch/main.py
    {{ python_path }} examples/python/spot_funds/main.py
    just run-examples-python-table-release examples/tables/spot/coverage.md

# Run a spot-policy scenario table through the Python spot_table example (debug).
[unix]
run-examples-python-table-debug test_file="examples/tables/spot/coverage.md": python-develop-debug
    {{ python_path }} examples/python/spot_table/main.py --table $(pwd)/{{ test_file }}
[windows]
run-examples-python-table-debug test_file="examples/tables/spot/coverage.md": python-develop-debug
    {{ python_path }} {{ just_helper }} run-python-spot-table "{{ test_file }}"

# Run a spot-policy scenario table through the Python spot_table example (release).
[unix]
run-examples-python-table-release test_file="examples/tables/spot/coverage.md": python-develop-release
    {{ python_path }} examples/python/spot_table/main.py --table $(pwd)/{{ test_file }}
[windows]
run-examples-python-table-release test_file="examples/tables/spot/coverage.md": python-develop-release
    {{ python_path }} {{ just_helper }} run-python-spot-table "{{ test_file }}"

# Repeat-run a scenario table through the Python example for `dur` (debug).
[unix]
run-examples-python-table-repeat-debug test_file="examples/tables/spot/coverage.md" dur="3m": python-develop-debug
    {{ python_path }} examples/python/spot_table/main.py --table $(pwd)/{{ test_file }} --min-duration {{ dur }}
[windows]
run-examples-python-table-repeat-debug test_file="examples/tables/spot/coverage.md" dur="3m": python-develop-debug
    {{ python_path }} {{ just_helper }} run-python-spot-table "{{ test_file }}" --min-duration "{{ dur }}"

# Repeat-run a scenario table through the Python example for `dur` (release).
[unix]
run-examples-python-table-repeat-release test_file="examples/tables/spot/coverage.md" dur="3m": python-develop-release
    {{ python_path }} examples/python/spot_table/main.py --table $(pwd)/{{ test_file }} --min-duration {{ dur }}
[windows]
run-examples-python-table-repeat-release test_file="examples/tables/spot/coverage.md" dur="3m": python-develop-release
    {{ python_path }} {{ just_helper }} run-python-spot-table "{{ test_file }}" --min-duration "{{ dur }}"

# Full Go test suite (debug).
test-go-debug:
    just _go debug go test ./...
    just _go-examples debug go test ./...

# Full Go test suite (release runtime).
test-go-release:
    just _go release go test ./...
    just _go-examples release go test ./...

# Full Go test suite (debug and release runtime, plus race instrumentation).
test-go-full: test-go-debug test-go-release test-go-race

# Go race test suite. Race instrumentation is debug-only.
[unix]
test-go-race:
    just _go debug go test -race ./...
    just _go-examples debug go test -race ./...
[windows]
test-go-race:
    @echo Skipping Go race tests on Windows: Go ThreadSanitizer is not compatible with the LLVM/MSVC CGo toolchain.

# Go tests with actionable coverage summary. Coverage is instrumentation-only.
[unix]
test-go-cov:
    just _go debug go test -coverprofile=coverage.out -coverpkg=./... ./...
    cd bindings/go && go tool cover -func=coverage.out | grep -v '100.0%'
[windows]
test-go-cov: _ensure-python-env
    {{ python_path }} {{ just_helper }} go-in debug bindings/go go test -coverprofile=coverage.out -coverpkg=./... ./...
    {{ python_path }} {{ just_helper }} go-cover-summary

# Run workspace Go examples from examples/go against local sources (debug).
run-examples-go-debug:
    just _go-in debug examples/go/rate_pnl_killswitch go run .
    just _go-in debug examples/go/spot_funds go run .
    just run-examples-go-table-debug

# Run workspace Go examples from examples/go against local sources (release runtime).
run-examples-go-release:
    just _go-in release examples/go/rate_pnl_killswitch go run .
    just _go-in release examples/go/spot_funds go run .
    just run-examples-go-table-release

# Run a spot-policy scenario table through the spot_table example (debug).
[unix]
run-examples-go-table-debug test_file="examples/tables/spot/coverage.md":
    just _go-in debug examples/go/spot_table go run . -table $(pwd)/{{ test_file }}
[windows]
run-examples-go-table-debug test_file="examples/tables/spot/coverage.md":
    just _go-embed-runtime debug
    {{ python_path }} {{ just_helper }} go-in debug "examples/go/spot_table" go run . -table "%CD%\{{ test_file }}"

# Run a spot-policy scenario table through the spot_table example (release).
[unix]
run-examples-go-table-release test_file="examples/tables/spot/coverage.md":
    just _go-in release examples/go/spot_table go run . -table $(pwd)/{{ test_file }}
[windows]
run-examples-go-table-release test_file="examples/tables/spot/coverage.md":
    just _go-embed-runtime release
    {{ python_path }} {{ just_helper }} go-in release "examples/go/spot_table" go run . -table "%CD%\{{ test_file }}"

# Repeat-run a scenario table for `dur` (debug).
[unix]
run-examples-go-table-repeat-debug test_file="examples/tables/spot/coverage.md" dur="3m":
    just _go-in debug examples/go/spot_table go run . -table $(pwd)/{{ test_file }} -min-duration {{ dur }}
[windows]
run-examples-go-table-repeat-debug test_file="examples/tables/spot/coverage.md" dur="3m":
    just _go-embed-runtime debug
    {{ python_path }} {{ just_helper }} go-in debug "examples/go/spot_table" go run . -table "%CD%\{{ test_file }}" -min-duration {{ dur }}

# Repeat-run a scenario table for `dur` (release).
[unix]
run-examples-go-table-repeat-release test_file="examples/tables/spot/coverage.md" dur="3m":
    just _go-in release examples/go/spot_table go run . -table $(pwd)/{{ test_file }} -min-duration {{ dur }}
[windows]
run-examples-go-table-repeat-release test_file="examples/tables/spot/coverage.md" dur="3m":
    just _go-embed-runtime release
    {{ python_path }} {{ just_helper }} go-in release "examples/go/spot_table" go run . -table "%CD%\{{ test_file }}" -min-duration {{ dur }}

# Compile C examples embedded in public README files.
[unix]
test-c-examples:
    awk 'BEGIN { in_block = 0; first_block = 1 } /^```c$/ { in_block = 1; if (!first_block) print ""; first_block = 0; next } /^```$/ && in_block { in_block = 0; next } in_block { print }' bindings/c/README.md > /tmp/openpit_readme_example.c
    cc -std=c11 -fsyntax-only -I bindings/c /tmp/openpit_readme_example.c
[windows]
test-c-examples: _ensure-python-env
    {{ python_path }} {{ just_helper }} compile-c-readme-examples

# Configure and build the C++ binding against the debug FFI runtime.
build-cpp-debug:
    just _build-cpp debug

# Configure and build the C++ binding against the release FFI runtime.
build-cpp-release:
    just _build-cpp release

# Configure and build the C++ examples against the debug FFI runtime.
build-examples-cpp-debug:
    just _build-examples-cpp debug

# Configure and build the C++ examples against the release FFI runtime.
build-examples-cpp-release:
    just _build-examples-cpp release

# Run the C++ binding tests via ctest (debug).
[unix]
test-cpp-debug: build-cpp-debug
    ctest --test-dir bindings/cpp/build-debug --output-on-failure
[windows]
test-cpp-debug: build-cpp-debug
    {{ python_path }} {{ just_helper }} ctest bindings/cpp/build-debug debug

# Run the C++ binding tests via ctest (release).
[unix]
test-cpp-release: build-cpp-release
    ctest --test-dir bindings/cpp/build-release --output-on-failure --build-config Release
[windows]
test-cpp-release: build-cpp-release
    {{ python_path }} {{ just_helper }} ctest bindings/cpp/build-release release

# Run the C++ binding tests via ctest (debug and release).
test-cpp-full: test-cpp-debug test-cpp-release

# Run the C++ examples end-to-end against local sources (debug).
[unix]
run-examples-cpp-debug: build-examples-cpp-debug
    ./examples/cpp/build-debug/rate_pnl_killswitch/rate_pnl_killswitch
    ./examples/cpp/build-debug/spot_funds/spot_funds
    just run-examples-cpp-table-debug
[windows]
run-examples-cpp-debug: build-examples-cpp-debug
    {{ python_path }} {{ just_helper }} run-cpp debug rate_pnl_killswitch
    {{ python_path }} {{ just_helper }} run-cpp debug spot_funds
    just run-examples-cpp-table-debug

# Run the C++ examples end-to-end against local sources (release).
[unix]
run-examples-cpp-release: build-examples-cpp-release
    ./examples/cpp/build-release/rate_pnl_killswitch/rate_pnl_killswitch
    ./examples/cpp/build-release/spot_funds/spot_funds
    just run-examples-cpp-table-release
[windows]
run-examples-cpp-release: build-examples-cpp-release
    {{ python_path }} {{ just_helper }} run-cpp release rate_pnl_killswitch
    {{ python_path }} {{ just_helper }} run-cpp release spot_funds
    just run-examples-cpp-table-release

# Run a spot-policy scenario table through the C++ spot_table example (debug).
[unix]
run-examples-cpp-table-debug test_file="examples/tables/spot/coverage.md": build-examples-cpp-debug
    ./examples/cpp/build-debug/spot_table/spot_table --table "$(pwd)/{{ test_file }}"
[windows]
run-examples-cpp-table-debug test_file="examples/tables/spot/coverage.md": build-examples-cpp-debug
    {{ python_path }} {{ just_helper }} run-cpp debug spot_table --table "{{ test_file }}"

# Run a spot-policy scenario table through the C++ spot_table example (release).
[unix]
run-examples-cpp-table-release test_file="examples/tables/spot/coverage.md": build-examples-cpp-release
    ./examples/cpp/build-release/spot_table/spot_table --table "$(pwd)/{{ test_file }}"
[windows]
run-examples-cpp-table-release test_file="examples/tables/spot/coverage.md": build-examples-cpp-release
    {{ python_path }} {{ just_helper }} run-cpp release spot_table --table "{{ test_file }}"

# Build the C++ examples and run each example's gtest smoke test via ctest (debug).
[unix]
test-examples-cpp-debug: build-examples-cpp-debug
    ctest --test-dir examples/cpp/build-debug --output-on-failure
[windows]
test-examples-cpp-debug: build-examples-cpp-debug
    {{ python_path }} {{ just_helper }} ctest examples/cpp/build-debug debug

# Build the C++ examples and run each example's gtest smoke test via ctest (release).
[unix]
test-examples-cpp-release: build-examples-cpp-release
    ctest --test-dir examples/cpp/build-release --output-on-failure --build-config Release
[windows]
test-examples-cpp-release: build-examples-cpp-release
    {{ python_path }} {{ just_helper }} ctest examples/cpp/build-release release

# Build the C++ examples and run gtest smoke tests via ctest (debug and release).
test-examples-cpp-full: test-examples-cpp-debug test-examples-cpp-release

# Format C++ sources in place.
[unix]
fmt-cpp:
    find bindings/cpp/include bindings/cpp/test e2e/clients/cpp -type f \( -name '*.hpp' -o -name '*.cpp' \) -print0 | xargs -0 clang-format -i
    find examples/cpp -type f \( -name '*.hpp' -o -name '*.cpp' \) -not -path '*/build/*' -not -path '*/build-*/*' -print0 | xargs -0 clang-format -i
[windows]
fmt-cpp: _ensure-python-env
    {{ python_path }} {{ just_helper }} cpp-format

# Check C++ formatting without modifying files.
[unix]
fmt-check-cpp:
    find bindings/cpp/include bindings/cpp/test e2e/clients/cpp -type f \( -name '*.hpp' -o -name '*.cpp' \) -print0 | xargs -0 clang-format --dry-run -Werror
    find examples/cpp -type f \( -name '*.hpp' -o -name '*.cpp' \) -not -path '*/build/*' -not -path '*/build-*/*' -print0 | xargs -0 clang-format --dry-run -Werror
[windows]
fmt-check-cpp: _ensure-python-env
    {{ python_path }} {{ just_helper }} cpp-format --check

# Lint C++ sources with clang-tidy (requires debug build dirs).
[unix]
lint-cpp: build-cpp-debug build-examples-cpp-debug
    #!/usr/bin/env bash
    set -euo pipefail
    repo="$(pwd)"
    tidy_args=(--extra-arg=-std=gnu++17)
    if [[ "$(uname -s)" == "Darwin" ]]; then
      tidy_args+=(--extra-arg=-isysroot --extra-arg="$(xcrun --show-sdk-path)")
    fi
    cmake -S bindings/cpp -B bindings/cpp/build-debug -DCMAKE_BUILD_TYPE=Debug -DCMAKE_EXPORT_COMPILE_COMMANDS=ON
    header_filter="[{\"name\":\"${repo}/bindings/cpp/include/.*\"}]"
    find bindings/cpp/test -type f -name '*.cpp' -print0 | xargs -0 clang-tidy "${tidy_args[@]}" --config-file bindings/cpp/.clang-tidy -p bindings/cpp/build-debug --line-filter="$header_filter"
    cmake -S examples/cpp -B examples/cpp/build-debug -DCMAKE_BUILD_TYPE=Debug -DCMAKE_EXPORT_COMPILE_COMMANDS=ON
    find examples/cpp -type f -name '*.cpp' -not -path '*/build/*' -not -path '*/build-*/*' -not -path '*/test/*' -not -name '*_test.cpp' -print0 | xargs -0 clang-tidy "${tidy_args[@]}" --config-file bindings/cpp/.clang-tidy -p examples/cpp/build-debug
[windows]
lint-cpp: build-cpp-debug build-examples-cpp-debug
    {{ python_path }} {{ just_helper }} lint-cpp

# Generate the Doxygen-backed C++ API reference committed under docs/cpp-api.
[unix]
gen-docs-cpp:
    #!/usr/bin/env bash
    set -euo pipefail
    command -v doxygen >/dev/null || {
      echo "doxygen is required to generate docs/cpp-api" >&2
      exit 1
    }
    command -v dot >/dev/null || {
      echo "graphviz dot is required to generate docs/cpp-api" >&2
      exit 1
    }
    rm -rf docs/cpp-api
    doxygen bindings/cpp/Doxyfile
    {{ python_path }} scripts/_generate_api_c_sitemap.py
[windows]
gen-docs-cpp: _ensure-python-env
    {{ python_path }} {{ just_helper }} gen-docs-cpp

# Format all.
[parallel]
fmt-all: fmt-rust fmt-python fmt-go fmt-cpp

# Format Rust.
fmt-rust:
    cargo fmt --all

# Format Python.
fmt-python: _ensure-python-env
    {{ python_path }} -m black --config {{ python_config }} . -q

# Format Go.
fmt-go:
    cd bindings/go && gofmt -w .
    gofmt -w examples/go

# Prepare new release (kind is patch, minor or major).
release kind: check-full
    cargo release {{ kind }} --execute --no-confirm

# Push the current HEAD to the dry-run branch and start the staging release workflow.
release-dry:
    git push --force-with-lease origin HEAD:release-dry-run
    gh workflow run release.yml --ref release-dry-run -f dry_run=true

# Install Python bindings into the current Python environment (debug build).
[unix]
python-develop-debug: _ensure-python-env
    {{ python_path }} -m maturin develop -q --manifest-path bindings/python/Cargo.toml
[windows]
python-develop-debug: _ensure-python-env
    {{ python_path }} {{ just_helper }} python-develop debug

# Install Python bindings into the current Python environment (release build).
[unix]
python-develop-release: _ensure-python-env
    {{ python_path }} -m maturin develop -q --release --manifest-path bindings/python/Cargo.toml
[windows]
python-develop-release: _ensure-python-env
    {{ python_path }} {{ just_helper }} python-develop release

# Generate the C header and Markdown docs for the FFI crate.
[unix]
gen-api-c: _ensure-python-env
    {{ python_path }} scripts/generate_api_c.py > /dev/null
[windows]
gen-api-c: _ensure-python-env
    {{ python_path }} scripts/generate_api_c.py > NUL

# Generate derived API and reference artifacts.
gen-all: gen-api-c gen-docs-cpp

# Build FFI in the requested mode.
[unix]
_build-ffi mode:
    #!/usr/bin/env bash
    set -euo pipefail
    profile="{{ mode }}"
    case "$profile" in
      debug) cargo_args=() ;;
      release) cargo_args=(--release) ;;
      *) echo "unsupported build mode: $profile" >&2; exit 1 ;;
    esac
    case "$(uname -s)" in
      MINGW*|MSYS*|CYGWIN*)
        export RUSTFLAGS="${RUSTFLAGS:+${RUSTFLAGS} }-C target-feature=+crt-static"
        ;;
    esac
    cargo build -p openpit-ffi "${cargo_args[@]}" --locked -q
[windows]
_build-ffi mode: _ensure-python-env
    {{ python_path }} {{ just_helper }} build-ffi {{ mode }}

# Configure and build the C++ binding in the requested mode.
[unix]
_build-cpp mode:
    #!/usr/bin/env bash
    set -euo pipefail
    profile="{{ mode }}"
    case "$profile" in
      debug) config=Debug ;;
      release) config=Release ;;
      *) echo "unsupported build mode: $profile" >&2; exit 1 ;;
    esac
    just _build-ffi "$profile"
    case "$(uname -s)" in
      Darwin) lib="$(pwd)/target/${profile}/libopenpit_ffi.dylib" ;;
      Linux)  lib="$(pwd)/target/${profile}/libopenpit_ffi.so" ;;
      MINGW*|MSYS*|CYGWIN*)
        lib="$(pwd)/target/${profile}/openpit_ffi.dll"
        implib="${lib}.lib"
        ;;
      *) echo "unsupported OS for pit-ffi runtime lookup" >&2; exit 1 ;;
    esac
    cmake_args=(-DOPENPIT_RUNTIME_LIBRARY="$lib" -DCMAKE_BUILD_TYPE="$config")
    if [[ -n "${implib:-}" ]]; then
      cmake_args+=(-DOPENPIT_RUNTIME_IMPORT_LIBRARY="$implib")
    fi
    cmake -S bindings/cpp -B "bindings/cpp/build-${profile}" "${cmake_args[@]}"
    cmake --build "bindings/cpp/build-${profile}" --config "$config"
[windows]
_build-cpp mode:
    just _build-ffi {{ mode }}
    {{ python_path }} {{ just_helper }} build-cpp binding {{ mode }}

# Configure and build the C++ examples in the requested mode.
[unix]
_build-examples-cpp mode:
    #!/usr/bin/env bash
    set -euo pipefail
    profile="{{ mode }}"
    case "$profile" in
      debug) config=Debug ;;
      release) config=Release ;;
      *) echo "unsupported build mode: $profile" >&2; exit 1 ;;
    esac
    just _build-ffi "$profile"
    case "$(uname -s)" in
      Darwin) lib="$(pwd)/target/${profile}/libopenpit_ffi.dylib" ;;
      Linux)  lib="$(pwd)/target/${profile}/libopenpit_ffi.so" ;;
      MINGW*|MSYS*|CYGWIN*)
        lib="$(pwd)/target/${profile}/openpit_ffi.dll"
        implib="${lib}.lib"
        ;;
      *) echo "unsupported OS for pit-ffi runtime lookup" >&2; exit 1 ;;
    esac
    cmake_args=(-DOPENPIT_RUNTIME_LIBRARY="$lib" -DCMAKE_BUILD_TYPE="$config")
    if [[ -n "${implib:-}" ]]; then
      cmake_args+=(-DOPENPIT_RUNTIME_IMPORT_LIBRARY="$implib")
    fi
    cmake -S examples/cpp -B "examples/cpp/build-${profile}" "${cmake_args[@]}"
    cmake --build "examples/cpp/build-${profile}" --config "$config"
[windows]
_build-examples-cpp mode:
    just _build-ffi {{ mode }}
    {{ python_path }} {{ just_helper }} build-cpp examples {{ mode }}

# Run a Go command in the bindings/go module with FFI runtime path configured.
_go mode +args:
    just _go-in {{ mode }} bindings/go {{ args }}

# Run a Go command in every examples/go module, with the FFI runtime configured.
[unix]
_go-examples mode +args:
    #!/usr/bin/env bash
    set -euo pipefail
    profile="{{ mode }}"
    case "$profile" in debug|release) ;; *) echo "unsupported build mode: $profile" >&2; exit 1 ;; esac
    just _go-embed-runtime "$profile"
    case "$(uname -s)" in
      Darwin) lib="$(pwd)/target/${profile}/libopenpit_ffi.dylib" ;;
      Linux)  lib="$(pwd)/target/${profile}/libopenpit_ffi.so" ;;
      *) echo "unsupported OS for pit-ffi runtime lookup" >&2; exit 1 ;;
    esac
    export OPENPIT_RUNTIME_LIBRARY_PATH="$lib"
    for d in examples/go/*/; do
      [ -f "${d}go.mod" ] || continue
      echo ">> ${d}" >&2
      ( cd "$d" && {{ args }} )
    done
[windows]
_go-examples mode +args: _ensure-python-env
    just _go-embed-runtime {{ mode }}
    {{ python_path }} {{ just_helper }} go-examples {{ mode }} {{ args }}

# Place the freshly built FFI runtime into the Go embed tree.
[unix]
_go-embed-runtime mode:
    #!/usr/bin/env bash
    set -euo pipefail
    profile="{{ mode }}"
    case "$profile" in debug|release) ;; *) echo "unsupported build mode: $profile" >&2; exit 1 ;; esac
    just _build-ffi "$profile"
    os="$(uname -s)"; arch="$(uname -m)"
    case "$arch" in aarch64|arm64) arch=arm64 ;; x86_64|amd64) arch=amd64 ;; esac
    case "$os" in
      Darwin) plat="darwin-$arch"; lib="libopenpit_ffi.dylib" ;;
      Linux)  plat="linux-$arch";  lib="libopenpit_ffi.so" ;;
      *) echo "unsupported OS for go runtime embed: $os" >&2; exit 1 ;;
    esac
    cp "target/${profile}/$lib" "bindings/go/internal/runtime/$plat/$lib"
[windows]
_go-embed-runtime mode: _ensure-python-env
    just _build-ffi {{ mode }}
    {{ python_path }} {{ just_helper }} go-embed-runtime {{ mode }}

# Run a Go command in a workspace-level subdirectory with FFI runtime path configured.
[unix]
_go-in mode dir +args:
    #!/usr/bin/env bash
    set -euo pipefail
    profile="{{ mode }}"
    case "$profile" in debug|release) ;; *) echo "unsupported build mode: $profile" >&2; exit 1 ;; esac
    just _go-embed-runtime "$profile"
    case "$(uname -s)" in
      Darwin) lib="$(pwd)/target/${profile}/libopenpit_ffi.dylib" ;;
      Linux)  lib="$(pwd)/target/${profile}/libopenpit_ffi.so" ;;
      *) echo "unsupported OS for pit-ffi runtime lookup" >&2; exit 1 ;;
    esac
    ( cd "{{ dir }}" && OPENPIT_RUNTIME_LIBRARY_PATH="$lib" {{ args }} )
[windows]
_go-in mode dir +args: _ensure-python-env
    just _go-embed-runtime {{ mode }}
    {{ python_path }} {{ just_helper }} go-in {{ mode }} "{{ dir }}" {{ args }}
