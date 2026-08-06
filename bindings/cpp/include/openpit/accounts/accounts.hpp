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

#include "openpit/detail/handle.hpp"
#include "openpit/error.hpp"
#include "openpit/param/account_id.hpp"
#include "openpit/param/asset.hpp"
#include "openpit/pretrade/decision.hpp"
#include "openpit/string.hpp"

#include <openpit.h>

#include <cstdint>
#include <optional>
#include <string>
#include <string_view>
#include <utility>
#include <vector>

// Account administration surface.
//
// `Accounts` is the engine.accounts() admin handle: it manages account-group
// membership and records or lifts pre-trade blocks, addressable both by
// individual `param::AccountId` and by an account-group predicate
// (`param::AccountGroupId`, i.e. membership in that group), over the one
// unified blocked-accounts list the engine owns. It also clears the engine-wide
// block the engine raises on its own. Account blocking is owned by the engine;
// this handle only forwards to it.
//
// Block/unblock by id and clearing the engine-wide block are infallible.
// Reason-replacement and every group-scoped operation can fail with an
// expected, structured outcome - returned as a `std::optional` value, never
// thrown. SDK boundary failures throw `openpit::Error`.
//
// `AccountControl` is the engine-provided handle a custom callback uses to
// record a kill-switch block against the account bound to its context; it is
// move-only RAII and valid only within the pre-trade transaction that produced
// it. `AccountBlock` is the value type that block carries.

namespace openpit::accounts {

// A kill-switch block record. Produced by policy callbacks and read back from
// `Engine::ApplyExecutionReport`; also the payload recorded through
// `AccountControl::Block`. `userData` is an opaque caller token the SDK never
// inspects (zero means unset).
struct AccountBlock {
  std::string policy;
  std::string reason;
  std::string details;
  std::uintptr_t userData = 0;
  ::openpit::pretrade::RejectCode code = ::openpit::pretrade::RejectCode::Other;

  AccountBlock() = default;

  AccountBlock(::openpit::pretrade::RejectCode blockCode,
               std::string policyName, std::string blockReason,
               std::string blockDetails)
      : policy(std::move(policyName)),
        reason(std::move(blockReason)),
        details(std::move(blockDetails)),
        code(blockCode) {}

 private:
  friend class ::openpit::detail::NativeAccess;

  [[nodiscard]] static AccountBlock FromRaw(
      const OpenPitPretradeAccountBlock& raw) {
    AccountBlock out;
    out.policy =
        ::openpit::detail::FromNative<::openpit::StringView>(raw.policy)
            .ToString();
    out.reason =
        ::openpit::detail::FromNative<::openpit::StringView>(raw.reason)
            .ToString();
    out.details =
        ::openpit::detail::FromNative<::openpit::StringView>(raw.details)
            .ToString();
    out.userData = reinterpret_cast<std::uintptr_t>(raw.user_data);
    out.code = static_cast<::openpit::pretrade::RejectCode>(raw.code);
    return out;
  }

  // Builds a C account-block record whose string views borrow this object's
  // strings; valid only while this `AccountBlock` is alive and unchanged.
  [[nodiscard]] OpenPitPretradeAccountBlock Native() const noexcept {
    OpenPitPretradeAccountBlock raw{};
    raw.policy = ::openpit::detail::MakeStringView(policy);
    raw.reason = ::openpit::detail::MakeStringView(reason);
    raw.details = ::openpit::detail::MakeStringView(details);
    raw.user_data = reinterpret_cast<void*>(userData);
    raw.code = static_cast<OpenPitPretradeRejectCode>(
        static_cast<std::uint16_t>(code));
    return raw;
  }
};

// Account block inserted for an account selected by the engine.
struct AccountBlockOutcome {
  ::openpit::param::AccountId accountId;
  AccountBlock block;
};

namespace detail {

using RawAccountControl = ::OpenPitAccountControl;
using RawEngine = ::OpenPitEngine;

struct AccountControlDeleter {
  void operator()(OpenPitAccountControl* handle) const noexcept {
    openpit_destroy_account_control(handle);
  }
};

struct AccountBlockErrorDeleter {
  void operator()(OpenPitAccountBlockError* handle) const noexcept {
    openpit_destroy_account_block_error(handle);
  }
};

struct AccountGroupErrorDeleter {
  void operator()(OpenPitAccountGroupError* handle) const noexcept {
    openpit_destroy_account_group_error(handle);
  }
};

}  // namespace detail

// Engine-provided handle that records kill-switch blocks against the account
// bound to a callback context.
//
// Move-only RAII; destruction releases the handle. It is valid to use only
// within the pre-trade transaction of the request it belongs to - from the
// callback that produced it through the commit or rollback of that request's
// reservation, so it may be captured for deferred blocking. Recording a block
// through it afterwards is undefined.
class AccountControl {
 public:
  AccountControl() = default;

