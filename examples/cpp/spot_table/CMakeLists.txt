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

# Standalone CMake project for the spot_table C++ example.
#
# It depends on the OpenPit C++ binding target `OpenPit::openpit`. Three ways to
# provide it, tried in order:
#
#   1. In-repo source (default): the binding lives at ../../../bindings/cpp
#      relative to this directory; it is pulled in via add_subdirectory.
#   2. FetchContent: set OPENPIT_FETCH=ON to fetch the binding by Git tag instead
#      (handy when this example is copied out of the repo tree).
#   3. find_package: set OPENPIT_USE_FIND_PACKAGE=ON to consume an *installed*
#      OpenPit (the form the README documents for external projects).
#
# The bundled runtime resolver (OpenPitDownloadRuntime) still resolves the
# prebuilt native library at configure time; point it at a local build with
#   -DOPENPIT_RUNTIME_LIBRARY=/abs/path/to/libopenpit_ffi.dylib

cmake_minimum_required(VERSION 3.21)

if(POLICY CMP0091)
  cmake_policy(SET CMP0091 NEW)
endif()

project(openpit_example_spot_table
  DESCRIPTION "Table-driven spot-funds policy runner"
  LANGUAGES CXX)

if(MSVC AND NOT DEFINED CMAKE_MSVC_RUNTIME_LIBRARY)
  set(CMAKE_MSVC_RUNTIME_LIBRARY "MultiThreaded$<$<CONFIG:Debug>:Debug>")
endif()

set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_CXX_EXTENSIONS OFF)

option(OPENPIT_FETCH "Fetch the OpenPit C++ binding via FetchContent" OFF)
option(OPENPIT_USE_FIND_PACKAGE
  "Consume an installed OpenPit via find_package(OpenPit)" OFF)
option(OPENPIT_EXAMPLE_BUILD_TESTS
  "Build the spot_table gtest smoke test" "${PROJECT_IS_TOP_LEVEL}")

# The in-repo binding source directory (relative to this example).
get_filename_component(OPENPIT_CPP_BINDING_DIR
  "${CMAKE_CURRENT_SOURCE_DIR}/../../../bindings/cpp" ABSOLUTE)

if(OPENPIT_USE_FIND_PACKAGE)
  # External-project form, as documented in the README:
  #   find_package(OpenPit CONFIG REQUIRED)
  find_package(OpenPit CONFIG REQUIRED)
elseif(TARGET OpenPit::openpit)
  # Already provided by an enclosing build (e.g. the examples aggregator added
  # the binding once and shares it across every example).
elseif(OPENPIT_FETCH)
  include(FetchContent)
  FetchContent_Declare(OpenPit
    GIT_REPOSITORY https://github.com/openpitkit/pit.git
    GIT_TAG main
    SOURCE_SUBDIR bindings/cpp)
  FetchContent_MakeAvailable(OpenPit)
elseif(EXISTS "${OPENPIT_CPP_BINDING_DIR}/CMakeLists.txt")
  # Default: build the binding from this repository's tree.
  add_subdirectory("${OPENPIT_CPP_BINDING_DIR}" openpit_cpp_binding)
else()
  message(FATAL_ERROR
    "OpenPit C++ binding not found at ${OPENPIT_CPP_BINDING_DIR}; "
    "set -DOPENPIT_FETCH=ON or -DOPENPIT_USE_FIND_PACKAGE=ON")
endif()

#-------------------------------------------------------------------------------
# Library of the example's modules, reused by both the CLI and the test.

add_library(spot_table_lib STATIC
  table.cpp
  platform.cpp
  marketdata.cpp
  builder.cpp
  runner.cpp
  duration.cpp)

target_include_directories(spot_table_lib PUBLIC "${CMAKE_CURRENT_SOURCE_DIR}")
target_link_libraries(spot_table_lib PUBLIC OpenPit::openpit)
if(TARGET OpenPit::project_warnings)
  target_link_libraries(spot_table_lib PRIVATE OpenPit::project_warnings)
endif()

#-------------------------------------------------------------------------------
# CLI executable.

add_executable(spot_table main.cpp)
target_link_libraries(spot_table PRIVATE spot_table_lib)
if(COMMAND openpit_copy_runtime_dll)
  openpit_copy_runtime_dll(spot_table)
endif()

#-------------------------------------------------------------------------------
# gtest smoke test.

if(OPENPIT_EXAMPLE_BUILD_TESTS)
  enable_testing()

  include(FetchContent)
  FetchContent_Declare(googletest
    GIT_REPOSITORY https://github.com/google/googletest.git
    GIT_TAG v1.15.2)
  set(gtest_force_shared_crt OFF CACHE BOOL "" FORCE)
  FetchContent_MakeAvailable(googletest)
  include(GoogleTest)

  # Absolute path to the bundled coverage scenario, injected so the test resolves
  # it regardless of the working directory.
  get_filename_component(SPOT_TABLE_COVERAGE_PATH
    "${CMAKE_CURRENT_SOURCE_DIR}/../../tables/spot/coverage.md" ABSOLUTE)

  add_executable(spot_table_tests main_test.cpp)
  target_link_libraries(spot_table_tests
    PRIVATE spot_table_lib GTest::gmock_main)
  if(TARGET OpenPit::project_warnings)
    target_link_libraries(spot_table_tests PRIVATE OpenPit::project_warnings)
  endif()
  if(COMMAND openpit_copy_runtime_dll)
    openpit_copy_runtime_dll(spot_table_tests)
  endif()
  target_compile_definitions(spot_table_tests
    PRIVATE "SPOT_TABLE_COVERAGE_PATH=\"${SPOT_TABLE_COVERAGE_PATH}\"")

  gtest_discover_tests(spot_table_tests
    DISCOVERY_MODE PRE_TEST
    DISCOVERY_TIMEOUT 60)
endif()
