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

work_root="/tmp/openpit-cpp-vcpkg-release-e2e"
examples_root="/tmp/openpit-cpp-vcpkg-examples"
vcpkg_root="${work_root}/vcpkg"
release_api="https://api.github.com/repos/${OPENPIT_RELEASE_REPOSITORY:-openpitkit/pit}"
release_metadata="${work_root}/release.json"

rm -rf "${work_root}" "${examples_root}"
mkdir -p "${work_root}"

download_release_metadata() {
  local curl_args=(-fsSL)
  if [[ -n "${OPENPIT_RELEASE_DOWNLOAD_TOKEN:-}" ]]; then
    curl_args+=(
      --header "Authorization: Bearer ${OPENPIT_RELEASE_DOWNLOAD_TOKEN}"
      --header "Accept: application/vnd.github+json"
    )
  fi
  curl "${curl_args[@]}" \
    "${release_api}/releases/tags/v${OPENPIT_VERSION}" \
    -o "${release_metadata}"
}

download_draft_runtime() {
  if [[ -z "${OPENPIT_RELEASE_DOWNLOAD_TOKEN:-}" ]]; then
    return
  fi

  local asset="openpit-ffi--linux-amd64-libopenpit_ffi.so"
  download_release_metadata
  local asset_id
  asset_id="$(python3 - "${release_metadata}" "${asset}" <<'PY'
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
  OPENPIT_VCPKG_RUNTIME_LIBRARY="${work_root}/libopenpit_ffi.so"
  curl -fsSL \
    --header "Authorization: Bearer ${OPENPIT_RELEASE_DOWNLOAD_TOKEN}" \
    --header "Accept: application/octet-stream" \
    "${release_api}/releases/assets/${asset_id}" \
    -o "${OPENPIT_VCPKG_RUNTIME_LIBRARY}"
  local sha_asset_id
  sha_asset_id="$(python3 - "${release_metadata}" "${asset}.sha256" <<'PY'
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
    "${release_api}/releases/assets/${sha_asset_id}" \
    -o "${OPENPIT_VCPKG_RUNTIME_LIBRARY}.sha256"
  expected_sha="$(tr -d '[:space:]' < "${OPENPIT_VCPKG_RUNTIME_LIBRARY}.sha256")"
  actual_sha="$(sha256sum "${OPENPIT_VCPKG_RUNTIME_LIBRARY}" | awk '{print $1}')"
  if [[ "${actual_sha}" != "${expected_sha}" ]]; then
    echo "sha256 mismatch for ${asset}" >&2
    exit 1
  fi
  export OPENPIT_VCPKG_RUNTIME_LIBRARY
}

resolve_registry_baseline() {
  if [[ -n "${OPENPIT_VCPKG_REGISTRY_PATH:-}" \
      || -n "${OPENPIT_VCPKG_REGISTRY_BASELINE:-}" ]]; then
    return
  fi

  if [[ ! -f "${release_metadata}" ]]; then
    download_release_metadata
  fi
  OPENPIT_VCPKG_REGISTRY_BASELINE="$(python3 - "${release_metadata}" <<'PY'
import json
import re
import sys

with open(sys.argv[1], encoding="utf-8") as metadata_file:
    release = json.load(metadata_file)
match = re.search(
    r"vcpkg-registry\.git at baseline\s+`([0-9a-f]{40})`",
    release.get("body") or "",
)
if match is None:
    raise SystemExit(
        "could not resolve the OpenPit vcpkg registry baseline from release notes"
    )
print(match.group(1))
PY
)"
  export OPENPIT_VCPKG_REGISTRY_BASELINE
}

write_vcpkg_configuration() {
  if [[ -n "${OPENPIT_VCPKG_REGISTRY_PATH:-}" ]]; then
    cat > "${work_root}/vcpkg-configuration.json" <<EOF
{
  "registries": [
    {
      "kind": "filesystem",
      "path": "${OPENPIT_VCPKG_REGISTRY_PATH}",
      "baseline": "default",
      "packages": ["openpit"]
    }
  ]
}
EOF
    return
  fi

  cat > "${work_root}/vcpkg-configuration.json" <<EOF
{
  "registries": [
    {
      "kind": "git",
      "repository": "${OPENPIT_VCPKG_REGISTRY_REPOSITORY:-https://github.com/openpitkit/vcpkg-registry.git}",
      "baseline": "${OPENPIT_VCPKG_REGISTRY_BASELINE}",
      "packages": ["openpit"]
    }
  ]
}
EOF
}

download_draft_runtime

echo "==> Bootstrapping isolated vcpkg"
git clone --depth 1 https://github.com/microsoft/vcpkg.git "${vcpkg_root}"
"${vcpkg_root}/bootstrap-vcpkg.sh" -disableMetrics
vcpkg_baseline="$(git -C "${vcpkg_root}" rev-parse HEAD)"
# Source: bindings/cpp/README.md - Getting Started / vcpkg
cat > "${work_root}/vcpkg.json" <<EOF
{
  "name": "openpit-release-e2e-cpp-vcpkg",
  "version-string": "0",
  "builtin-baseline": "${vcpkg_baseline}",
  "dependencies": ["openpit"]
}
EOF
resolve_registry_baseline
write_vcpkg_configuration

echo "==> Installing OpenPit from vcpkg"
(
  cd "${work_root}"
  "${vcpkg_root}/vcpkg" install --triplet x64-linux
)

cmake_args=(
  "-DCMAKE_TOOLCHAIN_FILE=${vcpkg_root}/scripts/buildsystems/vcpkg.cmake"
  "-DVCPKG_MANIFEST_DIR=${work_root}"
  "-DVCPKG_INSTALLED_DIR=${work_root}/vcpkg_installed"
  "-DVCPKG_TARGET_TRIPLET=x64-linux"
)
if [[ -n "${OPENPIT_VCPKG_RUNTIME_LIBRARY:-}" ]]; then
  cmake_args+=("-DOPENPIT_RUNTIME_LIBRARY=${OPENPIT_VCPKG_RUNTIME_LIBRARY}")
fi

echo "==> Building minimal C++ consumer through vcpkg"
cmake -S /opt/e2e/cpp-consumer -B "${work_root}/consumer-build" \
  "${cmake_args[@]}"
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

  echo "==> Building C++ example ${name} through vcpkg"
  cmake -S "${src}" -B "${build}" "${cmake_args[@]}" "$@"
  cmake --build "${build}" --parallel

  echo "==> Testing C++ example ${name} through vcpkg"
  ctest --test-dir "${build}" --output-on-failure
}

run_example "rate_pnl_killswitch" \
  -DOPENPIT_EXAMPLE_USE_INSTALLED_PACKAGE=ON
run_example "spot_funds"
run_example "spot_table" \
  -DOPENPIT_USE_FIND_PACKAGE=ON
run_example "spot_loadtest" \
  -DOPENPIT_USE_FIND_PACKAGE=ON
