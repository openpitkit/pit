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

project(spot_loadtest
  DESCRIPTION "OpenPit C++ spot-limit open-loop load test"
  LANGUAGES CXX)

if(MSVC AND NOT DEFINED CMAKE_MSVC_RUNTIME_LIBRARY)
  set(CMAKE_MSVC_RUNTIME_LIBRARY "MultiThreaded$<$<CONFIG:Debug>:Debug>")
endif()

set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_CXX_EXTENSIONS OFF)

if(NOT CMAKE_BUILD_TYPE AND NOT CMAKE_CONFIGURATION_TYPES)
  set(CMAKE_BUILD_TYPE Release CACHE STRING "Build type" FORCE)
endif()

option(SPOT_LOADTEST_BUILD_TESTS "Build the spot_loadtest gtest suite" ON)
option(OPENPIT_USE_FIND_PACKAGE
  "Consume an installed OpenPit package via find_package(OpenPit)" OFF)

#-------------------------------------------------------------------------------
# OpenPit C++ binding.
#
# The example consumes the header-only OpenPit C++ binding (the OpenPit::openpit
# INTERFACE target, which transitively links the prebuilt native runtime). It is
# embedded via add_subdirectory from the in-tree binding directory; point
# OPENPIT_CPP_DIR elsewhere to build against an installed/relocated binding.
#
# Provide the native runtime exactly as the binding documents, e.g.:
#   -DOPENPIT_RUNTIME_LIBRARY=$(pwd)/target/release/libopenpit_ffi.dylib

set(OPENPIT_CPP_DIR "${CMAKE_CURRENT_SOURCE_DIR}/../../../bindings/cpp"
  CACHE PATH "Path to the OpenPit C++ binding source directory")

if(OPENPIT_USE_FIND_PACKAGE)
  find_package(OpenPit CONFIG REQUIRED)
elseif(NOT TARGET OpenPit::openpit)
  # The binding builds its own tests by default when top-level; disable them
  # here since we only consume the library target.
  set(OPENPIT_CPP_BUILD_TESTS OFF CACHE BOOL "" FORCE)
  add_subdirectory("${OPENPIT_CPP_DIR}" "${CMAKE_CURRENT_BINARY_DIR}/_openpit_cpp")
endif()

find_package(Threads REQUIRED)

#-------------------------------------------------------------------------------
# Core library: every internal layer (config, measurement, generator, env,
# progress, reporter, driver), mirroring the Go internal/* decomposition.

add_library(spot_loadtest_core STATIC
  src/decimal.cpp
  src/sha256.cpp
  src/config.cpp
  src/histogram.cpp
  src/measurement.cpp
  src/rng.cpp
  src/event.cpp
  src/ledger.cpp
  src/cohort.cpp
  src/lifecycle.cpp
  src/generator.cpp
  src/env.cpp
  src/progress.cpp
  src/reporter.cpp
  src/driver.cpp
  src/driver_oracle.cpp)

target_include_directories(spot_loadtest_core PUBLIC
  "${CMAKE_CURRENT_SOURCE_DIR}/include")
# The driver build/oracle internal headers live in src/.
target_include_directories(spot_loadtest_core PRIVATE
  "${CMAKE_CURRENT_SOURCE_DIR}/src")

target_link_libraries(spot_loadtest_core
  PUBLIC OpenPit::openpit
  PRIVATE Threads::Threads)
if(TARGET OpenPit::project_warnings)
  target_link_libraries(spot_loadtest_core PRIVATE OpenPit::project_warnings)
endif()

#-------------------------------------------------------------------------------
# Executable.

add_executable(spot_loadtest src/main.cpp)
target_link_libraries(spot_loadtest PRIVATE spot_loadtest_core)
if(COMMAND openpit_copy_runtime_dll)
  openpit_copy_runtime_dll(spot_loadtest)
endif()

#-------------------------------------------------------------------------------
# Tests (mirror of the Go *_test.go files: measurement, progress, config,
# driver, generator, reporter), registered in this example's own CMake.

if(SPOT_LOADTEST_BUILD_TESTS)
  include(FetchContent)
  FetchContent_Declare(
    googletest
    GIT_REPOSITORY https://github.com/google/googletest.git
    GIT_TAG v1.15.2)
  set(gtest_force_shared_crt OFF CACHE BOOL "" FORCE)
  FetchContent_MakeAvailable(googletest)

  include(GoogleTest)
  enable_testing()

  function(spot_loadtest_add_test name source)
    add_executable(${name} "${source}")
    target_link_libraries(${name}
      PRIVATE spot_loadtest_core GTest::gmock_main
              Threads::Threads)
    if(TARGET OpenPit::project_warnings)
      target_link_libraries(${name} PRIVATE OpenPit::project_warnings)
    endif()
    target_include_directories(${name} PRIVATE
      "${CMAKE_CURRENT_SOURCE_DIR}/src")
    if(COMMAND openpit_copy_runtime_dll)
      openpit_copy_runtime_dll(${name})
    endif()
    # The committed baseline config path, so the doc-backing / config tests can
    # load the shipped reference file directly.
    target_compile_definitions(${name} PRIVATE
      "SPOT_LOADTEST_BASELINE_INI=\"${CMAKE_CURRENT_SOURCE_DIR}/configs/baseline.ini\"")
    gtest_discover_tests(${name}
      DISCOVERY_MODE PRE_TEST
      DISCOVERY_TIMEOUT 60)
  endfunction()

  spot_loadtest_add_test(spot_loadtest_measurement_tests test/measurement_test.cpp)
  spot_loadtest_add_test(spot_loadtest_progress_tests test/progress_test.cpp)
  spot_loadtest_add_test(spot_loadtest_config_tests test/config_test.cpp)
  spot_loadtest_add_test(spot_loadtest_generator_tests test/generator_test.cpp)
  spot_loadtest_add_test(spot_loadtest_driver_tests test/driver_test.cpp)
  spot_loadtest_add_test(spot_loadtest_reporter_tests test/reporter_test.cpp)
endif()
