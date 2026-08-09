<!--
Copyright The Pit Project Owners. All rights reserved.
SPDX-License-Identifier: Apache-2.0
-->

# openpit Spot-Limit Load Test (C++)

C++17 load harness for the openpit spot-limit funds policy. It reports
open-loop order-check and settlement latency through the public C++ async API.

Sample output: [sample_report.txt](sample_report.txt).

## Requirements

- C++17 compiler and CMake 3.21+
- Release-built openpit native core

## Build and run

<!-- Test mirror: examples/cpp/spot_loadtest/test/driver_test.cpp
     Driver.DocBackingBaselineRecipe -->

Run from the repository root:

```sh
cargo build --release

# Linux: swap .dylib for .so
cmake -S examples/cpp/spot_loadtest -B build -DCMAKE_BUILD_TYPE=Release \
  -DOPENPIT_RUNTIME_LIBRARY=$(pwd)/target/release/libopenpit_ffi.dylib

cmake --build build
./build/spot_loadtest \
  --config examples/cpp/spot_loadtest/configs/baseline.ini
```

The harness rejects a debug-built core unless `--allow-debug-core` is passed.
The runtime directory may need to be exposed through `DYLD_LIBRARY_PATH` on
macOS or `LD_LIBRARY_PATH` on Linux.

## CLI

| Flag | Description |
| --- | --- |
| `--config PATH` | Load the required INI scenario |
| `--validate-config` | Validate the scenario and exit |
| `--allow-debug-core` | Allow a debug core for development |
| `--progress=false` | Disable live progress |

## Configuration and output

[`configs/baseline.ini`](configs/baseline.ini) is the reference scenario. Its
supported settings are documented inline.

The report is written to stdout and progress to stderr. For async queueing,
ordering, retry, and shutdown contracts, see the
[Async Engine wiki](https://github.com/openpitkit/pit/wiki/Async-Engine).
