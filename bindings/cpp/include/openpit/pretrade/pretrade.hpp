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

// Aggregate header for the pre-trade module: lifecycle results, callback-scoped
// collectors and contexts, policy adapters, pre-trade lock, built-in policy
// configurations, and custom-policy authoring glue.

#include "openpit/pretrade/adapters.hpp"
#include "openpit/pretrade/callbacks.hpp"
#include "openpit/pretrade/context.hpp"
#include "openpit/pretrade/custom_policy.hpp"
#include "openpit/pretrade/dry_run_report.hpp"
#include "openpit/pretrade/policies.hpp"
#include "openpit/pretrade/pre_trade_lock.hpp"
#include "openpit/pretrade/request.hpp"
#include "openpit/pretrade/reservation.hpp"
#include "openpit/pretrade/start_result.hpp"
