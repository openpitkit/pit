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

$sourcePath = Join-Path $PSScriptRoot '..\diagnose-windows-image.ps1'
$source = Get-Content -Raw -LiteralPath $sourcePath
$tokens = $null
$parseErrors = $null
$ast = [System.Management.Automation.Language.Parser]::ParseInput($source, [ref]$tokens, [ref]$parseErrors)
if ($parseErrors.Count -ne 0) { throw ($parseErrors -join "`n") }
$functions = $ast.FindAll({ param($node) $node -is [System.Management.Automation.Language.FunctionDefinitionAst] }, $true)
$native = $functions | Where-Object Name -eq 'Invoke-Native'
foreach ($name in @('ConvertTo-NativeArgument', 'Invoke-Native')) {
  . ([scriptblock]::Create(($functions | Where-Object Name -eq $name).Extent.Text))
}
$testRoot = Join-Path ([IO.Path]::GetTempPath()) ('openpit-image-diagnostic-tests-' + [guid]::NewGuid().ToString('N'))
$OutputDirectory = Join-Path $testRoot 'native-logs'
New-Item -ItemType Directory -Path $OutputDirectory | Out-Null
$powershell = (Get-Command powershell.exe -CommandType Application).Source
$text = Invoke-Native $powershell @('-NoProfile', '-Command', '[Console]::Error.WriteLine("stderr evidence"); [Console]::WriteLine("stdout evidence"); exit 0') 'native-success'
if ($text.Trim() -ne 'stdout evidence') { throw 'native stdout or argument quoting was corrupted' }
if ((Get-Content -Raw (Join-Path $OutputDirectory 'native-success.stderr.log')).Trim() -ne 'stderr evidence') {
  throw 'native stderr was not captured'
}
try {
  Invoke-Native $powershell @('-NoProfile', '-Command', '[Console]::Error.WriteLine("failure evidence"); exit 47') 'native-failure' | Out-Null
  throw 'native failure was swallowed'
} catch {
  if ($_.Exception.Data['NativeExitCode'] -ne 47) { throw }
}
$OutputDirectory = Join-Path $testRoot 'missing-log-parent'
try {
  Invoke-Native $powershell @('-NoProfile', '-Command', 'exit 48') 'write-failure' | Out-Null
  throw 'native failure was lost when evidence persistence failed'
} catch {
  if ($_.Exception.Data['NativeExitCode'] -ne 48) { throw }
}

$mock = @'
function Invoke-Native {
  param([string]$File, [string[]]$Arguments, [string]$LogName)
  "$LogName $($Arguments -join ' ')" | Add-Content (Join-Path $OutputDirectory 'calls.log')
  $case = $env:OPENPIT_DIAGNOSTIC_TEST_CASE
  $code = 0
  if ($LogName -eq 'installer-run' -and $case -match '^installer') { $code = 42 }
  if ($LogName -eq 'docker-commit' -and $case -eq 'commit-failure') { $code = 17 }
  if ($LogName -eq 'copy-dd_setup.log' -and $case -eq 'copy-failure') { $code = 23 }
  if ($LogName -eq 'export-docker-events' -and $case -match 'export-failure') { $code = 24 }
  if ($LogName -eq 'hcs-analytic-restore' -and $case -eq 'restore-failure') { $code = 25 }
  if ($code -ne 0) {
    if ($case -eq 'installer-log-write-failure') {
      New-Item -ItemType Directory -Path (Join-Path $OutputDirectory 'failures.log') | Out-Null
    }
    $failure = New-Object System.Exception "mock $LogName failed"
    $failure.Data['NativeExitCode'] = $code
    throw $failure
  }
  if ($LogName -eq 'hcs-analytic-state') {
    if ($case -eq 'already-enabled') { return '<channel enabled="true" />' }
    return '<channel enabled="false" />'
  }
  if ($LogName -eq 'container-state') {
    if ($case -match '^installer') { return '{"Status":"exited","ExitCode":42}' }
    return '{"Status":"exited","ExitCode":0}'
  }
  if ($LogName -eq 'copy-container-output') {
    $destination = $Arguments[2]
    New-Item -ItemType Directory -Path $destination | Out-Null
    $exit = if ($case -eq 'negative-exit-file') { -1 } elseif ($case -match '^installer') { 42 } else { 0 }
    if ($case -eq 'empty-exit-file') {
      [IO.File]::WriteAllText((Join-Path $destination 'installer-run-exit-code.txt'), '')
    } else {
      $exit | Set-Content (Join-Path $destination 'installer-run-exit-code.txt')
    }
    '["C:\\Temp\\dd_setup.log","C:\\Temp\\dd_install.log"]' | Set-Content (Join-Path $destination 'installer-logs.json')
  }
  return ''
}
'@
$mocked = $source.Remove($native.Extent.StartOffset, $native.Extent.EndOffset - $native.Extent.StartOffset).Insert($native.Extent.StartOffset, $mock)
$fixtureDirectory = Join-Path $testRoot 'env\docker\windows-binary'
New-Item -ItemType Directory -Path $fixtureDirectory | Out-Null
Copy-Item -LiteralPath (Join-Path $PSScriptRoot '..\env\docker\windows-binary\Dockerfile') -Destination $fixtureDirectory
$scriptPath = Join-Path $testRoot 'diagnostic.ps1'
$digest = 'mcr.microsoft.com/dotnet/framework/runtime@sha256:' + ('a' * 64)
$script:sequence = 0

