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
#include "spot_loadtest/env/env.hpp"
#include "spot_loadtest/generator/event.hpp"
#include "spot_loadtest/measurement/snapshot.hpp"

#include <cstddef>
#include <ostream>
#include <string>

// Writes a plain-text load-test report to an std::ostream (typically
// std::cout).
//
// Block order: Headline, Environment, Workload, Trajectory, Distribution,
// Diagnostics, Disclaimer.
//
// A run is INVALID when it hit dispatch backpressure (QueueLimit) or produced a
// zero anti-DCE checksum on a non-empty run. For an invalid run the caller uses
// WriteInvalid, which prints a prominent banner and the non-latency diagnostics
// and OMITS the Headline, Trajectory, and Distribution percentile blocks.
//
// Stdout/stderr separation: this package writes only to the `out` argument
// (stdout). Progress noise belongs on stderr and never passes through here.

namespace spot_loadtest::reporter {

// Prints the full post-run report to `out`. Use ONLY for a VALID run.
void Write(std::ostream &out, const env::Env &e, const config::Config &cfg,
           const std::string &configFlag, const measurement::Snapshot &snap,
           std::size_t actualSubmitterThreads,
           const generator::StreamStats &streamStats);

// Prints the INVALID-RUN report for a run that is not a valid measurement (for
// ANY invalid reason: dispatch backpressure or a zero anti-DCE checksum). Omits
// the Headline and all latency-distribution percentile blocks; keeps the
// non-latency diagnostics and a prominent invalid-run banner.
void WriteInvalid(std::ostream &out, const env::Env &e,
                  const config::Config &cfg, const std::string &configFlag,
                  const measurement::Snapshot &snap,
                  std::size_t actualSubmitterThreads,
                  const generator::StreamStats &streamStats);

} // namespace spot_loadtest::reporter
