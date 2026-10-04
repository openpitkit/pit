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
  [Parameter(Mandatory)]
  [ValidatePattern('@sha256:[0-9a-f]{64}$')]
  [string]$BaseImage,
  [Parameter(Mandatory)]
  [ValidateNotNullOrEmpty()]
  [string]$OutputDirectory
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

function ConvertTo-NativeArgument {
  param([AllowEmptyString()][string]$Value)

  '"' + (($Value -replace '(\\*)"', '$1$1\"') -replace '(\\+)$', '$1$1') + '"'
}

function Invoke-Native {
  param([string]$File, [string[]]$Arguments, [string]$LogName)

  $startInfo = New-Object System.Diagnostics.ProcessStartInfo
  $startInfo.FileName = (Get-Command $File -CommandType Application -ErrorAction Stop).Source
  $startInfo.Arguments = ($Arguments | ForEach-Object { ConvertTo-NativeArgument $_ }) -join ' '
  $startInfo.UseShellExecute = $false
  $startInfo.CreateNoWindow = $true
  $startInfo.RedirectStandardOutput = $true
  $startInfo.RedirectStandardError = $true
  $process = New-Object System.Diagnostics.Process
  $process.StartInfo = $startInfo
  try {
    if (-not $process.Start()) { throw "could not start $File" }
    $stdout = $process.StandardOutput.ReadToEndAsync()
    $stderr = $process.StandardError.ReadToEndAsync()
    $process.WaitForExit()
    $stdoutText = $stdout.GetAwaiter().GetResult()
    $stderrText = $stderr.GetAwaiter().GetResult()
    Write-Host $stdoutText
    Write-Host $stderrText
    try {
      $stdoutText | Set-Content -Encoding UTF8 (Join-Path $OutputDirectory "$LogName.stdout.log")
      $stderrText | Set-Content -Encoding UTF8 (Join-Path $OutputDirectory "$LogName.stderr.log")
    } catch {
      if ($process.ExitCode -ne 0) { $_.Exception.Data['NativeExitCode'] = $process.ExitCode }
      throw
    }
    if ($process.ExitCode -ne 0) {
      $failure = New-Object System.Exception "$File failed with exit code $($process.ExitCode); see $LogName.stderr.log"
      $failure.Data['NativeExitCode'] = $process.ExitCode
      throw $failure
    }
    return $stdoutText
  } finally {
    $process.Dispose()
  }
}

function Save-DriveSpace {
  param([string]$Phase)

  Get-PSDrive -PSProvider FileSystem |
    Select-Object Name, Root, Used, Free |
    ConvertTo-Json |
    Set-Content -Encoding UTF8 (Join-Path $OutputDirectory "host-drives-$Phase.json")
}

function Save-Failure {
  param([System.Management.Automation.ErrorRecord]$Failure, [switch]$Primary)

  $failureCode = 1
  if ($Failure.Exception.Data.Contains('NativeExitCode')) {
    $failureCode = [int]$Failure.Exception.Data['NativeExitCode']
  }
  if ($Primary -and $script:resultCode -eq 0) {
    $script:resultCode = $failureCode
  } elseif (-not $Primary -and $script:diagnosticCode -eq 0) {
    $script:diagnosticCode = $failureCode
  }
  $script:failureCount++
  $message = $Failure.ToString()
  Write-Host "DIAGNOSTIC FAILURE: $message"
  try {
    $message | Add-Content -Encoding UTF8 (Join-Path $OutputDirectory 'failures.log')
  } catch {
    Write-Host "COULD NOT WRITE FAILURE LOG: $_"
  }
}

$OutputDirectory = [IO.Path]::GetFullPath($OutputDirectory)
if (Test-Path -LiteralPath $OutputDirectory) {
  throw "OutputDirectory must not already exist: $OutputDirectory"
}
New-Item -ItemType Directory -Path $OutputDirectory | Out-Null
$containerName = 'openpit-buildtools-diagnostic-' + [guid]::NewGuid().ToString('N')
$containerDirectory = 'C:\openpit-image-diagnostics'
$analyticChannel = 'Microsoft-Windows-Hyper-V-Compute-Analytic'
$analyticWasEnabled = $null
$containerCreated = $false
$script:resultCode = 0
$script:diagnosticCode = 0
$script:failureCount = 0

