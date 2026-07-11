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

use std::cell::Cell;

use crate::time::Instant;

thread_local! {
    static START_PRE_TRADE_NOW: Cell<Option<Instant>> = const { Cell::new(None) };
}

struct StartPreTradeNowGuard<'a> {
    slot: &'a Cell<Option<Instant>>,
    previous: Option<Instant>,
}

impl Drop for StartPreTradeNowGuard<'_> {
    fn drop(&mut self) {
        self.slot.set(self.previous);
    }
}

pub(crate) fn with_start_pre_trade_now<T>(now: Instant, f: impl FnOnce() -> T) -> T {
    START_PRE_TRADE_NOW.with(|slot| {
        let previous = slot.replace(Some(now));
        let _guard = StartPreTradeNowGuard { slot, previous };
        f()
    })
}

pub(crate) fn start_pre_trade_now() -> Instant {
    START_PRE_TRADE_NOW
        .with(|slot| slot.get())
        .unwrap_or_else(Instant::now)
}