  [[nodiscard]] explicit operator bool() const noexcept {
    return static_cast<bool>(m_handle);
  }

  // Records `block` against the bound account. The first cause recorded for an
  // account wins; later calls for the same account are no-ops.
  void Block(const AccountBlock& block) const noexcept {
    openpit_account_control_block(m_handle.Get(),
                                  ::openpit::detail::Native(block));
  }

  // Returns a new handle referring to the same account-control facility, for
  // retaining the ability to block from a later callback within the same
  // pre-trade transaction. Throws `openpit::Error` when this handle is empty.
  [[nodiscard]] AccountControl Clone() const {
    OpenPitAccountControl* raw = openpit_account_control_clone(m_handle.Get());
    if (raw == nullptr) {
      throw ::openpit::Error("openpit_account_control_clone failed");
    }
    return ::openpit::detail::FromNative<AccountControl>(raw);
  }

 private:
  friend class ::openpit::detail::NativeAccess;

  explicit AccountControl(detail::RawAccountControl* handle) noexcept
      : m_handle(handle) {}

  [[nodiscard]] detail::RawAccountControl* Native() const noexcept {
    return m_handle.Get();
  }

  ::openpit::detail::Handle<detail::RawAccountControl,
                            detail::AccountControlDeleter>
      m_handle;
};

// Classifies an `AccountBlockError`.
enum class AccountBlockErrorKind : std::uint32_t {
  // The targeted group is the reserved `param::DefaultAccountGroup`, which
  // cannot be blocked, unblocked, or have its reason replaced.
  ReservedGroup = 0,
  // A reason replacement targeted an account that is not blocked.
  AccountNotBlocked = 1,
  // A group operation targeted a group that is not blocked.
  GroupNotBlocked = 2,
};

// Expected outcome of a failed account- or group-block operation. A value type,
// never thrown. `Block`/`Unblock` are infallible; only `ReplaceBlockReason`,
// `BlockGroup`, `UnblockGroup`, and `ReplaceGroupBlockReason` can produce one.
// `Account` is set only for `AccountNotBlocked`, `Group` only for
// `GroupNotBlocked`.
struct AccountBlockError {
  std::string message;
  std::optional<::openpit::param::AccountId> account;
  std::optional<::openpit::param::AccountGroupId> group;
  AccountBlockErrorKind kind = AccountBlockErrorKind::ReservedGroup;

 private:
  friend class ::openpit::detail::NativeAccess;

  [[nodiscard]] static AccountBlockError FromRaw(
      OpenPitAccountBlockError* handle) {
    ::openpit::detail::Handle<OpenPitAccountBlockError,
                              detail::AccountBlockErrorDeleter>
        owner(handle);
    AccountBlockError out;
    out.message = ::openpit::detail::FromNative<::openpit::StringView>(
                      openpit_account_block_error_get_message(owner.Get()))
                      .ToString();
    out.kind = static_cast<AccountBlockErrorKind>(
        openpit_account_block_error_get_kind(owner.Get()));
    OpenPitParamAccountId accountId = 0;
    if (openpit_account_block_error_get_account(owner.Get(), &accountId)) {
      out.account =
          ::openpit::detail::FromNative<::openpit::param::AccountId>(accountId);
    }
    OpenPitParamAccountGroupId groupId = 0;
    if (openpit_account_block_error_get_group(owner.Get(), &groupId)) {
      out.group =
          ::openpit::detail::FromNative<::openpit::param::AccountGroupId>(
              groupId);
    }
    return out;
  }
};

// Expected outcome of a failed `RegisterGroup` / `UnregisterGroup`: a group
// conflict or a reserved default-group target. A value type, never thrown.
// `currentGroup` is set when the conflict is a duplicate registration (the
// group the account already belongs to) and absent for an unregister miss.
struct AccountGroupError {
  std::string message;
  ::openpit::param::AccountId account;
  std::optional<::openpit::param::AccountGroupId> currentGroup;

