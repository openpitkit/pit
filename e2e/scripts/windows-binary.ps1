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

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'

if ([string]::IsNullOrWhiteSpace($env:OPENPIT_VERSION)) {
  throw 'OPENPIT_VERSION is required'
}

$version = $env:OPENPIT_VERSION
$releaseRepository = if ($env:OPENPIT_RELEASE_REPOSITORY) {
  $env:OPENPIT_RELEASE_REPOSITORY
} else {
  'openpitkit/pit'
}
$releaseApi = "https://api.github.com/repos/$releaseRepository"
$releaseBaseUrl = "https://github.com/$releaseRepository/releases/download/v$version"
$workRoot = 'C:\work\openpit-release-e2e'
$script:releaseMetadata = $null
$knownTargets = @(
  'python-wheel-windows-amd64',
  'go-windows-amd64',
  'cpp-windows-amd64',
  'cpp-vcpkg-windows-amd64'
)

function Invoke-External {
  param(
    [Parameter(Mandatory)]
    [string]$File,
    [string[]]$Arguments = @()
  )

  & $File @Arguments
  if ($LASTEXITCODE -ne 0) {
    throw "command failed ($LASTEXITCODE): $File $($Arguments -join ' ')"
  }
}

function Remove-Directory {
  param([Parameter(Mandatory)][string]$Path)

  if (Test-Path $Path) {
    Remove-Item -Recurse -Force $Path
  }
}

function Get-ReleaseMetadata {
  if ($null -ne $script:releaseMetadata) {
    return $script:releaseMetadata
  }

  $headers = @{ Accept = 'application/vnd.github+json' }
  if (-not [string]::IsNullOrWhiteSpace($env:OPENPIT_RELEASE_DOWNLOAD_TOKEN)) {
    $headers.Authorization = "Bearer $env:OPENPIT_RELEASE_DOWNLOAD_TOKEN"
  }
  $releaseUri = "$releaseApi/releases/tags/v$version"
  if (-not [string]::IsNullOrWhiteSpace($env:OPENPIT_RELEASE_ID)) {
    if ($env:OPENPIT_RELEASE_ID -notmatch '^\d+$') {
      throw 'OPENPIT_RELEASE_ID must be numeric'
    }
    $releaseUri = "$releaseApi/releases/$env:OPENPIT_RELEASE_ID"
  }
  $script:releaseMetadata = Invoke-RestMethod -Headers $headers -Uri $releaseUri
  return $script:releaseMetadata
}

function Download-ReleaseAsset {
  param(
    [Parameter(Mandatory)][string]$Name,
    [Parameter(Mandatory)][string]$Destination
  )

  $destinationDirectory = Split-Path -Parent $Destination
  New-Item -ItemType Directory -Force $destinationDirectory | Out-Null
  if ([string]::IsNullOrWhiteSpace($env:OPENPIT_RELEASE_DOWNLOAD_TOKEN)) {
    Invoke-WebRequest "$releaseBaseUrl/$Name" -OutFile $Destination
    return
  }

  $asset = (Get-ReleaseMetadata).assets | Where-Object { $_.name -eq $Name } | Select-Object -First 1
  if ($null -eq $asset) {
    throw "release asset not found: $Name"
  }
  Invoke-WebRequest -Headers @{
    Authorization = "Bearer $env:OPENPIT_RELEASE_DOWNLOAD_TOKEN"
    Accept = 'application/octet-stream'
  } -Uri $asset.url -OutFile $Destination
}

function Test-FileSha256 {
  param(
    [Parameter(Mandatory)][string]$Path,
    [Parameter(Mandatory)][string]$Sidecar
  )

  $expected = (Get-Content -Raw $Sidecar).Trim().ToLowerInvariant()
  $actual = (Get-FileHash -Algorithm SHA256 $Path).Hash.ToLowerInvariant()
  if ($actual -ne $expected) {
    throw "sha256 mismatch for $Path"
  }
}

function Download-VerifiedReleaseAsset {
  param(
    [Parameter(Mandatory)][string]$Name,
    [Parameter(Mandatory)][string]$Destination
  )

  Download-ReleaseAsset $Name $Destination
  Download-ReleaseAsset "$Name.sha256" "$Destination.sha256"
  Test-FileSha256 $Destination "$Destination.sha256"
}

