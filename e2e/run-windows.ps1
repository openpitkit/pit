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

[CmdletBinding()]
param(
  [Parameter(Mandatory, Position = 0)]
  [string]$Version
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

if ([string]::IsNullOrWhiteSpace($Version)) {
  throw 'OpenPit release version is required'
}

$rootDir = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$knownTargets = @(
  'python-wheel-windows-amd64',
  'go-windows-amd64',
  'cpp-windows-amd64',
  'cpp-vcpkg-windows-amd64'
)
$requestedTargets = @()
if (-not [string]::IsNullOrWhiteSpace($env:OPENPIT_RELEASE_E2E_TARGETS)) {
  $requestedTargets = @(
    $env:OPENPIT_RELEASE_E2E_TARGETS -split '\s+' |
      Where-Object { -not [string]::IsNullOrWhiteSpace($_) }
  )
  if ($requestedTargets.Count -eq 0) {
    throw 'OPENPIT_RELEASE_E2E_TARGETS did not select any targets'
  }
  foreach ($target in $requestedTargets) {
    if ($knownTargets -notcontains $target) {
      throw "unknown Windows release E2E target: $target; known targets: $($knownTargets -join ' ')"
    }
  }
}

$dockerPlatform = & docker version --format '{{.Server.Os}}/{{.Server.Arch}}'
if ($LASTEXITCODE -ne 0) {
  throw 'could not query the Docker server'
}
$dockerPlatform = "$dockerPlatform".Trim()
if ($dockerPlatform -ne 'windows/amd64') {
  throw "Windows release E2E requires a Windows Docker engine, got '$dockerPlatform'"
}

$image = "openpit-release-e2e-windows:$Version"
$dockerfile = Join-Path $rootDir 'e2e\env\docker\windows-binary\Dockerfile'
& docker build --pull --isolation=process --memory 4GB --tag $image --file $dockerfile $rootDir
if ($LASTEXITCODE -ne 0) {
  exit $LASTEXITCODE
}

$targetList = if ($requestedTargets.Count -eq 0) {
  $knownTargets -join ' '
} else {
  $requestedTargets -join ' '
}
$runArgs = @(
  'run',
  '--rm',
  '--isolation=process',
  '--env', "OPENPIT_VERSION=$Version",
  '--env', "OPENPIT_RELEASE_E2E_TARGETS=$targetList"
)
foreach ($envName in @(
  'OPENPIT_RELEASE_DOWNLOAD_TOKEN',
  'OPENPIT_RELEASE_REPOSITORY',
  'OPENPIT_RELEASE_ID',
  'OPENPIT_VCPKG_REGISTRY_BASELINE',
  'OPENPIT_VCPKG_REGISTRY_REPOSITORY'
)) {
  $value = [Environment]::GetEnvironmentVariable($envName)
  if (-not [string]::IsNullOrWhiteSpace($value)) {
    $runArgs += '--env', "$envName=$value"
  }
}

if (-not [string]::IsNullOrWhiteSpace($env:OPENPIT_VCPKG_REGISTRY_PATH)) {
  $registryPath = (Resolve-Path $env:OPENPIT_VCPKG_REGISTRY_PATH).Path
  $runArgs += '--env', 'OPENPIT_VCPKG_REGISTRY_PATH=C:\e2e\vcpkg-registry'
  $runArgs += '--mount', "type=bind,source=$registryPath,target=C:\e2e\vcpkg-registry,readonly"
}

$runArgs += $image
& docker @runArgs
exit $LASTEXITCODE