 private:
  friend class ::openpit::detail::NativeAccess;

  [[nodiscard]] static AccountGroupError FromRaw(
      OpenPitAccountGroupError* handle) {
    ::openpit::detail::Handle<OpenPitAccountGroupError,
                              detail::AccountGroupErrorDeleter>
        owner(handle);
    AccountGroupError out;
    out.message = ::openpit::detail::FromNative<::openpit::StringView>(
                      openpit_account_group_error_get_message(owner.Get()))
                      .ToString();
    out.account = ::openpit::detail::FromNative<::openpit::param::AccountId>(
        openpit_account_group_error_get_account(owner.Get()));
    OpenPitParamAccountGroupId groupId = 0;
    if (openpit_account_group_error_get_current_group(owner.Get(), &groupId)) {
      out.currentGroup =
          ::openpit::detail::FromNative<::openpit::param::AccountGroupId>(
              groupId);
    }
    return out;
  }
};

/// \brief Account-group and account/group block administration for an engine.
//
// Account-group management and account/group pre-trade blocking bound to an
// engine. Obtained from `Engine::Accounts()`. It carries no state of its own:
// every call forwards to the engine it was created from and is valid for as
// long as that engine is. Non-owning; copyable.
//
// For an example of good practice in building a control plane on this SDK,
// see Pit Officer at <http://officer.openpit.dev/>.
class Accounts {
 public:
  Accounts() = default;

  // Atomically registers every account into `group`; all-or-nothing. Returns an
  // `AccountGroupError` when any account is already in a group or when `group`
  // is the reserved `param::DefaultAccountGroup`. Throws `openpit::Error` on a
  // boundary failure. If joining changes effective currency, stored PnL and
  // cost basis accumulated under the previous one lose their meaning; the SDK
  // guarantees nothing about them.
  // If it changes the effective PnL barrier, current account PnL is checked in
  // the same call (unset is zero and halted is a
  // breach), and any resulting block is latched before return. An unchanged
  // barrier is not checked again.
  [[nodiscard]] std::optional<AccountGroupError> RegisterGroup(
      const std::vector<::openpit::param::AccountId>& accounts,
      ::openpit::param::AccountGroupId group) const {
    return GroupOp(openpit_engine_register_account_group, accounts, group,
                   "openpit_engine_register_account_group failed");
  }

  // Atomically removes every account from `group`; all-or-nothing. Returns an
  // `AccountGroupError` when any account is not in `group` or when `group` is
  // the reserved `param::DefaultAccountGroup`. Throws `openpit::Error` on a
  // boundary failure. If leaving changes effective currency, stored PnL and
  // cost basis accumulated under the previous one lose their meaning; the SDK
  // guarantees nothing about them.
  // If it changes the effective PnL barrier, current account PnL is checked in
  // the same call (unset is zero and halted is a
  // breach), and any resulting block is latched before return. An unchanged
  // barrier is not checked again.
  [[nodiscard]] std::optional<AccountGroupError> UnregisterGroup(
      const std::vector<::openpit::param::AccountId>& accounts,
      ::openpit::param::AccountGroupId group) const {
    return GroupOp(openpit_engine_unregister_account_group, accounts, group,
                   "openpit_engine_unregister_account_group failed");
  }

  // The account-group of `account`, or `std::nullopt` when it belongs to none.
  [[nodiscard]] std::optional<::openpit::param::AccountGroupId> GroupOf(
      ::openpit::param::AccountId account) const {
    OpenPitParamAccountGroupId group = 0;
    if (openpit_engine_account_group(
            m_engine, ::openpit::detail::Native(account), &group)) {
      return ::openpit::detail::FromNative<::openpit::param::AccountGroupId>(
          group);
    }
    return std::nullopt;
  }

