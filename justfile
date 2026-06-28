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

venv_dir := env_var_or_default("PIT_VIRTUAL_ENV", justfile_directory() / ".venv")
python_path := venv_dir / "bin/python"
export VIRTUAL_ENV := venv_dir
export PYO3_PYTHON := python_path
export PATH := venv_dir / "bin" + ":" + env_var("PATH")

# Rust build.
build:
    cargo build --workspace

# Build Go.
build-go:
    cd bindings/go && go build

# Format, generate, and lint and test the result.
check: (fmt-all) (gen-all) (check-dry)
# Lint and test the result (non-mutating).
check-dry: (lint-all) (test-all) (run-examples)
# Format, generate, and lint and test Rust.
check-rust: (fmt-all) (gen-api-c) (check-rust-dry)
# Lint and test Rust (non-mutating).
[parallel]
check-rust-dry: lint-rust test-rust
# Format, generate, and lint and test Go.
check-go: (fmt-all) (gen-api-c) (_check-go)
# Lint and test Go (non-mutating).
[parallel]
check-go-dry: lint-go test-go test-go-race
# Format, generate, and lint and test Python.
check-python: (fmt-all) (gen-api-c) (_check-python)
# Lint and test Python (non-mutating).
[parallel]
check-python-dry: (lint-python) (test-python)
    cargo nextest run -p openpit-python --locked --status-level fail --final-status-level fail
# Format, generate, and lint and test C++.
check-cpp: (fmt-cpp) (gen-docs-cpp) (lint-cpp) (test-cpp) (test-examples-cpp)
# Lint and test C++ (non-mutating).
check-cpp-dry: (lint-cpp) (test-cpp) (test-examples-cpp)

# Lint and test Go without the race detector.
[parallel]
_check-go: lint-go test-go

# Lint and test Python without the Rust wrapper package check.
[parallel]
_check-python: (lint-python) (test-python)

# Run all examples.
[parallel]
run-examples: run-examples-go run-examples-python run-examples-cpp

# Lint all.
[parallel]
lint-all: (lint-rust) (lint-python) (lint-go) (lint-cpp)
# Lint Rust.
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
test-all: test-rust test-python test-go test-go-race test-c-examples test-cpp test-examples-cpp

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
    _run_nextest run --workspace --exclude openpit-python --locked --status-level fail --final-status-level fail
    _run_nextest run -p openpit --all-features --locked --status-level fail --final-status-level fail
    # nextest does not run doctests; cover them via cargo test.
    cargo test --workspace --doc --locked
    cargo test -p openpit --all-features --doc --locked
