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

include(FetchContent)

if(MSVC AND NOT DEFINED CMAKE_MSVC_RUNTIME_LIBRARY)
  set(CMAKE_MSVC_RUNTIME_LIBRARY "MultiThreaded$<$<CONFIG:Debug>:Debug>")
endif()

# Pinned GoogleTest release (gtest + gmock).
FetchContent_Declare(
  googletest
  GIT_REPOSITORY https://github.com/google/googletest.git
  GIT_TAG v1.15.2)

# Follow CMAKE_MSVC_RUNTIME_LIBRARY instead of forcing /MD.
set(gtest_force_shared_crt OFF CACHE BOOL "" FORCE)
set(INSTALL_GTEST OFF CACHE BOOL "" FORCE)
FetchContent_MakeAvailable(googletest)

include(GoogleTest)

function(openpit_add_test_target target)
  target_link_libraries(${target} PRIVATE OpenPit::project_warnings)
  openpit_copy_runtime_dll(${target})
  gtest_discover_tests(${target}
    DISCOVERY_MODE PRE_TEST
    DISCOVERY_TIMEOUT 60)
endfunction()

add_executable(openpit_smoke_tests smoke_test.cpp)
target_link_libraries(openpit_smoke_tests
  PRIVATE OpenPit::openpit GTest::gmock_main)

openpit_add_test_target(openpit_smoke_tests)

add_executable(openpit_model_tests model_test.cpp)
target_link_libraries(openpit_model_tests
  PRIVATE OpenPit::openpit GTest::gmock_main)

openpit_add_test_target(openpit_model_tests)

add_executable(openpit_pretrade_tests pretrade_test.cpp)
target_link_libraries(openpit_pretrade_tests
  PRIVATE OpenPit::openpit GTest::gmock_main)

openpit_add_test_target(openpit_pretrade_tests)

add_executable(openpit_marketdata_tests marketdata_test.cpp)
target_link_libraries(openpit_marketdata_tests
  PRIVATE OpenPit::openpit GTest::gmock_main)

openpit_add_test_target(openpit_marketdata_tests)

add_executable(openpit_engine_tests engine_test.cpp)
target_link_libraries(openpit_engine_tests
  PRIVATE OpenPit::openpit GTest::gmock_main)

openpit_add_test_target(openpit_engine_tests)

add_executable(openpit_accounts_tests accounts_test.cpp)
target_link_libraries(openpit_accounts_tests
  PRIVATE OpenPit::openpit GTest::gmock_main)

openpit_add_test_target(openpit_accounts_tests)

add_executable(openpit_account_adjustment_tests account_adjustment_test.cpp)
target_link_libraries(openpit_account_adjustment_tests
  PRIVATE OpenPit::openpit GTest::gmock_main)

openpit_add_test_target(openpit_account_adjustment_tests)

add_executable(openpit_async_engine_tests async_engine_test.cpp)
target_link_libraries(openpit_async_engine_tests
  PRIVATE OpenPit::openpit GTest::gmock_main)

openpit_add_test_target(openpit_async_engine_tests)

add_executable(openpit_typed_async_engine_tests typed_async_engine_test.cpp)
target_link_libraries(openpit_typed_async_engine_tests
  PRIVATE OpenPit::openpit GTest::gmock_main)

openpit_add_test_target(openpit_typed_async_engine_tests)

add_executable(openpit_readme_example_test examples_readme_test.cpp)
target_link_libraries(openpit_readme_example_test
  PRIVATE OpenPit::openpit OpenPit::project_warnings)
openpit_copy_runtime_dll(openpit_readme_example_test)

add_test(NAME openpit_readme_example
  COMMAND openpit_readme_example_test)

# Compiling mirrors of the C++ snippets published on the wiki.
add_subdirectory(wiki)
