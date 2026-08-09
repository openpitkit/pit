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

#include "spot_loadtest/measurement/sink.hpp"

#include <atomic>
#include <functional>
#include <memory>

// A race-safe bridge between the driver's internal sink and the progress
// reporter.
//
// Holds an atomic pointer to the live-counter accessor; Run stores the real
// accessor before starting any thread, so the progress reporter can call
// Counters() at any time. Before the accessor is stored, Counters() returns a
// zero-value LiveCounters rather than crashing.

namespace spot_loadtest::driver {

class LiveSource {
public:
  LiveSource() = default;

  // Called by Run exactly once, before threads start.
  void Store(std::function<measurement::LiveCounters()> fn) {
    auto holder = std::make_shared<std::function<measurement::LiveCounters()>>(
        std::move(fn));
    std::atomic_store(&m_fn, holder);
  }

  // The current live counters; zero if Run has not yet stored the accessor.
  [[nodiscard]] measurement::LiveCounters Counters() const {
    auto holder = std::atomic_load(&m_fn);
    if (!holder) {
      return measurement::LiveCounters{};
    }
    return (*holder)();
  }

private:
  std::shared_ptr<std::function<measurement::LiveCounters()>> m_fn;
};

} // namespace spot_loadtest::driver
