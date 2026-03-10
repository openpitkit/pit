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

# Lint all.
lint-all:
    just lint-rust
    just lint-python

# Lint Rus.
lint-rust:
    cargo clippy --all-targets --all-features
    RUSTDOCFLAGS="-D warnings" cargo doc --no-deps
    cargo fmt --all -- --check

# Lint Python.
lint-python:
    python -m ruff check bindings/python
    python -m ruff format --check bindings/python

# Run all tests.
test-all: test-rust test-python

# Rust tests.
test-rust:
    cargo test --workspace

# Rust tests with coverage (requires cargo-llvm-cov).
test-rust-cov:
    cargo llvm-cov --workspace --all-features

# Dry-run the release workflow locally.
test-release:
    printf '%s\n' '{"ref":"refs/tags/v0.1.0","ref_name":"v0.1.0"}' > /tmp/pit-release-event.json
    act push -W .github/workflows/release.yml -e /tmp/pit-release-event.json -P ubuntu-latest=ghcr.io/catthehacker/ubuntu:act-latest --container-architecture linux/amd64 --matrix name:manylinux2014

# Install Python bindings into the current Python environment (debug build).
python-develop:
    maturin develop --manifest-path bindings/python/Cargo.toml

# Install Python bindings into the current Python environment (release build).
python-develop-release:
    maturin develop --release --manifest-path bindings/python/Cargo.toml

# Shared pytest runner helper.
_pytest args:
    # shellcheck disable=SC1083
    python -m pytest {{ args }}

# Full Python test suite.
test-python: python-develop
    just _pytest bindings/python/tests

# Python unit tests only.
test-python-unit: python-develop
    just _pytest bindings/python/tests/unit

# Python integration test only.
test-python-integration: python-develop
    just _pytest bindings/python/tests/integration

# Prepare new release (kind is patch, minor or major).
release kind:
    cargo release {{ kind }} --execute
