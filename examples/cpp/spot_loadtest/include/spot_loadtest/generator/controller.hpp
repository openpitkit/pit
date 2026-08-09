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

#include "spot_loadtest/generator/rng.hpp"

#include <cstdint>

// The offline reject-rate controller.
//
// Without engine feedback it calibrates how often the generator emits an order
// the shadow model predicts will be rejected, converging the predicted reject
// rate to the configured target. Error-driven (integral) control: force the
// next eligible order iff the running rate is below target. Eligibility is
// gated by the cohort's reject propensity.

namespace spot_loadtest::generator {

class RejectController {
public:
  explicit RejectController(double target) : m_target(target) {}

  // Whether the next eligible order should be forced into a predicted reject.
  [[nodiscard]] bool ShouldForce(Rng &g, double propensity) {
    if (m_target <= 0) {
      return false;
    }
    if (!g.Bernoulli(propensity)) {
      return false;
    }
    return Rate() < m_target;
  }

  // Records the realised order-check outcome so the running rate tracks what
  // the shadow model actually predicted.
  void Observe(bool accepted) {
    ++m_checks;
    if (!accepted) {
      ++m_rejects;
    }
  }

  [[nodiscard]] double Rate() const {
    if (m_checks == 0) {
      return 0.0;
    }
    return static_cast<double>(m_rejects) / static_cast<double>(m_checks);
  }

private:
  double m_target;
  std::uint64_t m_checks = 0;
  std::uint64_t m_rejects = 0;
};

} // namespace spot_loadtest::generator
