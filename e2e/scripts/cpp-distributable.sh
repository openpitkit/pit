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
# Please see https://openpit.dev and the OWNERS file for details.

set -euo pipefail

: "${OPENPIT_VERSION:?OPENPIT_VERSION is required}"

asset="openpit-cpp--${OPENPIT_VERSION}.tar.gz"
base_url="https://github.com/openpitkit/pit/releases/download/v${OPENPIT_VERSION}"
release_api="https://api.github.com/repos/${OPENPIT_RELEASE_REPOSITORY:-openpitkit/pit}"
release_metadata_url="${release_api}/releases/tags/v${OPENPIT_VERSION}"
if [[ -n "${OPENPIT_RELEASE_ID:-}" ]]; then
  if [[ ! "${OPENPIT_RELEASE_ID}" =~ ^[0-9]+$ ]]; then
    echo "OPENPIT_RELEASE_ID must be numeric" >&2
    exit 1
  fi
  release_metadata_url="${release_api}/releases/${OPENPIT_RELEASE_ID}"
fi
work_root="/tmp/openpit-cpp-release-e2e"
install_dir="${work_root}/install"
examples_root="/tmp/openpit-cpp-examples"

rm -rf "${work_root}" "${examples_root}"
mkdir -p "${work_root}" "${install_dir}"

download_release_asset() {
  local name="$1"
  local destination="$2"

  if [[ -z "${OPENPIT_RELEASE_DOWNLOAD_TOKEN:-}" ]]; then
    curl -fsSL "${base_url}/${name}" -o "${destination}"
    return
  fi

  local metadata="${work_root}/release.json"
  curl -fsSL \
    --header "Authorization: Bearer ${OPENPIT_RELEASE_DOWNLOAD_TOKEN}" \
    --header "Accept: application/vnd.github+json" \
    "${release_metadata_url}" \
    -o "${metadata}"
  local asset_id
  asset_id="$(python3 - "${metadata}" "${name}" <<'PY'
import json
import sys

metadata_path, asset_name = sys.argv[1:]
with open(metadata_path, encoding="utf-8") as metadata_file:
    release = json.load(metadata_file)
for release_asset in release["assets"]:
    if release_asset["name"] == asset_name:
        print(release_asset["id"])
        break
else:
    raise SystemExit(f"release asset not found: {asset_name}")
PY
)"
  curl -fsSL \
    --header "Authorization: Bearer ${OPENPIT_RELEASE_DOWNLOAD_TOKEN}" \
    --header "Accept: application/octet-stream" \
    "${release_api}/releases/assets/${asset_id}" \
    -o "${destination}"
}

verify_sha256() {
  local artifact="$1"
  local sidecar="$2"
  local expected_sha
  local actual_sha

  expected_sha="$(tr -d '[:space:]' < "${sidecar}")"
  if command -v sha256sum >/dev/null 2>&1; then
    actual_sha="$(sha256sum "${artifact}" | awk '{print $1}')"
  else
    actual_sha="$(shasum -a 256 "${artifact}" | awk '{print $1}')"
  fi
  if [[ "${actual_sha}" != "${expected_sha}" ]]; then
    echo "sha256 mismatch for ${artifact}" >&2
    echo "expected: ${expected_sha}" >&2
    echo "actual:   ${actual_sha}" >&2
    exit 1
  fi
}

echo "==> Downloading ${asset}"
download_release_asset "${asset}" "${work_root}/${asset}"
download_release_asset "${asset}.sha256" "${work_root}/${asset}.sha256"

verify_sha256 "${work_root}/${asset}" "${work_root}/${asset}.sha256"

tar -xzf "${work_root}/${asset}" -C "${install_dir}"

runtime_args=()
if [[ -n "${OPENPIT_RELEASE_DOWNLOAD_TOKEN:-}" ]]; then
  runtime_asset="openpit-ffi--linux-amd64-libopenpit_ffi.so"
  runtime_path="${work_root}/libopenpit_ffi.so"
  echo "==> Downloading ${runtime_asset} for the draft-release resolver"
  download_release_asset "${runtime_asset}" "${runtime_path}"
  download_release_asset "${runtime_asset}.sha256" "${runtime_path}.sha256"
  verify_sha256 "${runtime_path}" "${runtime_path}.sha256"
  runtime_args+=("-DOPENPIT_RUNTIME_LIBRARY=${runtime_path}")
fi

# Source: bindings/cpp/README.md - Getting Started / CMake find_package
echo "==> Building minimal C++ consumer"
cmake -S /opt/e2e/cpp-consumer -B "${work_root}/consumer-build" \
  -DCMAKE_PREFIX_PATH="${install_dir}" \
  "${runtime_args[@]}"
cmake --build "${work_root}/consumer-build" --parallel
"${work_root}/consumer-build/openpit_cpp_consumer"

mkdir -p "${examples_root}/cpp"
cp -R /opt/e2e/examples/. "${examples_root}/cpp"
cp -R /opt/e2e/tables "${examples_root}/tables"

run_example() {
  local name="$1"
  shift
  local src="${examples_root}/cpp/${name}"
  local build="${work_root}/examples/${name}"

  echo "==> Building C++ example ${name}"
  cmake -S "${src}" -B "${build}" \
    -DCMAKE_PREFIX_PATH="${install_dir}" \
    "${runtime_args[@]}" \
    "$@"
  cmake --build "${build}" --parallel

  echo "==> Testing C++ example ${name}"
  ctest --test-dir "${build}" --output-on-failure
}

run_example "rate_pnl_killswitch" \
  -DOPENPIT_EXAMPLE_USE_INSTALLED_PACKAGE=ON
run_example "spot_funds"
run_example "spot_table" \
  -DOPENPIT_USE_FIND_PACKAGE=ON
run_example "spot_loadtest" \
  -DOPENPIT_USE_FIND_PACKAGE=ON