function Get-DraftRuntimePath {
  if ([string]::IsNullOrWhiteSpace($env:OPENPIT_RELEASE_DOWNLOAD_TOKEN)) {
    return $null
  }

  $runtimeDirectory = Join-Path $workRoot 'runtime'
  $runtimePath = Join-Path $runtimeDirectory 'openpit_ffi.dll'
  Download-VerifiedReleaseAsset 'openpit-ffi--windows-amd64-openpit_ffi.dll' $runtimePath
  Download-VerifiedReleaseAsset 'openpit-ffi--windows-amd64-openpit_ffi.dll.lib' "$runtimePath.lib"
  return $runtimePath
}

function Get-SelectedTargets {
  if ([string]::IsNullOrWhiteSpace($env:OPENPIT_RELEASE_E2E_TARGETS)) {
    return $knownTargets
  }

  $targets = @(
    $env:OPENPIT_RELEASE_E2E_TARGETS -split '\s+' |
      Where-Object { -not [string]::IsNullOrWhiteSpace($_) }
  )
  if ($targets.Count -eq 0) {
    throw 'OPENPIT_RELEASE_E2E_TARGETS did not select any targets'
  }
  foreach ($target in $targets) {
    if ($knownTargets -notcontains $target) {
      throw "unknown Windows release E2E target: $target; known targets: $($knownTargets -join ' ')"
    }
  }
  return $targets
}

function Get-ConsumerExecutable {
  param([Parameter(Mandatory)][string]$BuildDirectory)

  $releasePath = Join-Path $BuildDirectory 'Release\openpit_cpp_consumer.exe'
  if (Test-Path $releasePath) {
    return $releasePath
  }
  $defaultPath = Join-Path $BuildDirectory 'openpit_cpp_consumer.exe'
  if (Test-Path $defaultPath) {
    return $defaultPath
  }
  throw "C++ consumer executable not found under $BuildDirectory"
}

function Test-CppDistributable {
  $workDirectory = Join-Path $workRoot 'cpp'
  $installDirectory = Join-Path $workDirectory 'install'
  $archive = Join-Path $workDirectory "openpit-cpp--$version.tar.gz"
  Remove-Directory $workDirectory
  New-Item -ItemType Directory -Force $installDirectory | Out-Null

  Write-Host "==> Downloading C++ distributable for $version"
  Download-VerifiedReleaseAsset "openpit-cpp--$version.tar.gz" $archive
  Invoke-External 'tar.exe' @('-xzf', $archive, '-C', $installDirectory)

  $runtimePath = Get-DraftRuntimePath
  $cmakeArgs = @(
    '-S', 'C:\e2e\cpp-consumer',
    '-B', (Join-Path $workDirectory 'consumer-build'),
    "-DCMAKE_PREFIX_PATH=$installDirectory"
  )
  if ($null -ne $runtimePath) {
    $cmakeArgs += "-DOPENPIT_RUNTIME_LIBRARY=$runtimePath"
  }
  Invoke-External 'cmake.exe' $cmakeArgs
  $consumerBuild = Join-Path $workDirectory 'consumer-build'
  Invoke-External 'cmake.exe' @('--build', $consumerBuild, '--config', 'Release', '--parallel')
  Invoke-External (Get-ConsumerExecutable $consumerBuild)

  $examplesRoot = Join-Path $workDirectory 'examples'
  Copy-Item -Recurse -Force 'C:\e2e\cpp-examples' $examplesRoot
  Copy-Item -Recurse -Force 'C:\e2e\tables' (Join-Path $workDirectory 'tables')
  $exampleArgs = @{
    'rate_pnl_killswitch' = @('-DOPENPIT_EXAMPLE_USE_INSTALLED_PACKAGE=ON')
    'spot_funds' = @()
    'spot_table' = @('-DOPENPIT_USE_FIND_PACKAGE=ON')
    'spot_loadtest' = @('-DOPENPIT_USE_FIND_PACKAGE=ON')
  }
  foreach ($name in $exampleArgs.Keys) {
    $source = Join-Path $examplesRoot $name
    $build = Join-Path $workDirectory "example-build-$name"
    $exampleCmakeArgs = @(
      '-S', $source,
      '-B', $build,
      "-DCMAKE_PREFIX_PATH=$installDirectory"
    )
    if ($null -ne $runtimePath) {
      $exampleCmakeArgs += "-DOPENPIT_RUNTIME_LIBRARY=$runtimePath"
    }
    $exampleCmakeArgs += $exampleArgs[$name]
    Write-Host "==> Building C++ example $name"
    Invoke-External 'cmake.exe' $exampleCmakeArgs
    Invoke-External 'cmake.exe' @('--build', $build, '--config', 'Release', '--parallel')
    Invoke-External 'ctest.exe' @('--test-dir', $build, '--build-config', 'Release', '--output-on-failure')
  }
}

