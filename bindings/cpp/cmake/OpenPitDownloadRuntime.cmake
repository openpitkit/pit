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

# Resolves the prebuilt `openpit-ffi` runtime library and exposes it as the
# IMPORTED target `OpenPit::runtime`. Consumers need no Rust toolchain.
#
# Resolution order:
#   1. OPENPIT_RUNTIME_LIBRARY  - absolute path to a prebuilt library file
#                                 (e.g. target/<triple>/release/libopenpit_ffi.so).
#   2. OPENPIT_RUNTIME_DIR       - directory holding the host library file under
#                                 its platform-default name.
#   3. Download the matching release asset from GitHub and verify its .sha256
#      sidecar.
#
# Inputs:
#   OPENPIT_RUNTIME_VERSION  - release version (defaults to the workspace
#                              version passed by the caller).
#   OPENPIT_RUNTIME_REPO     - "owner/repo" hosting the GitHub releases.
#   OPENPIT_RUNTIME_IMPORT_LIBRARY
#                            - Windows-only absolute path to the MSVC import
#                              library paired with OPENPIT_RUNTIME_LIBRARY.

include_guard(GLOBAL)

# Maps the host system to (goos, goarch, lib_name) matching the release asset
# naming scheme `openpit-ffi--<goos>-<goarch>-<lib_name>`.
function(_openpit_runtime_host_triple out_goos out_goarch out_lib_name)
  if(CMAKE_SYSTEM_NAME STREQUAL "Darwin")
    set(goos "darwin")
    set(lib_name "libopenpit_ffi.dylib")
  elseif(CMAKE_SYSTEM_NAME STREQUAL "Linux")
    set(goos "linux")
    set(lib_name "libopenpit_ffi.so")
  elseif(CMAKE_SYSTEM_NAME STREQUAL "Windows")
    set(goos "windows")
    set(lib_name "openpit_ffi.dll")
  else()
    message(FATAL_ERROR "OpenPit: unsupported host OS '${CMAKE_SYSTEM_NAME}'")
  endif()

  set(arch "${CMAKE_SYSTEM_PROCESSOR}")
  if(arch MATCHES "^(aarch64|arm64|ARM64)$")
    set(goarch "arm64")
  elseif(arch MATCHES "^(x86_64|amd64|AMD64)$")
    set(goarch "amd64")
  else()
    message(FATAL_ERROR "OpenPit: unsupported host arch '${arch}'")
  endif()

  set(${out_goos} "${goos}" PARENT_SCOPE)
  set(${out_goarch} "${goarch}" PARENT_SCOPE)
  set(${out_lib_name} "${lib_name}" PARENT_SCOPE)
endfunction()

# Resolves the Windows import library paired with a DLL runtime path.
function(_openpit_runtime_resolve_windows_implib lib_path out_implib_path)
  if(DEFINED OPENPIT_RUNTIME_IMPORT_LIBRARY
      AND NOT OPENPIT_RUNTIME_IMPORT_LIBRARY STREQUAL "")
    set(candidate "${OPENPIT_RUNTIME_IMPORT_LIBRARY}")
  else()
    set(candidate "${lib_path}.lib")
  endif()

  if(NOT EXISTS "${candidate}")
    message(FATAL_ERROR
      "OpenPit: Windows runtime import library not found at '${candidate}'. "
      "Set OPENPIT_RUNTIME_IMPORT_LIBRARY to the .lib paired with '${lib_path}'.")
  endif()

  set(${out_implib_path} "${candidate}" PARENT_SCOPE)
endfunction()

# Creates the IMPORTED target for a resolved library file. Windows consumers link
# through the MSVC import library and load the DLL at runtime.
function(_openpit_runtime_define_target lib_path)
  if(TARGET OpenPit::runtime)
    return()
  endif()
  if(NOT EXISTS "${lib_path}")
    message(FATAL_ERROR "OpenPit: runtime library not found at '${lib_path}'")
  endif()
  add_library(OpenPit::runtime SHARED IMPORTED GLOBAL)
  # Keep the absolute path in link commands. `IMPORTED_NO_SONAME` makes CMake
  # rewrite a path without a soname as `-l<filename>`, which cannot resolve
  # release assets whose names do not follow the platform library convention.
  set_target_properties(OpenPit::runtime PROPERTIES
    IMPORTED_LOCATION "${lib_path}")
  if(WIN32)
    _openpit_runtime_resolve_windows_implib("${lib_path}" implib_path)
    set_target_properties(OpenPit::runtime PROPERTIES
      IMPORTED_IMPLIB "${implib_path}")
  endif()
  set(OPENPIT_RUNTIME_LIBRARY_FILE "${lib_path}"
    CACHE INTERNAL "Resolved openpit-ffi runtime library file")