try {
  $dockerfile = Join-Path $PSScriptRoot 'env\docker\windows-binary\Dockerfile'
  $lines = @(Get-Content -LiteralPath $dockerfile)
  $runIndex = -1
  for ($i = 0; $i -lt $lines.Count; $i++) {
    if ($lines[$i] -match '^RUN\s+') { $runIndex = $i; break }
  }
  if ($runIndex -lt 0 -or $lines[0] -ne '# escape=`') {
    throw 'expected the first Dockerfile RUN to use the documented cmd shell and backtick escape'
  }
  $shells = @($lines[0..$runIndex] | Where-Object { $_ -match '^SHELL\s' })
  if ($shells.Count -ne 1 -or $shells[0] -ne 'SHELL ["cmd", "/S", "/C"]') {
    throw 'expected exactly one cmd SHELL before the first Dockerfile RUN'
  }
  $parts = @()
  for ($i = $runIndex; $i -lt $lines.Count; $i++) {
    $line = $lines[$i].Trim()
    if ($i -eq $runIndex) { $line = $line -replace '^RUN\s+', '' }
    $continued = $line.EndsWith('`')
    if ($continued) { $line = $line.Substring(0, $line.Length - 1).TrimEnd() }
    $parts += $line
    if (-not $continued) { break }
  }
  $installerCommand = $parts -join ' '
  if ($continued -or
      $installerCommand -notmatch '^curl\.exe -SL --output vs_buildtools\.exe https://aka\.ms/vs/17/release/vs_buildtools\.exe\s+&&\s+\(start /w vs_buildtools\.exe\s' -or
      $installerCommand -notmatch '--add Microsoft\.VisualStudio\.Workload\.VCTools\s' -or
      $installerCommand -notmatch '&& del /q vs_buildtools\.exe$') {
    throw 'unexpected first Dockerfile RUN: expected the complete Visual Studio Build Tools installation command'
  }
  $installerCommand | Set-Content -Encoding ASCII (Join-Path $OutputDirectory 'installer-command.cmd')
  @'
param([Parameter(Mandatory)][ValidateSet('before', 'after')][string]$Phase)
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
Write-Host "Installer TEMP: $env:TEMP"
Get-PSDrive -PSProvider FileSystem | Select-Object Name, Root, Used, Free |
  ConvertTo-Json | Set-Content -Encoding UTF8 "$PSScriptRoot\container-drives-$Phase.json"
if ($Phase -eq 'after') {
  $logs = @(Get-ChildItem -LiteralPath $env:TEMP -Filter 'dd_*.log' -File)
  if ($logs.Count -eq 0) { throw "no Visual Studio installer dd_*.log files found in $env:TEMP" }
  ConvertTo-Json -InputObject @($logs.FullName) |
    Set-Content -Encoding UTF8 "$PSScriptRoot\installer-logs.json"
}
'@ | Set-Content -Encoding ASCII (Join-Path $OutputDirectory 'collect-container.ps1')
  @'
@echo off
powershell.exe -NoLogo -NoProfile -ExecutionPolicy Bypass -File C:\openpit-image-diagnostics\collect-container.ps1 before
if errorlevel 1 exit /b %errorlevel%
cmd.exe /S /C C:\openpit-image-diagnostics\installer-command.cmd
set "INSTALLER_RUN_EXIT=%ERRORLEVEL%"
echo Dockerfile RUN command exit code: %INSTALLER_RUN_EXIT%
echo %INSTALLER_RUN_EXIT%>C:\openpit-image-diagnostics\installer-run-exit-code.txt
powershell.exe -NoLogo -NoProfile -ExecutionPolicy Bypass -File C:\openpit-image-diagnostics\collect-container.ps1 after
set "COLLECT_EXIT=%ERRORLEVEL%"
if not "%INSTALLER_RUN_EXIT%"=="0" exit /b %INSTALLER_RUN_EXIT%
exit /b %COLLECT_EXIT%
'@ | Set-Content -Encoding ASCII (Join-Path $OutputDirectory 'run-installer.cmd')

  $containerName | Set-Content -Encoding ASCII (Join-Path $OutputDirectory 'container-name.txt')
  $BaseImage | Set-Content -Encoding ASCII (Join-Path $OutputDirectory 'base-image.txt')
  Save-DriveSpace 'before'
  Invoke-Native 'docker.exe' @('version') 'docker-version-before' | Out-Null
  Invoke-Native 'docker.exe' @('info') 'docker-info-before' | Out-Null
  $channel = [xml](Invoke-Native 'wevtutil.exe' @('gl', $analyticChannel, '/f:xml') 'hcs-analytic-state')
  $enabled = $channel.DocumentElement.GetAttribute('enabled')
  if ($enabled -notin @('true', 'false')) { throw 'HCS analytic channel did not report an enabled state' }
  $analyticWasEnabled = $enabled -eq 'true'
  Invoke-Native 'wevtutil.exe' @('sl', $analyticChannel, '/e:true', '/q:true') 'hcs-analytic-enable' | Out-Null
  Invoke-Native 'docker.exe' @(
    'create', '--name', $containerName, '--isolation=process', '--memory', '4GB',
    '--entrypoint', 'cmd.exe', $BaseImage, '/S', '/C', "$containerDirectory\run-installer.cmd"
  ) 'docker-create' | Out-Null
  $containerCreated = $true
  $inputDirectory = Join-Path $OutputDirectory 'container-input'
  New-Item -ItemType Directory -Path $inputDirectory | Out-Null
  foreach ($file in @('installer-command.cmd', 'collect-container.ps1', 'run-installer.cmd')) {
    Copy-Item -LiteralPath (Join-Path $OutputDirectory $file) -Destination $inputDirectory
  }
  Invoke-Native 'docker.exe' @('cp', $inputDirectory, "${containerName}:$containerDirectory") 'copy-container-input' | Out-Null
  try {
    Invoke-Native 'docker.exe' @('start', '--attach', $containerName) 'installer-run' | Out-Null
  } catch {
    Save-Failure $_ -Primary
  }
  try {
    $state = Invoke-Native 'docker.exe' @('inspect', '--format', '{{json .State}}', $containerName) 'container-state'
    $state = $state | ConvertFrom-Json
    if ($state.Status -ne 'exited') { throw "diagnostic container state is '$($state.Status)', expected 'exited'" }
    if ($state.ExitCode -ne 0 -and $script:resultCode -eq 0) { $script:resultCode = [int]$state.ExitCode }
    $containerOutput = Join-Path $OutputDirectory 'container-output'
    Invoke-Native 'docker.exe' @('cp', "${containerName}:$containerDirectory", $containerOutput) 'copy-container-output' | Out-Null
    $installerExit = [int](Get-Content -Raw -LiteralPath (Join-Path $containerOutput 'installer-run-exit-code.txt'))
    Write-Host "Dockerfile RUN command exit code: $installerExit"
    if ($installerExit -ne 0 -and $script:resultCode -eq 0) { $script:resultCode = $installerExit }
    $logs = @(Get-Content -Raw -LiteralPath (Join-Path $containerOutput 'installer-logs.json') | ConvertFrom-Json)
    if ($logs.Count -eq 0) { throw 'installer log manifest is empty' }
    $logDirectory = Join-Path $OutputDirectory 'installer-logs'
    New-Item -ItemType Directory -Path $logDirectory | Out-Null
    foreach ($log in $logs) {
      $name = Split-Path -Leaf $log
      Invoke-Native 'docker.exe' @('cp', "${containerName}:$log", (Join-Path $logDirectory $name)) "copy-$name" | Out-Null
    }
  } catch {
    Save-Failure $_
  }
  if ($script:resultCode -eq 0 -and $script:failureCount -eq 0) {
    Invoke-Native 'docker.exe' @('commit', $containerName, "${containerName}:diagnostic") 'docker-commit' | Out-Null
  } else {
    Write-Host 'Skipping commit because installation or required log collection failed.'
  }
} catch {
  Save-Failure $_ -Primary
} finally {
  foreach ($phase in @('drive-space', 'docker-version', 'docker-info')) {
    try {
      if ($phase -eq 'drive-space') { Save-DriveSpace 'after' }
      elseif ($phase -eq 'docker-version') { Invoke-Native 'docker.exe' @('version') 'docker-version-after' | Out-Null }
      else { Invoke-Native 'docker.exe' @('info') 'docker-info-after' | Out-Null }
    } catch { Save-Failure $_ }
  }
  foreach ($channelName in @('Microsoft-Windows-Hyper-V-Compute-Admin', 'Microsoft-Windows-Hyper-V-Compute-Operational', $analyticChannel)) {
    try {
      Invoke-Native 'wevtutil.exe' @('epl', $channelName, (Join-Path $OutputDirectory "$channelName.evtx")) "export-$channelName" | Out-Null
    } catch { Save-Failure $_ }
  }
  try {
    Invoke-Native 'wevtutil.exe' @('epl', 'Application', (Join-Path $OutputDirectory 'docker-application.evtx'), "/q:*[System[Provider[@Name='docker']]]") 'export-docker-events' | Out-Null
  } catch { Save-Failure $_ }
  if ($null -ne $analyticWasEnabled) {
    try {
      Invoke-Native 'wevtutil.exe' @('sl', $analyticChannel, "/e:$($analyticWasEnabled.ToString().ToLowerInvariant())", '/q:true') 'hcs-analytic-restore' | Out-Null
    } catch { Save-Failure $_ }
  }
  if ($containerCreated) { Write-Host "Container retained: $containerName" }
}

if ($script:resultCode -eq 0 -and $script:diagnosticCode -ne 0) { $script:resultCode = $script:diagnosticCode }
Write-Host "Diagnostic exit code: $script:resultCode; evidence: $OutputDirectory"
exit $script:resultCode
