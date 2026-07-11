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

// Umbrella header for the OpenPit C++ binding foundation. Includes the core
// infrastructure and the minimal engine slice. Policy adapters live in the
// separate `openpit/adapters.hpp` header.

#include "openpit/account_adjustment.hpp"
#include "openpit/account_id.hpp"
#include "openpit/accounts.hpp"
#include "openpit/async_engine.hpp"
#include "openpit/bytes.hpp"
#include "openpit/engine.hpp"
#include "openpit/error.hpp"
#include "openpit/fwd.hpp"
#include "openpit/marketdata.hpp"
#include "openpit/model.hpp"
#include "openpit/param.hpp"
#include "openpit/pretrade/pretrade.hpp"
#include "openpit/reject.hpp"
#include "openpit/runtime.hpp"
#include "openpit/string.hpp"
#include "openpit/tx.hpp"
