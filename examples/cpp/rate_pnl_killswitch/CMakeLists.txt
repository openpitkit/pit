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

project(openpit_example_rate_pnl_killswitch
  DESCRIPTION "OpenPit C++ killswitch supervisor example"
  LANGUAGES CXX)

if(MSVC AND NOT DEFINED CMAKE_MSVC_RUNTIME_LIBRARY)
  set(CMAKE_MSVC_RUNTIME_LIBRARY "MultiThreaded$<$<CONFIG:Debug>:Debug>")
endif()

# -----------------------------------------------------------------------------
# Consume the OpenPit C++ binding.
#
# In-repo (the default when this example lives inside the pit checkout): pick up
# the binding next to it with add_subdirectory, so the example builds against
# local sources with no install step. Pass -DOPENPIT_RUNTIME_LIBRARY=<path> to
# link the locally built libopenpit_ffi, or let the binding download the
# matching prebuilt runtime.
#
# Standalone (the binding is installed): configure with
# -DOPENPIT_EXAMPLE_USE_INSTALLED_PACKAGE=ON and point CMAKE_PREFIX_PATH at the
# install tree; the example then resolves it with find_package(OpenPit). See the
# README for the external recipe.

option(OPENPIT_EXAMPLE_USE_INSTALLED_PACKAGE
  "Resolve the binding via find_package(OpenPit) instead of add_subdirectory"
  OFF)

# Default to the binding's in-repo location: examples/cpp/<name> -> bindings/cpp.
set(OPENPIT_CPP_BINDING_DIR "${CMAKE_CURRENT_SOURCE_DIR}/../../../bindings/cpp"
  CACHE PATH "Path to the OpenPit C++ binding source tree")

if(OPENPIT_EXAMPLE_USE_INSTALLED_PACKAGE)
  find_package(OpenPit CONFIG REQUIRED)
elseif(TARGET OpenPit::openpit)
  # Already provided by an enclosing build (e.g. the binding added us itself).
elseif(EXISTS "${OPENPIT_CPP_BINDING_DIR}/CMakeLists.txt")
  add_subdirectory("${OPENPIT_CPP_BINDING_DIR}" openpit-cpp-binding)
else()
  message(FATAL_ERROR
    "OpenPit: binding not found at '${OPENPIT_CPP_BINDING_DIR}'. Set "
    "OPENPIT_CPP_BINDING_DIR, or build standalone with "
    "-DOPENPIT_EXAMPLE_USE_INSTALLED_PACKAGE=ON and CMAKE_PREFIX_PATH.")
endif()

# -----------------------------------------------------------------------------
# The example executable.

add_executable(rate_pnl_killswitch main.cpp)
target_compile_features(rate_pnl_killswitch PRIVATE cxx_std_17)
target_include_directories(rate_pnl_killswitch
  PRIVATE "${CMAKE_CURRENT_SOURCE_DIR}")
target_link_libraries(rate_pnl_killswitch PRIVATE OpenPit::openpit)
if(TARGET OpenPit::project_warnings)
  target_link_libraries(rate_pnl_killswitch PRIVATE OpenPit::project_warnings)
endif()
if(COMMAND openpit_copy_runtime_dll)
  openpit_copy_runtime_dll(rate_pnl_killswitch)
endif()

# Embed the runtime library location into the binary's RPATH so the loader finds
# libopenpit_ffi without a manually exported library path. The binding records
# the resolved file in OPENPIT_RUNTIME_LIBRARY_FILE.
if(DEFINED OPENPIT_RUNTIME_LIBRARY_FILE AND OPENPIT_RUNTIME_LIBRARY_FILE)
  get_filename_component(_openpit_runtime_dir
    "${OPENPIT_RUNTIME_LIBRARY_FILE}" DIRECTORY)
  set_target_properties(rate_pnl_killswitch PROPERTIES
    BUILD_RPATH "${_openpit_runtime_dir}")
endif()

# -----------------------------------------------------------------------------
# The example's own smoke test (gtest), mirroring main_test.go. Registered here,
# in the example's CMake - not in the binding's test tree.

option(OPENPIT_EXAMPLE_BUILD_TESTS "Build the rate_pnl_killswitch smoke test"
  "${PROJECT_IS_TOP_LEVEL}")

if(OPENPIT_EXAMPLE_BUILD_TESTS)
  enable_testing()

  # Reuse a GoogleTest already provided by an enclosing build; otherwise fetch
  # the same pinned release the binding tests use.
  if(NOT TARGET GTest::gmock_main)
    include(FetchContent)
    FetchContent_Declare(
      googletest
      GIT_REPOSITORY https://github.com/google/googletest.git
      GIT_TAG v1.15.2)
    set(gtest_force_shared_crt OFF CACHE BOOL "" FORCE)
    FetchContent_MakeAvailable(googletest)
  endif()

  include(GoogleTest)

  add_executable(rate_pnl_killswitch_tests test/killswitch_test.cpp)
  target_compile_features(rate_pnl_killswitch_tests PRIVATE cxx_std_17)
  target_include_directories(rate_pnl_killswitch_tests
    PRIVATE "${CMAKE_CURRENT_SOURCE_DIR}")
  target_link_libraries(rate_pnl_killswitch_tests
    PRIVATE OpenPit::openpit GTest::gmock_main)
  if(TARGET OpenPit::project_warnings)
    target_link_libraries(rate_pnl_killswitch_tests
      PRIVATE OpenPit::project_warnings)
  endif()
  if(COMMAND openpit_copy_runtime_dll)
    openpit_copy_runtime_dll(rate_pnl_killswitch_tests)
  endif()

  if(DEFINED OPENPIT_RUNTIME_LIBRARY_FILE AND OPENPIT_RUNTIME_LIBRARY_FILE)
    set_target_properties(rate_pnl_killswitch_tests PROPERTIES
      BUILD_RPATH "${_openpit_runtime_dir}")
  endif()

  gtest_discover_tests(rate_pnl_killswitch_tests
    DISCOVERY_MODE PRE_TEST
    DISCOVERY_TIMEOUT 60)
endif()
