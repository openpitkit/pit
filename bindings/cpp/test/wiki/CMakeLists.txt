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

# Compiling mirrors of the C++ snippets published on the wiki. Each
# `*_test.cpp` here is the same user code as a wiki code block (modulo a minimal
# harness) and keeps the published documentation honest. GoogleTest and the
# `gtest_discover_tests` macro are provided by the parent test directory.

file(GLOB openpit_wiki_test_sources CONFIGURE_DEPENDS
  "${CMAKE_CURRENT_SOURCE_DIR}/*_test.cpp")

foreach(openpit_wiki_test_source IN LISTS openpit_wiki_test_sources)
  get_filename_component(openpit_wiki_test_name "${openpit_wiki_test_source}"
    NAME_WE)
  set(openpit_wiki_target "openpit_wiki_${openpit_wiki_test_name}")

  add_executable(${openpit_wiki_target} "${openpit_wiki_test_source}")
  target_link_libraries(${openpit_wiki_target}
    PRIVATE OpenPit::openpit OpenPit::project_warnings
            GTest::gmock_main)

  openpit_copy_runtime_dll(${openpit_wiki_target})
  gtest_discover_tests(${openpit_wiki_target}
    DISCOVERY_MODE PRE_TEST
    DISCOVERY_TIMEOUT 60)
endforeach()