function Resolve-VcpkgRegistryConfiguration {
  if (-not [string]::IsNullOrWhiteSpace($env:OPENPIT_VCPKG_REGISTRY_PATH)) {
    return @{
      kind = 'filesystem'
      path = $env:OPENPIT_VCPKG_REGISTRY_PATH
      baseline = 'default'
      packages = @('openpit')
    }
  }

  if ([string]::IsNullOrWhiteSpace($env:OPENPIT_VCPKG_REGISTRY_BASELINE)) {
    $body = (Get-ReleaseMetadata).body
    if ($body -notmatch 'vcpkg-registry\.git at baseline\s+`([0-9a-f]{40})`') {
      throw 'could not resolve the OpenPit vcpkg registry baseline from release notes'
    }
    $env:OPENPIT_VCPKG_REGISTRY_BASELINE = $Matches[1]
  }
  return @{
    kind = 'git'
    repository = if ($env:OPENPIT_VCPKG_REGISTRY_REPOSITORY) {
      $env:OPENPIT_VCPKG_REGISTRY_REPOSITORY
    } else {
      'https://github.com/openpitkit/vcpkg-registry.git'
    }
    baseline = $env:OPENPIT_VCPKG_REGISTRY_BASELINE
    packages = @('openpit')
  }
}

# vcpkg keeps the port's CMake output in per-port log files and only prints
# their paths, which are unreachable once the container exits. Dump them so a
# failed build stays diagnosable from the CI log alone.
function Show-VcpkgPortLogs {
  param([Parameter(Mandatory)][string]$VcpkgRoot)

  $logDirectory = Join-Path $VcpkgRoot 'buildtrees\openpit'
  if (-not (Test-Path $logDirectory)) {
    return
  }
  Get-ChildItem -Path $logDirectory -Filter '*.log' -File | ForEach-Object {
    Write-Host "==> vcpkg log $($_.Name)"
    Write-Host (Get-Content -Raw $_.FullName)
  }
}

