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
  explicit Mutations(OpenPitMutations* native) noexcept : m_native(native) {}

  Mutations(const Mutations&) = delete;
  Mutations& operator=(const Mutations&) = delete;
  Mutations(Mutations&&) = delete;
  Mutations& operator=(Mutations&&) = delete;

  // Registers one commit/rollback pair. Exactly one callback runs when the
  // request is finalized. The callbacks may capture ordinary C++ state by
  // value; their storage is released after execution or collector drop.
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

  [[nodiscard]] OpenPitMutations* Native() const noexcept { return m_native; }

 private:
  struct Callbacks {
    Callbacks(std::function<void()> onCommit, std::function<void()> onRollback)
        : commit(std::move(onCommit)), rollback(std::move(onRollback)) {}

    std::function<void()> commit;
    std::function<void()> rollback;
  };

  static void CommitTrampoline(void* userData) noexcept {
    try {
      static_cast<Callbacks*>(userData)->commit();
    } catch (...) {
      ::openpit::detail::CaptureCurrentCallbackException();
    }
  }

  static void RollbackTrampoline(void* userData) noexcept {
    try {
      static_cast<Callbacks*>(userData)->rollback();
    } catch (...) {
      ::openpit::detail::CaptureCurrentCallbackException();
    }
  }

  static void FreeTrampoline(void* userData) noexcept {
    delete static_cast<Callbacks*>(userData);
  }

  OpenPitMutations* m_native = nullptr;
};

}  // namespace openpit::tx
