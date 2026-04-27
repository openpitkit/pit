#!/usr/bin/env bash
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

set -euo pipefail

ROOT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
VERSION="${1:-}"

if [[ -z "${VERSION}" ]]; then
  echo "Usage: $0 <version>"
  echo "Example: $0 0.1.0"
  exit 1
fi

run_case() {
  local name="$1"
  local dockerfile="$2"
  local platform="${3:-}"
  local image="openpit-release-e2e-${name}:${VERSION}"
  local build_args=()
  local run_args=()

  if [[ -n "${platform}" ]]; then
    build_args+=(--platform "${platform}")
    run_args+=(--platform "${platform}")
  fi

  echo
  echo "==> Building ${name} image"
  docker build --pull \
    "${build_args[@]}" \
    --tag "${image}" \
    --file "${dockerfile}" \
    "${ROOT_DIR}"

  echo "==> Running ${name} checks"
  docker run --rm \
    "${run_args[@]}" \
    --env OPENPIT_VERSION="${VERSION}" \
    "${image}"
}

failures=0
passes=0
passed_cases=()
failed_cases=()

print_banner() {
  local title="$1"
  printf '\n%s\n' "============================================================"
  printf '%s\n' "${title}"
  printf '%s\n' "============================================================"
}

run_or_record() {
  local name="$1"
  local dockerfile="$2"
  local platform="${3:-}"

  if run_case "${name}" "${dockerfile}" "${platform}"; then
    passes=$((passes + 1))
    passed_cases+=("${name}")
    printf '==> %s checks passed\n' "${name}"
  else
    printf '==> %s checks failed\n' "${name}"
    failures=$((failures + 1))
    failed_cases+=("${name}")
  fi
}

print_banner "Release e2e for openpit ${VERSION}"

run_or_record "rust-amd64" "${ROOT_DIR}/e2e/env/docker/rust-crate/Dockerfile" "linux/amd64"
run_or_record "rust-arm64" "${ROOT_DIR}/e2e/env/docker/rust-crate/Dockerfile" "linux/arm64"
run_or_record "python-wheel-amd64" "${ROOT_DIR}/e2e/env/docker/python-wheel/Dockerfile" "linux/amd64"
run_or_record "python-wheel-arm64" "${ROOT_DIR}/e2e/env/docker/python-wheel/Dockerfile" "linux/arm64"
run_or_record "python-source-arm64" "${ROOT_DIR}/e2e/env/docker/python-sdist/Dockerfile" "linux/arm64"
run_or_record "go-amd64" "${ROOT_DIR}/e2e/env/docker/go-module/Dockerfile" "linux/amd64"

if [[ "${failures}" -ne 0 ]]; then
  print_banner "RELEASE E2E FAILED"
  printf 'Passed: %s\n' "${passes}"
  printf 'Failed: %s\n' "${failures}"
  printf 'Passing scenarios: %s\n' "${passed_cases[*]:-none}"
  printf 'Failing scenarios: %s\n' "${failed_cases[*]:-none}"
  exit 1
fi

print_banner "RELEASE E2E PASSED"
printf 'Passed: %s\n' "${passes}"
printf 'Failing scenarios: none\n'
printf 'Successful scenarios: %s\n' "${passed_cases[*]}"