function Invoke-Case {
  param([string]$Case, [int]$ExpectedExit, [string]$Code = $mocked)

  $script:sequence++
  $out = Join-Path $testRoot "case-$script:sequence"
  $Code | Set-Content -Encoding UTF8 $scriptPath
  $env:OPENPIT_DIAGNOSTIC_TEST_CASE = $Case
  & $powershell -NoProfile -ExecutionPolicy Bypass -File $scriptPath -BaseImage $digest -OutputDirectory $out *> (Join-Path $testRoot "case-$script:sequence.log")
  $actual = $LASTEXITCODE
  if ($actual -ne $ExpectedExit) { throw "$Case expected exit $ExpectedExit, got $actual" }
  $calls = @(Get-Content (Join-Path $out 'calls.log'))
  if ($Case -eq 'invalid-run') {
    if ($calls -match '^docker-create |^docker-commit ') { throw 'invalid RUN reached container creation' }
    return
  }
  $restore = @($calls | Where-Object { $_ -match '^hcs-analytic-restore ' })
  $expectedState = if ($Case -eq 'already-enabled') { 'true' } else { 'false' }
  if ($restore.Count -ne 1 -or $restore[0] -notmatch "/e:$expectedState") { throw "$Case did not restore HCS state" }
  $commitIndex = -1
  $copyIndex = -1
  for ($i = 0; $i -lt $calls.Count; $i++) {
    if ($calls[$i] -match '^docker-commit ') { $commitIndex = $i }
    if ($calls[$i] -match '^copy-dd_setup.log ') { $copyIndex = $i }
  }
  if ($Case -eq 'empty-exit-file') {
    if ($copyIndex -ge 0) { throw 'empty exit code was accepted before copying installer logs' }
    if ((Get-Content -Raw (Join-Path $out 'failures.log')) -notmatch 'must contain the Dockerfile RUN command exit code') {
      throw 'missing installer exit code was not explained'
    }
  } else {
    if ($copyIndex -lt 0) { throw "$Case skipped installer log copying" }
    if ($Case -ne 'copy-failure' -and -not ($calls -match '^copy-dd_install.log ')) {
      throw "$Case did not copy every installer log from the JSON manifest"
    }
  }
  $shouldCommit = $Case -notmatch '^installer|^copy-failure|^empty-exit-file|^negative-exit-file'
  if ($shouldCommit -and $commitIndex -le $copyIndex) { throw "$Case did not copy logs before commit" }
  if (-not $shouldCommit -and $commitIndex -ge 0) { throw "$Case committed after a failed prerequisite" }
  if (-not ($calls -match '^export-docker-events ') -or -not ($calls -match '^docker-info-after ')) {
    throw "$Case skipped final diagnostics"
  }
}

