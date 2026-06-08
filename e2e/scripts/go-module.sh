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

# 1. Minimal smoke consumer: ensures `go get go.openpit.dev/openpit` works.
rm -rf /tmp/openpit-go-consumer
mkdir -p /tmp/openpit-go-consumer
cp -R /opt/e2e/go-consumer/. /tmp/openpit-go-consumer

cd /tmp/openpit-go-consumer
sed "s/__OPENPIT_VERSION__/${OPENPIT_VERSION}/g" go.mod.in > go.mod
rm go.mod.in

go mod tidy
go test ./...

# 2. Real examples from examples/go/*: each is tested against the published
#    module by stripping its local `replace` directive and pinning the version.
#    This exercises exactly what an SDK consumer sees when they copy the
#    example from the repository.
#
#    Examples are laid out under a shared root that mirrors the repository:
#    each example lives at <root>/go/<name> with the scenario tables at
#    <root>/tables, so the examples' own `../../tables/...` references resolve
#    exactly as they do in the source tree.
examples_root="/tmp/openpit-go-examples"
rm -rf "${examples_root}"
mkdir -p "${examples_root}/go"
cp -R /opt/e2e/tables "${examples_root}/tables"

for example_src in /opt/e2e/examples/*/; do
  name="$(basename "${example_src}")"
  workdir="${examples_root}/go/${name}"
  echo "==> Testing example ${name} against go.openpit.dev/openpit ${OPENPIT_VERSION}"
  rm -rf "${workdir}"
  mkdir -p "${workdir}"
  cp -R "${example_src}." "${workdir}"

  cd "${workdir}"
  # Drop the local replace and pin the require to the released version. Any
  # require line referencing go.openpit.dev/openpit is rewritten regardless of
  # the version the source tree pinned for development.
  rm -f go.sum
  awk -v ver="${OPENPIT_VERSION}" '
    /^replace go\.openpit\.dev\/openpit/ { next }
    /^require go\.openpit\.dev\/openpit/ {
      print "require go.openpit.dev/openpit v" ver
      next
    }
    { print }
  ' go.mod > go.mod.tmp && mv go.mod.tmp go.mod

  go mod tidy
  go test ./...
done