  // Sets the explicit currency used by account-aware policies. Effective
  // currency resolves from the account, then its group, then
  // param::DefaultAccountGroup. This write is unchecked and re-evaluates
  // nothing. Stored PnL and cost basis accumulated under a different effective
  // currency lose their meaning; the SDK guarantees nothing about them and does
  // not convert, detect, or report the change. Barrier selection itself stays
  // deterministic: the next policy access re-resolves the cascade with the new
  // effective currency.
  void SetCurrency(::openpit::param::AccountId account,
                   const ::openpit::param::Asset& asset) const {
    OpenPitSharedString* error = nullptr;
    if (!openpit_engine_set_account_currency(
            m_engine, ::openpit::detail::Native(account),
            ::openpit::detail::Native(asset), &error)) {
      ::openpit::detail::ThrowFromSharedString(
          error, "openpit_engine_set_account_currency failed");
    }
  }

  // Clears the explicit account currency. Effective currency resolves from the
  // account, then its group, then param::DefaultAccountGroup. This write is
  // unchecked and re-evaluates nothing. Stored PnL and cost basis accumulated
  // under a different effective currency lose their meaning; the SDK guarantees
  // nothing about them and does not convert, detect, or report the change.
  // Barrier selection itself stays deterministic: the next policy access
  // re-resolves the cascade with the new effective currency.
  void ClearCurrency(::openpit::param::AccountId account) const noexcept {
    openpit_engine_clear_account_currency(m_engine,
                                          ::openpit::detail::Native(account));
  }

  // Sets the currency shared by a group. Effective currency resolves from the
  // account, then its group, then param::DefaultAccountGroup. This write is
  // unchecked and re-evaluates nothing. Stored PnL and cost basis accumulated
  // under a different effective currency lose their meaning; the SDK guarantees
  // nothing about them and does not convert, detect, or report the change.
  // Barrier selection itself stays deterministic: the next policy access
  // re-resolves the cascade with the new effective currency.
  void SetGroupCurrency(::openpit::param::AccountGroupId group,
                        const ::openpit::param::Asset& asset) const {
    OpenPitSharedString* error = nullptr;
    if (!openpit_engine_set_account_group_currency(
            m_engine, ::openpit::detail::Native(group),
            ::openpit::detail::Native(asset), &error)) {
      ::openpit::detail::ThrowFromSharedString(
          error, "openpit_engine_set_account_group_currency failed");
    }
  }

  // Clears the group currency. Effective currency resolves from the account,
  // then its group, then param::DefaultAccountGroup. This write is unchecked
  // and re-evaluates nothing. Stored PnL and cost basis accumulated under a
  // different effective currency lose their meaning; the SDK guarantees nothing
  // about them and does not convert, detect, or report the change. Barrier
  // selection itself stays deterministic: the next policy access re-resolves
  // the cascade with the new effective currency.
  void ClearGroupCurrency(
      ::openpit::param::AccountGroupId group) const noexcept {
    openpit_engine_clear_account_group_currency(
        m_engine, ::openpit::detail::Native(group));
  }

  // Blocks `account` with `reason`, gating its pre-trade orders until
  // unblocked. The first reason recorded for an account wins; `reason` may be
  // empty.
  void Block(::openpit::param::AccountId account,
             std::string_view reason) const noexcept {
    openpit_engine_block_account(m_engine, ::openpit::detail::Native(account),
                                 ::openpit::detail::MakeStringView(reason));
  }

  // Lifts the block on `account`. Unblocking an unblocked account is a no-op.
  void Unblock(::openpit::param::AccountId account) const noexcept {
    openpit_engine_unblock_account(m_engine,
                                   ::openpit::detail::Native(account));
  }

  // Clears the engine-wide block, letting every account through again.
  //
  // A global block is raised by the engine itself, never by an admin call: a
  // kill switch reported for an execution report with no readable account, or a
  // mutation finalizer of a custom policy that failed - every mutation a C++
  // policy registers through `tx::Mutations::Push` is one. This is the
  // operator's counterpart, so the engine can be returned to service once the
  // inconsistency has been investigated.
  //
  // Idempotent: a no-op when no global block is active. Accounts and account
  // groups blocked individually stay blocked; clear those with `Unblock` and
  // `UnblockGroup`.
  void UnblockAll() const noexcept {
    openpit_engine_unblock_all_accounts(m_engine);
  }