function Test-CppVcpkg {
  $workDirectory = Join-Path $workRoot 'cpp-vcpkg'
  $vcpkgRoot = Join-Path $workDirectory 'vcpkg'
  Remove-Directory $workDirectory
  New-Item -ItemType Directory -Force $workDirectory | Out-Null

  $runtimePath = Get-DraftRuntimePath
  Write-Host '==> Bootstrapping isolated vcpkg'
  Invoke-External 'git.exe' @('clone', '--depth', '1', 'https://github.com/microsoft/vcpkg.git', $vcpkgRoot)
  Invoke-External (Join-Path $vcpkgRoot 'bootstrap-vcpkg.bat') @('-disableMetrics')
  $vcpkgBaseline = & git.exe -C $vcpkgRoot rev-parse HEAD
  if ($LASTEXITCODE -ne 0) {
    throw 'could not resolve the vcpkg baseline'
  }
  $vcpkgBaseline = "$vcpkgBaseline".Trim()

  @{
    name = 'openpit-release-e2e-cpp-vcpkg'
    'version-string' = '0'
    'builtin-baseline' = $vcpkgBaseline
    dependencies = @('openpit')
  } | ConvertTo-Json -Depth 4 | Set-Content -Encoding ascii (Join-Path $workDirectory 'vcpkg.json')
  $registry = Resolve-VcpkgRegistryConfiguration
  @{ registries = @($registry) } |
    ConvertTo-Json -Depth 6 |
    Set-Content -Encoding ascii (Join-Path $workDirectory 'vcpkg-configuration.json')

  $previousRuntime = $env:OPENPIT_VCPKG_RUNTIME_LIBRARY
  $previousKeepEnvVars = $env:VCPKG_KEEP_ENV_VARS
  try {
    if ($null -ne $runtimePath) {
      $env:OPENPIT_VCPKG_RUNTIME_LIBRARY = $runtimePath
      # On Windows vcpkg builds ports in a clean environment isolated from the
      # caller, so the draft-runtime override has to be allowed through
      # explicitly or the port falls back to the not-yet-public release asset.
      $keepEnvVars = 'OPENPIT_VCPKG_RUNTIME_LIBRARY'
      if (-not [string]::IsNullOrWhiteSpace($previousKeepEnvVars)) {
        $keepEnvVars = "$previousKeepEnvVars;$keepEnvVars"
      }
      $env:VCPKG_KEEP_ENV_VARS = $keepEnvVars
    }
    Push-Location $workDirectory
    try {
      Invoke-External (Join-Path $vcpkgRoot 'vcpkg.exe') @('install', '--triplet', 'x64-windows')
    } catch {
      Show-VcpkgPortLogs $vcpkgRoot
      throw
    } finally {
      Pop-Location
    }
  } finally {
    $env:OPENPIT_VCPKG_RUNTIME_LIBRARY = $previousRuntime
    $env:VCPKG_KEEP_ENV_VARS = $previousKeepEnvVars
  }

  # Toolchain-only arguments, shared by the consumer and every example. Source
  # and build directories stay per-project.
  $vcpkgCmakeArgs = @(
    "-DCMAKE_TOOLCHAIN_FILE=$vcpkgRoot\scripts\buildsystems\vcpkg.cmake",
    "-DVCPKG_MANIFEST_DIR=$workDirectory",
    "-DVCPKG_INSTALLED_DIR=$workDirectory\vcpkg_installed",
    '-DVCPKG_TARGET_TRIPLET=x64-windows'
  )
  if ($null -ne $runtimePath) {
    $vcpkgCmakeArgs += "-DOPENPIT_RUNTIME_LIBRARY=$runtimePath"
  }
  $consumerBuild = Join-Path $workDirectory 'consumer-build'
  Write-Host '==> Building minimal C++ consumer through vcpkg'
  Invoke-External 'cmake.exe' (
    @('-S', 'C:\e2e\cpp-consumer', '-B', $consumerBuild) + $vcpkgCmakeArgs
  )
  Invoke-External 'cmake.exe' @('--build', $consumerBuild, '--config', 'Release', '--parallel')
  Invoke-External (Get-ConsumerExecutable $consumerBuild)

  $examplesRoot = Join-Path $workDirectory 'examples'
  Copy-Item -Recurse -Force 'C:\e2e\cpp-examples' $examplesRoot
  Copy-Item -Recurse -Force 'C:\e2e\tables' (Join-Path $workDirectory 'tables')
  $exampleArgs = @{
    'rate_pnl_killswitch' = @('-DOPENPIT_EXAMPLE_USE_INSTALLED_PACKAGE=ON')
    'spot_funds' = @()
    'spot_table' = @('-DOPENPIT_USE_FIND_PACKAGE=ON')
    'spot_loadtest' = @('-DOPENPIT_USE_FIND_PACKAGE=ON')
  }
  foreach ($name in $exampleArgs.Keys) {
    $source = Join-Path $examplesRoot $name
    $build = Join-Path $workDirectory "example-build-$name"
    $exampleCmakeArgs = @('-S', $source, '-B', $build) + $vcpkgCmakeArgs + $exampleArgs[$name]
    Write-Host "==> Building C++ example $name through vcpkg"
    Invoke-External 'cmake.exe' $exampleCmakeArgs
    Invoke-External 'cmake.exe' @('--build', $build, '--config', 'Release', '--parallel')
    Invoke-External 'ctest.exe' @('--test-dir', $build, '--build-config', 'Release', '--output-on-failure')
  }
}

function Test-PythonWheel {
  $workDirectory = Join-Path $workRoot 'python-wheel'
  $distDirectory = Join-Path $workDirectory 'dist'
  Remove-Directory $workDirectory
  New-Item -ItemType Directory -Force $distDirectory | Out-Null

  Write-Host "==> Installing openpit $version Windows wheel"
  Invoke-External 'python.exe' @('-m', 'pip', 'download', '--only-binary=:all:', '--no-deps', "openpit==$version", '--dest', $distDirectory)
  $wheel = Get-ChildItem -Path $distDirectory -Filter '*.whl' | Select-Object -First 1
  if ($null -eq $wheel) {
    throw "could not download a Windows wheel for openpit $version"
  }
  Invoke-External 'python.exe' @('-m', 'pip', 'install', '--force-reinstall', '--no-deps', $wheel.FullName)
  Invoke-External 'python.exe' @('-c', "import importlib.metadata as m; assert m.version('openpit') == '$version'")

  Push-Location 'C:\e2e\tests'
  try {
    Invoke-External 'python.exe' @('-m', 'pytest', 'integration')
  } finally {
    Pop-Location
  }
  Get-ChildItem -Path 'C:\e2e\python-examples' -Directory | ForEach-Object {
    Write-Host "==> Testing Python example $($_.Name) against openpit $version"
    Invoke-External 'python.exe' @('-m', 'pytest', $_.FullName)
  }
}

