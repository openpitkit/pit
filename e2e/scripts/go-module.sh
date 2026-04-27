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
mkdir -p /tmp/openpit-go-consumer/tests
cp -R /opt/e2e/go-integration-tests /tmp/openpit-go-consumer/tests/integration

cd /tmp/openpit-go-consumer
sed "s/__OPENPIT_VERSION__/${OPENPIT_VERSION}/g" go.mod.in > go.mod
rm go.mod.in
find ./tests/integration -type f -name "*.go" -exec \
  sed -i "s|github.com/openpitkit/pit/bindings/go|github.com/openpitkit/pit-go|g" {} +

go mod tidy
go test ./...
