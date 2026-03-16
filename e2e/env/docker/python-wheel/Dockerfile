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

FROM python:3.11-slim-bookworm

WORKDIR /opt/e2e

COPY bindings/python/pyproject.toml /opt/e2e/pyproject.toml
COPY bindings/python/tests/conftest.py /opt/e2e/tests/conftest.py
COPY bindings/python/tests/integration /opt/e2e/tests/integration
COPY e2e/scripts/python-wheel.sh /usr/local/bin/run-release-e2e
COPY e2e/clients/python/readme_quickstart.py /opt/e2e/readme_quickstart.py
COPY e2e/clients/python/wiki_examples.py /opt/e2e/wiki_examples.py

RUN python -m pip install --upgrade pip pytest \
    && chmod +x /usr/local/bin/run-release-e2e

ENTRYPOINT ["/usr/local/bin/run-release-e2e"]
