# escape=`
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

FROM mcr.microsoft.com/dotnet/framework/runtime:4.8.1-windowsservercore-ltsc2022

# Visual Studio Build Tools requires cmd.exe for its bootstrapper. See
# https://learn.microsoft.com/visualstudio/install/build-tools-container
SHELL ["cmd", "/S", "/C"]

RUN curl.exe -SL --output vs_buildtools.exe https://aka.ms/vs/17/release/vs_buildtools.exe `
    && (start /w vs_buildtools.exe --quiet --wait --norestart --nocache `
        --installPath "%ProgramFiles(x86)%\Microsoft Visual Studio\2022\BuildTools" `
        --add Microsoft.VisualStudio.Workload.VCTools `
        --includeRecommended `
        --remove Microsoft.VisualStudio.Component.Windows10SDK.10240 `
        --remove Microsoft.VisualStudio.Component.Windows10SDK.10586 `
        --remove Microsoft.VisualStudio.Component.Windows10SDK.14393 `
        --remove Microsoft.VisualStudio.Component.Windows81SDK `
        || IF "%ERRORLEVEL%"=="3010" EXIT 0) `
    && del /q vs_buildtools.exe

SHELL ["C:\\Windows\\System32\\WindowsPowerShell\\v1.0\\powershell.exe", "-NoLogo", "-NoProfile", "-Command", "$ErrorActionPreference = 'Stop'; $ProgressPreference = 'SilentlyContinue';"]

RUN Invoke-WebRequest 'https://github.com/Kitware/CMake/releases/download/v3.28.6/cmake-3.28.6-windows-x86_64.zip' -OutFile C:\cmake.zip; `
    Expand-Archive C:\cmake.zip -DestinationPath C:\; `
    Move-Item C:\cmake-3.28.6-windows-x86_64 C:\cmake; `
    Remove-Item C:\cmake.zip

RUN Invoke-WebRequest 'https://github.com/git-for-windows/git/releases/download/v2.45.2.windows.1/Git-2.45.2-64-bit.exe' -OutFile C:\git-installer.exe; `
    $installer = Start-Process C:\git-installer.exe -Wait -PassThru -ArgumentList '/VERYSILENT', '/NORESTART', '/SP-', '/SUPPRESSMSGBOXES', '/DIR=C:\Git'; `
    if ($installer.ExitCode -ne 0) { throw ('Git installer failed with exit code ' + $installer.ExitCode) }; `
    Remove-Item C:\git-installer.exe

RUN Invoke-WebRequest 'https://www.python.org/ftp/python/3.10.11/python-3.10.11-amd64.exe' -OutFile C:\python-installer.exe; `
    $installer = Start-Process C:\python-installer.exe -Wait -PassThru -ArgumentList '/quiet', 'InstallAllUsers=1', 'TargetDir=C:\Python310', 'Include_test=0', 'Include_doc=0', 'Include_launcher=0'; `
    if ($installer.ExitCode -ne 0) { throw ('Python installer failed with exit code ' + $installer.ExitCode) }; `
    Remove-Item C:\python-installer.exe

RUN Invoke-WebRequest 'https://go.dev/dl/go1.22.3.windows-amd64.zip' -OutFile C:\go.zip; `
    Expand-Archive C:\go.zip -DestinationPath C:\; `
    Remove-Item C:\go.zip

ENV PATH="C:\cmake\bin;C:\Git\cmd;C:\Python310;C:\Python310\Scripts;C:\go\bin;C:\Windows\System32\WindowsPowerShell\v1.0;C:\Windows\System32;C:\Windows"

RUN & C:\Python310\python.exe -m pip install --disable-pip-version-check --no-cache-dir --upgrade pip pytest

WORKDIR C:\e2e

COPY e2e/scripts/windows-binary.ps1 C:\e2e\run-release-e2e.ps1
COPY e2e/scripts/windows-entrypoint.cmd C:\e2e\entrypoint.cmd
COPY e2e/clients/cpp C:\e2e\cpp-consumer
COPY e2e/clients/go C:\e2e\go-consumer
COPY bindings/python/pyproject.toml C:\e2e\pyproject.toml
COPY bindings/python/tests/conftest.py C:\e2e\tests\conftest.py
COPY bindings/python/tests/integration C:\e2e\tests\integration
COPY examples/cpp C:\e2e\cpp-examples
COPY examples/go C:\e2e\go-examples
COPY examples/python C:\e2e\python-examples
COPY examples/tables C:\e2e\tables

ENTRYPOINT ["cmd", "/S", "/C", "C:\\e2e\\entrypoint.cmd"]
