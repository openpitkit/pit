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

# Single-source the package version from the workspace Cargo manifest unless the
# caller pins it explicitly. Runtime release tags may include a prerelease suffix
# that CMake's project(VERSION) grammar does not accept.
set(OPENPIT_CARGO_MANIFEST "${CMAKE_CURRENT_SOURCE_DIR}/../../crates/openpit/Cargo.toml")
if(DEFINED OPENPIT_PACKAGE_VERSION AND NOT OPENPIT_PACKAGE_VERSION STREQUAL "")
  set(OPENPIT_VERSION "${OPENPIT_PACKAGE_VERSION}")
elseif(EXISTS "${OPENPIT_CARGO_MANIFEST}")
  file(READ "${OPENPIT_CARGO_MANIFEST}" _openpit_cargo_toml)
  string(REGEX MATCH "(^|\n)version[ \t]*=[ \t]*\"([0-9][^\"]*)\""
    _openpit_version_match "${_openpit_cargo_toml}")
  set(OPENPIT_VERSION "${CMAKE_MATCH_2}")
  unset(_openpit_cargo_toml)
endif()
if(NOT OPENPIT_VERSION)
  message(FATAL_ERROR
    "OpenPit: could not determine version; set OPENPIT_PACKAGE_VERSION")
endif()

project(OpenPit
  VERSION "${OPENPIT_VERSION}"
  DESCRIPTION "Modern C++17 binding for the OpenPit pre-trade risk SDK"
  LANGUAGES CXX)

list(APPEND CMAKE_MODULE_PATH "${CMAKE_CURRENT_SOURCE_DIR}/cmake")

# GitHub repository hosting the prebuilt runtime release assets.
set(OPENPIT_RUNTIME_REPO "openpitkit/pit"
  CACHE STRING "GitHub owner/repo hosting OpenPit runtime releases")
# Make the release tag visible to the runtime resolver and the installed config.
if(NOT DEFINED OPENPIT_RUNTIME_VERSION OR OPENPIT_RUNTIME_VERSION STREQUAL "")
  set(OPENPIT_RUNTIME_VERSION "${OPENPIT_VERSION}"
    CACHE STRING "OpenPit prebuilt runtime version")
endif()

# Detect standalone vs add_subdirectory/FetchContent embedding. Both lead to the
# same OpenPit::openpit alias.
if(NOT DEFINED OPENPIT_CPP_IS_TOP_LEVEL)
  set(OPENPIT_CPP_IS_TOP_LEVEL "${PROJECT_IS_TOP_LEVEL}")
endif()

option(OPENPIT_CPP_BUILD_TESTS "Build OpenPit C++ binding tests"
  "${OPENPIT_CPP_IS_TOP_LEVEL}")

#-------------------------------------------------------------------------------
# Resolve the prebuilt runtime (download or local override) into OpenPit::runtime.

include(OpenPitDownloadRuntime)
openpit_resolve_runtime()

#-------------------------------------------------------------------------------
# Library target.

# Header-only binding: an INTERFACE library carries the include dirs, the C++17
# requirement, and the runtime link so consumers get them transitively.
add_library(openpit INTERFACE)
add_library(OpenPit::openpit ALIAS openpit)

target_compile_features(openpit INTERFACE cxx_std_17)

target_include_directories(openpit INTERFACE
  "$<BUILD_INTERFACE:${CMAKE_CURRENT_SOURCE_DIR}/include>"
  # Generated C ABI header (bindings/c/openpit.h), included as <openpit.h>.
  "$<BUILD_INTERFACE:${CMAKE_CURRENT_SOURCE_DIR}/../c>"
  "$<INSTALL_INTERFACE:include>")

target_link_libraries(openpit INTERFACE OpenPit::runtime)

# Internal validation target. It is intentionally not linked from
# OpenPit::openpit so downstream consumers do not inherit project warning
# policy.
add_library(openpit_project_warnings INTERFACE)
add_library(OpenPit::project_warnings ALIAS openpit_project_warnings)
target_compile_options(openpit_project_warnings INTERFACE
  $<$<CXX_COMPILER_ID:GNU,Clang,AppleClang>:-Wall -Wextra -Wpedantic -Werror>
  $<$<CXX_COMPILER_ID:MSVC>:/W4 /WX /permissive->)

#-------------------------------------------------------------------------------
# Tests.

if(OPENPIT_CPP_BUILD_TESTS)
  enable_testing()
  add_subdirectory(test)
endif()

#-------------------------------------------------------------------------------
# Install + config-file package.

include(GNUInstallDirs)
include(CMakePackageConfigHelpers)

set(OPENPIT_INSTALL_CMAKEDIR "${CMAKE_INSTALL_LIBDIR}/cmake/OpenPit")

install(TARGETS openpit
  EXPORT OpenPitTargets
  INCLUDES DESTINATION "${CMAKE_INSTALL_INCLUDEDIR}")

# Public C++ headers and the generated C ABI header.
install(DIRECTORY "${CMAKE_CURRENT_SOURCE_DIR}/include/openpit"
  DESTINATION "${CMAKE_INSTALL_INCLUDEDIR}")
install(FILES "${CMAKE_CURRENT_SOURCE_DIR}/../c/openpit.h"
  DESTINATION "${CMAKE_INSTALL_INCLUDEDIR}")

install(EXPORT OpenPitTargets
  FILE OpenPitTargets.cmake
  NAMESPACE OpenPit::
  DESTINATION "${OPENPIT_INSTALL_CMAKEDIR}")

# Ship the runtime resolver so installed consumers can fetch/override the lib.
install(FILES "${CMAKE_CURRENT_SOURCE_DIR}/cmake/OpenPitDownloadRuntime.cmake"
  DESTINATION "${OPENPIT_INSTALL_CMAKEDIR}")

configure_package_config_file(
  "${CMAKE_CURRENT_SOURCE_DIR}/cmake/OpenPitConfig.cmake.in"
  "${CMAKE_CURRENT_BINARY_DIR}/OpenPitConfig.cmake"
  INSTALL_DESTINATION "${OPENPIT_INSTALL_CMAKEDIR}")

write_basic_package_version_file(
  "${CMAKE_CURRENT_BINARY_DIR}/OpenPitConfigVersion.cmake"
  VERSION "${OPENPIT_VERSION}"
  COMPATIBILITY SameMajorVersion)

install(FILES
  "${CMAKE_CURRENT_BINARY_DIR}/OpenPitConfig.cmake"
  "${CMAKE_CURRENT_BINARY_DIR}/OpenPitConfigVersion.cmake"
  DESTINATION "${OPENPIT_INSTALL_CMAKEDIR}")
