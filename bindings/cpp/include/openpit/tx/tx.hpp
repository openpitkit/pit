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

#include "openpit/detail/callback_error.hpp"
#include "openpit/error.hpp"

#include <openpit.h>

#include <functional>
#include <utility>

// Callback-scoped reversible mutation collector used by custom policies.

namespace openpit::tx {

class Mutations {
 public:
  Mutations(const Mutations&) = delete;
  Mutations& operator=(const Mutations&) = delete;
  Mutations(Mutations&&) = delete;
  Mutations& operator=(Mutations&&) = delete;

  // Applies tentative state before registering one commit/rollback pair.
  // Ordinary pre-trade finalization calls one callback. A drop-copy operation
  // registers the same pair and finalizes it the same way, from `Commit()` or
  // `Rollback()` on the operation. A rollback also runs for pairs whose commit
  // was not reached, because their tentative state was already applied. Their
  // storage is released after execution or collector drop.
  //
  // Neither callback has the right to fail: by the time a finalizer runs the
  // decision is already made and the state it finalizes was applied eagerly, so
  // there is nothing left to compensate. A callback that throws anyway never
  // stops the batch, and it is reported on two independent channels. Its
  // original exception is rethrown from the finalizing call - `Commit()` or
  // `Rollback()` on the reservation or the drop-copy operation - once every
  // remaining callback of the batch has run. Separately, and never as a failure
  // of that void call, the engine arms its kill switch, because its own
  // bookkeeping is now in an unknown state: a mutation pushed here belongs to a
  // custom policy whose state reach the engine cannot bound, so EVERY account
  // is blocked, not only the order's own. Nothing reports that block to the
  // finalizing caller - it surfaces when the next pre-trade call is rejected
  // with `pretrade::RejectCode::SystemUnavailable`, and an operator clears it
  // with `accounts::Accounts::UnblockAll()`, which leaves accounts and account
  // groups blocked individually intact. The implicit rollback of an unresolved
  // reservation or operation arms the same kill switch, but a destructor cannot
  // rethrow, so there the block is the only channel left.
  template <typename Commit, typename Rollback>
  void Push(Commit&& commit, Rollback&& rollback) {
    auto* callbacks =
        new Callbacks(std::function<void()>(std::forward<Commit>(commit)),
                      std::function<void()>(std::forward<Rollback>(rollback)));
    OpenPitSharedString* error = nullptr;
    if (!openpit_mutations_push(m_native, &CommitTrampoline,
                                &RollbackTrampoline, callbacks, &FreeTrampoline,
                                &error)) {
      delete callbacks;
      ::openpit::detail::ThrowFromSharedString(error,
                                               "openpit_mutations_push failed");
    }
  }

 private:
  friend class ::openpit::detail::NativeAccess;

  explicit Mutations(OpenPitMutations* native) noexcept : m_native(native) {}

  [[nodiscard]] OpenPitMutations* Native() const noexcept { return m_native; }

  struct Callbacks {
    Callbacks(std::function<void()> onCommit, std::function<void()> onRollback)
        : commit(std::move(onCommit)), rollback(std::move(onRollback)) {}

    std::function<void()> commit;
    std::function<void()> rollback;
  };

  static bool CommitTrampoline(void* userData,
                               OpenPitSharedString** outError) noexcept {
    try {
      static_cast<Callbacks*>(userData)->commit();
      return true;
    } catch (...) {
      ::openpit::detail::CaptureCurrentCallbackException(outError);
      return false;
    }
  }

  static bool RollbackTrampoline(void* userData,
                                 OpenPitSharedString** outError) noexcept {
    try {
      static_cast<Callbacks*>(userData)->rollback();
      return true;
    } catch (...) {
      ::openpit::detail::CaptureCurrentCallbackException(outError);
      return false;
    }
  }

  static void FreeTrampoline(void* userData) noexcept {
    delete static_cast<Callbacks*>(userData);
  }

  OpenPitMutations* m_native = nullptr;
};

}  // namespace openpit::tx
