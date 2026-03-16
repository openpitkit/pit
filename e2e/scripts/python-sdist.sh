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

python -m venv /tmp/openpit-venv
source /tmp/openpit-venv/bin/activate

python -m pip install --upgrade pip pytest

rm -rf /tmp/openpit-dist
mkdir -p /tmp/openpit-dist

python -m pip download \
  --no-binary=openpit \
  --no-deps \
  "openpit==${OPENPIT_VERSION}" \
  --dest /tmp/openpit-dist

python -m pip install --force-reinstall --no-deps /tmp/openpit-dist/*.tar.gz

python -c "import importlib.metadata as m; version = m.version('openpit'); print(version); assert version == '${OPENPIT_VERSION}'"

cd /opt/e2e/tests
pytest integration/test_engine_integration.py

python /opt/e2e/readme_quickstart.py
python /opt/e2e/wiki_examples.py