endfunction()

function(openpit_copy_runtime_dll target)
  if(WIN32 AND TARGET OpenPit::runtime)
    add_custom_command(TARGET "${target}" POST_BUILD
      COMMAND "${CMAKE_COMMAND}" -E copy_if_different
        "$<TARGET_FILE:OpenPit::runtime>" "$<TARGET_FILE_DIR:${target}>")
  endif()
endfunction()

function(_openpit_runtime_download_asset base_url asset out_path)
  set(sha_path "${out_path}.sha256")

  message(STATUS "OpenPit: downloading runtime '${asset}' from ${base_url}")
  file(DOWNLOAD "${base_url}/${asset}.sha256" "${sha_path}"
    STATUS sha_status TLS_VERIFY ON)
  list(GET sha_status 0 sha_code)
  if(NOT sha_code EQUAL 0)
    list(GET sha_status 1 sha_msg)
    message(FATAL_ERROR
      "OpenPit: failed to download sha256 sidecar for '${asset}': ${sha_msg}")
  endif()

  file(READ "${sha_path}" expected_sha)
  string(REGEX MATCH "[0-9a-fA-F]+" expected_sha "${expected_sha}")
  string(TOLOWER "${expected_sha}" expected_sha)

  file(DOWNLOAD "${base_url}/${asset}" "${out_path}"
    STATUS lib_status
    EXPECTED_HASH "SHA256=${expected_sha}"
    TLS_VERIFY ON)
  list(GET lib_status 0 lib_code)
  if(NOT lib_code EQUAL 0)
    list(GET lib_status 1 lib_msg)
    file(REMOVE "${out_path}")
    message(FATAL_ERROR
      "OpenPit: failed to download runtime '${asset}': ${lib_msg}")
  endif()
endfunction()

function(openpit_resolve_runtime)
  if(TARGET OpenPit::runtime)
    return()
  endif()

  _openpit_runtime_host_triple(goos goarch lib_name)

  # 1. Explicit library file override.
  if(DEFINED OPENPIT_RUNTIME_LIBRARY AND NOT OPENPIT_RUNTIME_LIBRARY STREQUAL "")
    message(STATUS
      "OpenPit: using runtime library override '${OPENPIT_RUNTIME_LIBRARY}'")
    _openpit_runtime_define_target("${OPENPIT_RUNTIME_LIBRARY}")
    return()
  endif()

  # 2. Directory override holding the host library under its default name.
  if(DEFINED OPENPIT_RUNTIME_DIR AND NOT OPENPIT_RUNTIME_DIR STREQUAL "")
    set(candidate "${OPENPIT_RUNTIME_DIR}/${lib_name}")
    message(STATUS "OpenPit: using runtime dir override '${candidate}'")
    _openpit_runtime_define_target("${candidate}")
    return()
  endif()

  # 3. Download the matching release asset and verify the sha256 sidecar.
  if(NOT DEFINED OPENPIT_RUNTIME_VERSION OR OPENPIT_RUNTIME_VERSION STREQUAL "")
    message(FATAL_ERROR
      "OpenPit: OPENPIT_RUNTIME_VERSION is required to download the runtime")
  endif()

  set(asset "openpit-ffi--${goos}-${goarch}-${lib_name}")
  set(base_url
    "https://github.com/${OPENPIT_RUNTIME_REPO}/releases/download/v${OPENPIT_RUNTIME_VERSION}")
  set(download_dir
    "${CMAKE_BINARY_DIR}/openpit-runtime/v${OPENPIT_RUNTIME_VERSION}")
  set(lib_path "${download_dir}/${lib_name}")

  if(NOT EXISTS "${lib_path}")
    file(MAKE_DIRECTORY "${download_dir}")
    _openpit_runtime_download_asset("${base_url}" "${asset}" "${lib_path}")
  endif()

  if(WIN32)
    set(implib_name "${lib_name}.lib")
    set(implib_asset "openpit-ffi--${goos}-${goarch}-${implib_name}")
    set(implib_path "${download_dir}/${implib_name}")
    if(NOT EXISTS "${implib_path}")
      file(MAKE_DIRECTORY "${download_dir}")
      _openpit_runtime_download_asset("${base_url}" "${implib_asset}"
        "${implib_path}")
    endif()
    # `_openpit_runtime_define_target` re-derives the implib as "${lib_path}.lib"
    # (== "${implib_path}"), so no explicit hand-off is needed here.
  endif()

  _openpit_runtime_define_target("${lib_path}")
endfunction()
