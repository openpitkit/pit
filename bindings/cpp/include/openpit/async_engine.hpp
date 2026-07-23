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

// Aggregate header for the async-engine module.
//
// The async engine is a C++ concurrency facade implementing account-scoped
// serialization and bounded queues. The SDK core never spawns OS threads (see
// the project "Threading Contract").
// It provides `Sharded`/`Dynamic` dispatch strategies, futures, graceful/hard
// stop, and an optional observer on top of `std::thread` over a generic driver.
// See
// `openpit/asyncengine/engine.hpp` for the threading contract and the driver
// seam.
//
// `openpit/asyncengine/typed.hpp` layers the concrete typed surface on top: an
// `openpit::Engine`-backed driver and a `TypedAsyncEngine` exposing the named
// pre-trade pipeline operations (StartPreTrade / ExecutePreTrade /
// ApplyAccountAdjustment), while leaving the generic driver seam intact.

#include "openpit/asyncengine/engine.hpp"
#include "openpit/asyncengine/future.hpp"
#include "openpit/asyncengine/observer.hpp"
#include "openpit/asyncengine/typed.hpp"
