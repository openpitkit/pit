# Release e2e

Docker-based end-to-end checks that verify the **published** OpenPit artifacts
work for a real downstream consumer. For a given version, each scenario pulls
the released artifact from its public registry (crates.io, PyPI, the Go module
proxy), builds a minimal consumer and the workspace examples against it, and
runs their tests — exactly what an SDK user sees when they add the dependency.

## Layout

- `run.sh` — orchestrator: builds one image per scenario and runs its checks.
- `env/docker/<target>/Dockerfile` — per-target build environment.
- `scripts/<target>.sh` — in-container runner (fetch the release, build, test).
- `clients/<lang>` — the minimal smoke consumer for each language.

## How to run

Requires Docker with `buildx` (the scenarios cross-build for amd64 and arm64).
Run against an already-published version:

```sh
just test-release-e2e 0.4.0
# or directly:
./e2e/run.sh 0.4.0
```

The suite builds and checks these scenarios, then prints a pass/fail summary:
`rust-amd64`, `rust-arm64`, `python-wheel-amd64`, `python-wheel-arm64`,
`python-source-arm64`, `go-amd64`.

## How to run the tests

There is no separate unit-test step: each scenario *is* the test. A scenario
fails (and the run exits non-zero) if the released artifact cannot be fetched,
the consumer or an example fails to build, or any of their tests fail.
