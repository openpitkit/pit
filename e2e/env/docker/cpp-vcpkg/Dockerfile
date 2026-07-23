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

FROM debian:bookworm

RUN apt-get update \
  && apt-get install --yes --no-install-recommends \
    ca-certificates \
    cmake \
    curl \
    g++ \
    git \
    make \
    python3 \
    tar \
    unzip \
    zip \
  && rm -rf /var/lib/apt/lists/*

WORKDIR /opt/e2e

COPY e2e/scripts/cpp-vcpkg.sh /usr/local/bin/run-release-e2e
COPY e2e/clients/cpp /opt/e2e/cpp-consumer
COPY examples/cpp /opt/e2e/examples
COPY examples/tables /opt/e2e/tables

RUN chmod +x /usr/local/bin/run-release-e2e

ENTRYPOINT ["/usr/local/bin/run-release-e2e"]