function Test-GoModule {
  $workDirectory = Join-Path $workRoot 'go-module'
  Remove-Directory $workDirectory
  New-Item -ItemType Directory -Force $workDirectory | Out-Null

  $consumerDirectory = Join-Path $workDirectory 'consumer'
  Copy-Item -Recurse -Force 'C:\e2e\go-consumer' $consumerDirectory
  $goMod = Join-Path $consumerDirectory 'go.mod.in'
  (Get-Content -Raw $goMod).Replace('v__OPENPIT_VERSION__', "v$version") |
    Set-Content -Encoding ascii (Join-Path $consumerDirectory 'go.mod')
  Remove-Item $goMod
  Push-Location $consumerDirectory
  try {
    Invoke-External 'go.exe' @('mod', 'tidy')
    Invoke-External 'go.exe' @('test', './...')
  } finally {
    Pop-Location
  }

  $examplesRoot = Join-Path $workDirectory 'examples'
  New-Item -ItemType Directory -Force (Join-Path $examplesRoot 'go') | Out-Null
  Copy-Item -Recurse -Force 'C:\e2e\tables' (Join-Path $examplesRoot 'tables')
  Get-ChildItem -Path 'C:\e2e\go-examples' -Directory | ForEach-Object {
    $name = $_.Name
    $destination = Join-Path (Join-Path $examplesRoot 'go') $name
    Copy-Item -Recurse -Force $_.FullName $destination
    $exampleGoMod = Join-Path $destination 'go.mod'
    $contents = Get-Content $exampleGoMod |
      Where-Object { $_ -notmatch '^replace go\.openpit\.dev/openpit' } |
      ForEach-Object {
        if ($_ -match '^require go\.openpit\.dev/openpit') {
          "require go.openpit.dev/openpit v$version"
        } else {
          $_
        }
      }
    Set-Content -Encoding ascii $exampleGoMod $contents
    Push-Location $destination
    try {
      Write-Host "==> Testing Go example $name against go.openpit.dev/openpit $version"
      Invoke-External 'go.exe' @('mod', 'tidy')
      Invoke-External 'go.exe' @('test', './...')
    } finally {
      Pop-Location
    }
  }
}

Remove-Directory $workRoot
New-Item -ItemType Directory -Force $workRoot | Out-Null

$tests = @{
  'python-wheel-windows-amd64' = { Test-PythonWheel }
  'go-windows-amd64' = { Test-GoModule }
  'cpp-windows-amd64' = { Test-CppDistributable }
  'cpp-vcpkg-windows-amd64' = { Test-CppVcpkg }
}
$passedTargets = @()
$failedTargets = @()
foreach ($target in (Get-SelectedTargets)) {
  Write-Host "==> Running Windows release E2E target $target"
  try {
    & $tests[$target]
    $passedTargets += $target
    Write-Host "==> Windows release E2E target $target passed"
  } catch {
    $failedTargets += "${target}: $($_.Exception.Message)"
    Write-Host "==> Windows release E2E target $target failed: $($_.Exception.Message)"
  }
}

if ($failedTargets.Count -ne 0) {
  Write-Host '============================================================'
  Write-Host 'WINDOWS RELEASE E2E FAILED'
  Write-Host '============================================================'
  Write-Host "Passed: $($passedTargets.Count)"
  Write-Host "Failed: $($failedTargets.Count)"
  $failedTargets | ForEach-Object { Write-Host "Failure: $_" }
  exit 1
}

Write-Host '============================================================'
Write-Host 'WINDOWS RELEASE E2E PASSED'
Write-Host '============================================================'
Write-Host "Passed: $($passedTargets.Count)"