# Rust tests with actionable coverage summary.
test-rust-cov:
    mkdir -p target/llvm-cov
    cargo llvm-cov test --workspace --exclude openpit-python --all-features --json --output-path target/llvm-cov/workspace.json
    python scripts/summarize_llvm_cov.py target/llvm-cov/workspace.json --output target/llvm-cov/workspace-summary.json --text
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
test-python: (python-develop)
    #!/usr/bin/env bash
    set -euo pipefail
    just _pytest bindings/python/tests
    for d in examples/python/*/; do
      [ -f "${d}main.py" ] || continue
      if [[ -f "${d}requirements.txt" ]]; then
        python -m pip install -r "${d}requirements.txt"
      fi
      just _pytest "$d"
    done
# Python unit tests only.
test-python-unit: (python-develop)
    just _pytest bindings/python/tests/unit
# Python integration test only.
test-python-integration: (python-develop)
    just _pytest bindings/python/tests/integration
# Run a workspace Python example from examples/python against local sources.
run-examples-python: (python-develop)
    python examples/python/rate_pnl_killswitch/main.py
    python examples/python/spot_funds/main.py
    just run-examples-python-table examples/tables/spot/coverage.md
# Run a spot-policy scenario table through the Python spot_table example.
run-examples-python-table test_file="examples/tables/spot/coverage.md": (python-develop)
    python examples/python/spot_table/main.py --table $(pwd)/{{ test_file }}
# Repeat-run a scenario table through the Python example for `dur` (default 3m).
run-examples-python-table-repeat test_file="examples/tables/spot/coverage.md" dur="3m": (python-develop)
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

# Configure and build the C++ binding against the locally built FFI runtime
# (no release download needed). Uses the local lib via OPENPIT_RUNTIME_LIBRARY.
build-cpp: _build-ffi
    #!/usr/bin/env bash
    set -euo pipefail
    case "$(uname -s)" in
      Darwin) lib="$(pwd)/target/release/libopenpit_ffi.dylib" ;;
      Linux)  lib="$(pwd)/target/release/libopenpit_ffi.so" ;;
      MINGW*|MSYS*|CYGWIN*)
        lib="$(pwd)/target/release/openpit_ffi.dll"
        implib="${lib}.lib"
        ;;
      *) echo "unsupported OS for pit-ffi runtime lookup" >&2; exit 1 ;;
    esac
    cmake_args=(-DOPENPIT_RUNTIME_LIBRARY="$lib")
    if [[ -n "${implib:-}" ]]; then
      cmake_args+=(-DOPENPIT_RUNTIME_IMPORT_LIBRARY="$implib")
    fi
    cmake -S bindings/cpp -B bindings/cpp/build "${cmake_args[@]}"
    cmake --build bindings/cpp/build

# Run the C++ binding tests via ctest.
test-cpp: build-cpp
    #!/usr/bin/env bash
    set -euo pipefail
    ctest_args=(--test-dir bindings/cpp/build --output-on-failure)
    case "$(uname -s)" in
      MINGW*|MSYS*|CYGWIN*) ctest_args+=(--build-config Debug) ;;
    esac
    ctest "${ctest_args[@]}"

# Configure and build all four C++ examples (and their gtest smoke tests)
# against the locally built FFI runtime, mirroring build-cpp. The examples/cpp
# aggregator embeds the binding once and shares it across every example.
build-examples-cpp: _build-ffi
    #!/usr/bin/env bash
    set -euo pipefail
    case "$(uname -s)" in
      Darwin) lib="$(pwd)/target/release/libopenpit_ffi.dylib" ;;
      Linux)  lib="$(pwd)/target/release/libopenpit_ffi.so" ;;
      MINGW*|MSYS*|CYGWIN*)
        lib="$(pwd)/target/release/openpit_ffi.dll"
        implib="${lib}.lib"
        ;;
      *) echo "unsupported OS for pit-ffi runtime lookup" >&2; exit 1 ;;
    esac
    cmake_args=(-DOPENPIT_RUNTIME_LIBRARY="$lib")
    if [[ -n "${implib:-}" ]]; then
      cmake_args+=(-DOPENPIT_RUNTIME_IMPORT_LIBRARY="$implib")
    fi
    cmake -S examples/cpp -B examples/cpp/build "${cmake_args[@]}"
    cmake --build examples/cpp/build

# Run the C++ examples end-to-end against local sources. Mirrors run-examples-go:
# build first, then run each scenario. spot_loadtest is a 2M-op benchmark (like
# the Go mirror it is omitted here); its reduced-scale smoke run is exercised by
# test-examples-cpp instead.
run-examples-cpp: build-examples-cpp
    ./examples/cpp/build/rate_pnl_killswitch/rate_pnl_killswitch
    ./examples/cpp/build/spot_funds/spot_funds
    just run-examples-cpp-table
# Run a spot-policy scenario table through the C++ spot_table example.
run-examples-cpp-table test_file="examples/tables/spot/coverage.md": build-examples-cpp
    ./examples/cpp/build/spot_table/spot_table --table "$(pwd)/{{ test_file }}"

# Build the C++ examples and run each example's gtest smoke test via ctest.
# Mirrors `just _go-examples "go test ./..."` for the C++ side.
test-examples-cpp: build-examples-cpp
    #!/usr/bin/env bash
    set -euo pipefail
    ctest_args=(--test-dir examples/cpp/build --output-on-failure)
    case "$(uname -s)" in
      MINGW*|MSYS*|CYGWIN*) ctest_args+=(--build-config Debug) ;;
    esac
    ctest "${ctest_args[@]}"

# Format C++ sources in place.
fmt-cpp:
    find bindings/cpp/include bindings/cpp/test e2e/clients/cpp -type f \( -name '*.hpp' -o -name '*.cpp' \) -print0 | xargs -0 clang-format -i
    find examples/cpp -type f \( -name '*.hpp' -o -name '*.cpp' \) -not -path '*/build/*' -print0 | xargs -0 clang-format -i

# Check C++ formatting without modifying files.
fmt-check-cpp:
    find bindings/cpp/include bindings/cpp/test e2e/clients/cpp -type f \( -name '*.hpp' -o -name '*.cpp' \) -print0 | xargs -0 clang-format --dry-run -Werror
    find examples/cpp -type f \( -name '*.hpp' -o -name '*.cpp' \) -not -path '*/build/*' -print0 | xargs -0 clang-format --dry-run -Werror

# Lint C++ sources with clang-tidy (requires a configured build dir for the
# compile database).
lint-cpp: build-cpp build-examples-cpp
    #!/usr/bin/env bash
    set -euo pipefail
    repo="$(pwd)"
    tidy_args=(--extra-arg=-std=gnu++17)
    if [[ "$(uname -s)" == "Darwin" ]]; then
      tidy_args+=(--extra-arg=-isysroot --extra-arg="$(xcrun --show-sdk-path)")
    fi
    cmake -S bindings/cpp -B bindings/cpp/build -DCMAKE_EXPORT_COMPILE_COMMANDS=ON
    header_filter="[{\"name\":\"${repo}/bindings/cpp/include/.*\"}]"
    find bindings/cpp/test -type f -name '*.cpp' -print0 | xargs -0 clang-tidy "${tidy_args[@]}" --config-file bindings/cpp/.clang-tidy -p bindings/cpp/build --line-filter="$header_filter"
    cmake -S examples/cpp -B examples/cpp/build -DCMAKE_EXPORT_COMPILE_COMMANDS=ON
    find examples/cpp -type f -name '*.cpp' -not -path '*/build/*' -not -path '*/test/*' -not -name '*_test.cpp' -print0 | xargs -0 clang-tidy "${tidy_args[@]}" --config-file bindings/cpp/.clang-tidy -p examples/cpp/build

# Generate the Doxygen-backed C++ API reference committed under docs/cpp-api.
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
    python scripts/_generate_api_c_sitemap.py

# Format all.
[parallel]
fmt-all: (fmt-rust) (fmt-python) (fmt-go) (fmt-cpp)
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
    python -m maturin develop -q --manifest-path bindings/python/Cargo.toml
# Install Python bindings into the current Python environment (release build).
python-develop-release:
    python -m maturin develop -q --release --manifest-path bindings/python/Cargo.toml

# Generate the C header and Markdown docs for the FFI crate.
gen-api-c:
    python scripts/generate_api_c.py > /dev/null

# Generate derived API and reference artifacts.
[parallel]
gen-all: (gen-api-c) (gen-docs-cpp)

# Build FFI.
_build-ffi:
    #!/usr/bin/env bash
    set -euo pipefail
    case "$(uname -s)" in
      MINGW*|MSYS*|CYGWIN*)
        export RUSTFLAGS="${RUSTFLAGS:+${RUSTFLAGS} }-C target-feature=+crt-static"
        ;;
    esac
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
