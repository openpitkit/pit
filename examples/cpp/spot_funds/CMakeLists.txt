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

cmake_minimum_required(VERSION 3.21)

if(POLICY CMP0091)
  cmake_policy(SET CMP0091 NEW)
endif()

project(openpit_example_spot_funds
  DESCRIPTION "OpenPit C++ SpotFunds pre-trade policy example"
  LANGUAGES CXX)

if(MSVC AND NOT DEFINED CMAKE_MSVC_RUNTIME_LIBRARY)
  set(CMAKE_MSVC_RUNTIME_LIBRARY "MultiThreaded$<$<CONFIG:Debug>:Debug>")
endif()

#-------------------------------------------------------------------------------
# Resolve the OpenPit C++ binding (OpenPit::openpit).
#
# In-repo (this example lives inside the pit checkout) the binding is built from
# source by embedding bindings/cpp. A standalone copy of this example outside the
# repo instead finds an installed package - see the README's find_package(OpenPit)
# snippet. The example code is identical either way; only how the target is
# resolved differs.

set(_openpit_cpp_dir "${CMAKE_CURRENT_SOURCE_DIR}/../../../bindings/cpp")

if(NOT TARGET OpenPit::openpit)
  if(EXISTS "${_openpit_cpp_dir}/CMakeLists.txt")
    # In-repo: embed the binding from source. FetchContent is the documented
    # vehicle for consuming the binding by source tree; here it points at the
    # local checkout so no network fetch happens.
    include(FetchContent)
    # The binding's own tests are off by default when embedded; this example
    # ships its own smoke test below.
    set(OPENPIT_CPP_BUILD_TESTS OFF CACHE BOOL "" FORCE)
    FetchContent_Declare(openpit_cpp SOURCE_DIR "${_openpit_cpp_dir}")
    FetchContent_MakeAvailable(openpit_cpp)
  else()
    # Standalone: consume an installed binding + its bundled runtime resolver.
    find_package(OpenPit CONFIG REQUIRED)
  endif()
endif()

#-------------------------------------------------------------------------------
# Example executable.

add_executable(spot_funds src/main.cpp)
target_include_directories(spot_funds PRIVATE "${CMAKE_CURRENT_SOURCE_DIR}/src")
target_compile_features(spot_funds PRIVATE cxx_std_17)
target_link_libraries(spot_funds PRIVATE OpenPit::openpit)
if(TARGET OpenPit::project_warnings)
  target_link_libraries(spot_funds PRIVATE OpenPit::project_warnings)
endif()
if(COMMAND openpit_copy_runtime_dll)
  openpit_copy_runtime_dll(spot_funds)
endif()

#-------------------------------------------------------------------------------
# Smoke test (gtest). Registered here, in the example's own CMake, not in the
# binding's bindings/cpp/test tree. Built only when this example is the top-level
# project, or when explicitly requested, so embedding it elsewhere stays light.

if(NOT DEFINED OPENPIT_EXAMPLE_BUILD_TESTS)
  set(OPENPIT_EXAMPLE_BUILD_TESTS "${PROJECT_IS_TOP_LEVEL}")
endif()

if(OPENPIT_EXAMPLE_BUILD_TESTS)
  enable_testing()

  include(FetchContent)
  # Pinned GoogleTest release, matching the binding's own test pin.
  FetchContent_Declare(
    googletest
    GIT_REPOSITORY https://github.com/google/googletest.git
    GIT_TAG v1.15.2)
  set(gtest_force_shared_crt OFF CACHE BOOL "" FORCE)
  FetchContent_MakeAvailable(googletest)

  include(GoogleTest)

  add_executable(spot_funds_smoke_tests test/smoke_test.cpp)
  target_include_directories(spot_funds_smoke_tests
    PRIVATE "${CMAKE_CURRENT_SOURCE_DIR}/src")
  target_compile_features(spot_funds_smoke_tests PRIVATE cxx_std_17)
  target_link_libraries(spot_funds_smoke_tests
    PRIVATE OpenPit::openpit GTest::gmock_main)
  if(TARGET OpenPit::project_warnings)
    target_link_libraries(spot_funds_smoke_tests
      PRIVATE OpenPit::project_warnings)
  endif()
  if(COMMAND openpit_copy_runtime_dll)
    openpit_copy_runtime_dll(spot_funds_smoke_tests)
  endif()

  gtest_discover_tests(spot_funds_smoke_tests
    DISCOVERY_MODE PRE_TEST
    DISCOVERY_TIMEOUT 60)
endif()
