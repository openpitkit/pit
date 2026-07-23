# Release e2e

Docker-based end-to-end checks that verify the **published** OpenPit artifacts
work for a real downstream consumer. For a given version, each scenario pulls
the released artifact from its public registry or release asset set, builds a
minimal consumer and the workspace examples against it, and runs their tests -
exactly what an SDK user sees when they add the dependency.

## Layout

- `run.sh` — orchestrator: builds one image per scenario and runs its checks.
- `run-windows.ps1` — Windows-container orchestrator for the Windows binary
  surfaces.
- `env/docker/<target>/Dockerfile` — per-target build environment.
- `scripts/<target>.sh` and `scripts/windows-binary.ps1` — in-container
  runners (fetch the release, build, test).
- `clients/<lang>` — the minimal smoke consumer for each language.

## How to run

Run against an already-published version.

### Linux

Requires Docker with `buildx`; the Linux scenarios cross-build for amd64 and
arm64.

```sh
just test-release-e2e 0.4.0
# or directly:
./e2e/run.sh 0.4.0
```

The Linux suite builds and checks these scenarios, then prints a pass/fail
summary: `rust-amd64`, `rust-arm64`, `python-wheel-amd64`,
`python-wheel-arm64`, `python-source-arm64`, `go-amd64`, `cpp-amd64`,
`cpp-vcpkg-amd64`, `js-amd64`.

### Windows

Requires a Windows Docker engine in Windows-container mode. The Windows
container image is based on Windows Server 2022, so use a compatible host such
as Windows Server 2022 or the `windows-2022` GitHub runner. Docker Desktop must
be switched to Windows containers before running the suite.

```powershell
just test-release-e2e 0.4.0
# or directly:
.\e2e\run-windows.ps1 0.4.0
```

The Windows suite builds and checks `python-wheel-windows-amd64`,
`go-windows-amd64`, `cpp-windows-amd64`, and
`cpp-vcpkg-windows-amd64`. The C++ scenarios also build and run the public C++
examples. The script rejects a Linux Docker engine, since Windows containers
cannot run there.

## How to run the tests

There is no separate unit-test step: each scenario *is* the test. A scenario
fails (and the run exits non-zero) if the released artifact cannot be fetched,
the consumer or an example fails to build, or any of their tests fail.
