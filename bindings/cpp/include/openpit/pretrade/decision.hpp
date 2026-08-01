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

#include "openpit/reject/reject.hpp"

#include <utility>
#include <vector>

namespace openpit::pretrade {

using RejectScope = ::openpit::reject::RejectScope;
using RejectCode = ::openpit::reject::RejectCode;
using Reject = ::openpit::reject::Reject;
using ::openpit::reject::IsEvaluationFailure;

/// Rejections produced by one custom-policy callback.
struct PolicyDecision {
  std::vector<Reject> rejects;

  [[nodiscard]] bool IsRejected() const noexcept { return !rejects.empty(); }

  void Push(Reject reject) { rejects.push_back(std::move(reject)); }
};

}  // namespace openpit::pretrade
