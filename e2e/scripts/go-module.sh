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

: "${OPENPIT_VERSION:?OPENPIT_VERSION is required}"

rm -rf /tmp/openpit-go-consumer
mkdir -p /tmp/openpit-go-consumer
cp -R /opt/e2e/go-consumer/. /tmp/openpit-go-consumer

cd /tmp/openpit-go-consumer
sed "s/__OPENPIT_VERSION__/${OPENPIT_VERSION}/g" go.mod.in > go.mod
rm go.mod.in

go mod download go.openpit.dev/openpit
module_dir="$(go list -m -f '{{.Dir}}' go.openpit.dev/openpit)"
goos="$(go env GOOS)"
goarch="$(go env GOARCH)"
case "${goos}" in
  linux)
    runtime_lib="libpit_ffi.so"
    ;;
  darwin)
    runtime_lib="libpit_ffi.dylib"
    ;;
  *)
    echo "unsupported Go release e2e platform: ${goos}/${goarch}" >&2
    exit 1
    ;;
esac
runtime_dir="${module_dir}/internal/runtime/${goos}-${goarch}"
runtime_path="${runtime_dir}/${runtime_lib}"
if [[ ! -f "${runtime_path}" ]]; then
  echo "published Go module runtime library not found: ${runtime_path}" >&2
  exit 1
fi

export CGO_LDFLAGS="-L${runtime_dir} -lpit_ffi -Wl,-rpath,${runtime_dir}"
export OPENPIT_RUNTIME_LIBRARY_PATH="${runtime_path}"

go mod tidy
go test ./...