  // Replaces the recorded reason of a blocked account. Returns an
  // `AccountBlockError` with kind `AccountNotBlocked` when `account` is not
  // blocked.
  [[nodiscard]] std::optional<AccountBlockError> ReplaceBlockReason(
      ::openpit::param::AccountId account, std::string_view reason) const {
    OpenPitAccountBlockError* error = nullptr;
    openpit_engine_replace_account_block_reason(
        m_engine, ::openpit::detail::Native(account),
        ::openpit::detail::MakeStringView(reason), &error);
    if (error != nullptr) {
      return ::openpit::detail::FromNative<AccountBlockError>(error);
    }
    return std::nullopt;
  }

  // Blocks `group` with `reason`, gating the pre-trade orders of every account
  // in it. The first reason recorded for a group wins; `reason` may be empty.
  // Returns an `AccountBlockError` with kind `ReservedGroup` when `group` is
  // the reserved `param::DefaultAccountGroup`.
  [[nodiscard]] std::optional<AccountBlockError> BlockGroup(
      ::openpit::param::AccountGroupId group, std::string_view reason) const {
    OpenPitAccountBlockError* error = nullptr;
    openpit_engine_block_account_group(
        m_engine, ::openpit::detail::Native(group),
        ::openpit::detail::MakeStringView(reason), &error);
    if (error != nullptr) {
      return ::openpit::detail::FromNative<AccountBlockError>(error);
    }
    return std::nullopt;
  }

  // Lifts the block on `group`; accounts blocked individually stay blocked.
  // Unblocking an unblocked group is a no-op. Returns an `AccountBlockError`
  // with kind `ReservedGroup` when `group` is the reserved
  // `param::DefaultAccountGroup`.
  [[nodiscard]] std::optional<AccountBlockError> UnblockGroup(
      ::openpit::param::AccountGroupId group) const {
    OpenPitAccountBlockError* error = nullptr;
    openpit_engine_unblock_account_group(
        m_engine, ::openpit::detail::Native(group), &error);
    if (error != nullptr) {
      return ::openpit::detail::FromNative<AccountBlockError>(error);
    }
    return std::nullopt;
  }

  // Replaces the recorded reason of a blocked group. Returns an
  // `AccountBlockError` with kind `ReservedGroup` when `group` is the reserved
  // `param::DefaultAccountGroup`, or `GroupNotBlocked` when `group` is not
  // blocked.
  [[nodiscard]] std::optional<AccountBlockError> ReplaceGroupBlockReason(
      ::openpit::param::AccountGroupId group, std::string_view reason) const {
    OpenPitAccountBlockError* error = nullptr;
    openpit_engine_replace_account_group_block_reason(
        m_engine, ::openpit::detail::Native(group),
        ::openpit::detail::MakeStringView(reason), &error);
    if (error != nullptr) {
      return ::openpit::detail::FromNative<AccountBlockError>(error);
    }
    return std::nullopt;
  }

 private:
  friend class ::openpit::detail::NativeAccess;

  explicit Accounts(detail::RawEngine* engine) noexcept : m_engine(engine) {}

  [[nodiscard]] detail::RawEngine* Native() const noexcept { return m_engine; }

  // Shared register/unregister body. `fn` is the matching native runtime
  // symbol; both have an identical signature.
  template <typename Fn>
  [[nodiscard]] std::optional<AccountGroupError> GroupOp(
      Fn fn, const std::vector<::openpit::param::AccountId>& accounts,
      ::openpit::param::AccountGroupId group, const char* fallback) const {
    std::vector<OpenPitParamAccountId> raw;
    raw.reserve(accounts.size());
    for (const auto& account : accounts) {
      raw.push_back(::openpit::detail::Native(account));
    }
    OpenPitAccountGroupError* groupError = nullptr;
    OpenPitSharedString* error = nullptr;
    const bool ok = fn(m_engine, raw.empty() ? nullptr : raw.data(), raw.size(),
                       ::openpit::detail::Native(group), &groupError, &error);
    if (ok) {
      return std::nullopt;
    }
    if (groupError != nullptr) {
      return ::openpit::detail::FromNative<AccountGroupError>(groupError);
    }
    ::openpit::detail::ThrowFromSharedString(error, fallback);
  }

  detail::RawEngine* m_engine = nullptr;
};

}  // namespace openpit::accounts