try {
  Invoke-Case 'success' 0
  $successOutput = Join-Path $testRoot "case-$script:sequence"
  $exitLines = @(Get-Content (Join-Path $successOutput 'run-installer.cmd') | Where-Object { $_ -match 'installer-run-exit-code\.txt' })
  if ($exitLines.Count -ne 1) { throw 'expected exactly one CMD exit-code recording command' }
  $OutputDirectory = Join-Path $testRoot 'cmd-logs'
  New-Item -ItemType Directory -Path $OutputDirectory | Out-Null
  foreach ($expectedCode in @(0, 42, 3010, -1)) {
    $exitPath = Join-Path $testRoot "cmd-exit-$expectedCode.txt"
    $commandPath = Join-Path $testRoot "record-exit-$expectedCode.cmd"
    @(
      '@echo off'
      "set `"INSTALLER_RUN_EXIT=$expectedCode`""
      $exitLines[0].Replace('C:\openpit-image-diagnostics\installer-run-exit-code.txt', "`"$exitPath`"")
    ) | Set-Content -Encoding ASCII $commandPath
    Invoke-Native 'cmd.exe' @('/d', '/c', $commandPath) "cmd-exit-$expectedCode" | Out-Null
    if ((Get-Content -Raw -LiteralPath $exitPath).Trim() -ne "$expectedCode") {
      throw "CMD exit-code recording corrupted $expectedCode"
    }
  }
  Invoke-Case 'already-enabled' 0
  Invoke-Case 'empty-exit-file' 1
  Invoke-Case 'negative-exit-file' -1
  Invoke-Case 'installer-failure' 42
  Invoke-Case 'commit-failure' 17
  Invoke-Case 'copy-failure' 23
  Invoke-Case 'export-failure' 24
  Invoke-Case 'restore-failure' 25
  Invoke-Case 'installer-export-failure' 42
  Invoke-Case 'installer-log-write-failure' 42

  $mutations = @(
    @{ Case = 'negative-exit-file'; Exit = -1; From = '\A-?[0-9]+\s*\z'; To = '\A[0-9]+\s*\z' },
    @{ Case = 'installer-failure'; Exit = 42; From = '$script:resultCode = $failureCode'; To = '$script:resultCode = 1' },
    @{ Case = 'commit-failure'; Exit = 17; From = '$script:resultCode = $failureCode'; To = '$script:resultCode = 1' },
    @{ Case = 'copy-failure'; Exit = 23; From = '$script:resultCode = $script:diagnosticCode'; To = '$script:resultCode = 0' },
    @{ Case = 'export-failure'; Exit = 24; From = '$script:resultCode = $script:diagnosticCode'; To = '$script:resultCode = 0' },
    @{ Case = 'restore-failure'; Exit = 25; From = '$script:resultCode = $script:diagnosticCode'; To = '$script:resultCode = 0' },
    @{ Case = 'installer-export-failure'; Exit = 42; From = 'if ($script:resultCode -eq 0 -and $script:diagnosticCode -ne 0)'; To = 'if ($script:diagnosticCode -ne 0)' },
    @{ Case = 'already-enabled'; Exit = 0; From = '$analyticWasEnabled.ToString().ToLowerInvariant()'; To = "'false'" },
    @{ Case = 'installer-log-write-failure'; Exit = 42; From = 'Write-Host "COULD NOT WRITE FAILURE LOG: $_"'; To = 'throw' }
  )
  foreach ($mutation in $mutations) {
    if (-not $mocked.Contains($mutation.From)) { throw 'mutation anchor is missing' }
    $caught = $false
    try {
      Invoke-Case $mutation.Case $mutation.Exit ($mocked.Replace($mutation.From, $mutation.To))
    } catch { $caught = $true }
    if (-not $caught) { throw "mutation survived: $($mutation.Case)" }
  }
  $fixturePath = Join-Path $fixtureDirectory 'Dockerfile'
  $fixture = Get-Content -Raw -LiteralPath $fixturePath
  try {
    $fixture.Replace('RUN curl.exe', 'RUN unexpected.exe') | Set-Content -Encoding UTF8 $fixturePath
    Invoke-Case 'invalid-run' 1
    $caught = $false
    try {
      Invoke-Case 'invalid-run' 1 ($mocked.Replace('^curl\.exe', '^unexpected\.exe'))
    } catch { $caught = $true }
    if (-not $caught) { throw 'invalid RUN mutation survived' }
  } finally {
    $fixture | Set-Content -Encoding UTF8 $fixturePath
  }
  $mocked | Set-Content -Encoding UTF8 $scriptPath
  Write-Host 'Passed native output/exit checks, four real CMD exit-code checks, 12 lifecycle cases, and 10 intentional mutation checks.'
  Write-Host "Test evidence: $testRoot"
} finally {
  Remove-Item Env:\OPENPIT_DIAGNOSTIC_TEST_CASE
}
