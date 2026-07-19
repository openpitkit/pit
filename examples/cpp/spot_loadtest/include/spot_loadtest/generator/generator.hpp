// Copyright The Pit Project Owners. All rights reserved.
// SPDX-License-Identifier: Apache-2.0
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.
//
// Please see https://openpit.dev and the OWNERS file for details.

#pragma once

#include "spot_loadtest/config/config.hpp"
#include "spot_loadtest/generator/event.hpp"

#include <memory>

// Builds a seeded, deterministic, pre-materialised stream of abstract load-test
// events for the spot-limit harness.
//
// Mirror of: examples/go/spot_loadtest/internal/generator/generator.go
//
// It is an INDEPENDENT reimplementation of the spot-funds arithmetic — it
// never imports the engine — which is what makes the per-op oracle
// non-circular. The generator maintains a shadow ledger and a position
// lifecycle, and for every order it PREDICTS the engine's accept/reject
// decision and resulting balances.

namespace spot_loadtest::generator {

// Builds the full deterministic event stream for cfg. The same cfg (including
// run.seed) always yields a byte-identical serialised stream and identical
// predictions. Throws std::runtime_error on a generation failure.
[[nodiscard]] std::unique_ptr<Stream> Generate(const config::Config &cfg);

} // namespace spot_loadtest::generator
