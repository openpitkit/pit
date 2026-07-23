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
#include "openpit/detail/handle.hpp"
#include "openpit/engine.hpp"
#include "openpit/error.hpp"
#include "openpit/marketdata/account_info.hpp"
#include "openpit/marketdata/instrument_id.hpp"
#include "openpit/marketdata/quote.hpp"
#include "openpit/model/model.hpp"
#include "openpit/param/account_id.hpp"
#include "openpit/param/param.hpp"

#include <openpit.h>

#include <cstddef>
#include <cstdint>
#include <optional>
#include <vector>

// Market-data service binding.
//
// A `Service` is a shared, reference-counted registry of instruments and their
// latest quotes, shared between a feed that publishes quotes and the policies
// that read them. It is obtained from the engine builder so its synchronization
// mode is derived from the engine's: a no-sync engine yields a no-sync,
// single-threaded service whose locks are a free no-op; a full-sync engine
// yields a fully synchronized service safe for a concurrent quote feed. There
// is no standalone market-data runtime.
//
// Error model: runtime boundary failures (null handle, invalid mode, an invalid
// price in a pushed quote) throw `openpit::Error`. Expected business outcomes
// (already-registered, unknown instrument, no usable quote, no target) are
// returned by value as `RegisterStatus` / `GetStatus`, never as exceptions.

namespace openpit::marketdata {

// Outcome of a registration, push, or TTL-mutation call. Boundary failures are
// surfaced as a thrown `openpit::Error` instead.
enum class RegisterStatus : std::uint8_t {
  // The operation succeeded.
  Ok = 0,
  // The instrument is already registered (auto-id registration).
  AlreadyRegistered = 1,
  // The caller-supplied instrument id is already registered.
  DuplicateId = 2,
  // The instrument is already registered under a different id.
  DuplicateInstrument = 3,
  // The instrument id is not registered.
  UnknownInstrument = 4,
  // A `PushFor` call specified neither an account nor a group target.
  NoTarget = 6,
};

/// \brief Value result returned by market-data registration calls.
//
// Boundary/runtime failures throw `openpit::Error`; domain outcomes stay in
// `status`. `instrumentId` is set only when a new registration succeeded.
struct RegisterResult {
  RegisterStatus status = RegisterStatus::Ok;
  std::optional<InstrumentId> instrumentId;
  std::optional<InstrumentId> conflictingInstrumentId;
  std::optional<model::Instrument> conflictingInstrument;

  [[nodiscard]] bool Ok() const noexcept {
    return status == RegisterStatus::Ok;
  }
};

// Outcome of a quote read.
enum class GetStatus : std::uint8_t {
  // A usable quote was found and written to the out-parameter.
  Found = 0,
  // The instrument is registered but holds no usable quote (never pushed,
  // cleared, or aged past its TTL).
  Unavailable = 1,
  // The instrument id is not registered.
  UnknownInstrument = 2,
  // The selected quote exists but aged past its effective TTL.
  QuoteExpired = 3,
};

/// \brief Value result returned by account-aware quote reads.
//
// `quote` is set for `GetStatus::Found` and `GetStatus::QuoteExpired`; the
// latter preserves the stale quote for diagnostics and reconciliation.
struct GetResult {
  GetStatus status = GetStatus::UnknownInstrument;
  std::optional<Quote> quote;

  [[nodiscard]] bool Found() const noexcept {
    return status == GetStatus::Found;
  }

  [[nodiscard]] bool Expired() const noexcept {
    return status == GetStatus::QuoteExpired;
  }
};

namespace detail {

using RawService = ::OpenPitMarketDataService;

struct ServiceDeleter {
  void operator()(OpenPitMarketDataService* handle) const noexcept {
    openpit_destroy_marketdata_service(handle);
  }
};

}  // namespace detail

// RAII handle to a market-data service.
//
// Move-only. `Clone` hands out an additional handle to the same underlying
// service so that, for example, a feed and a policy can operate on identical
// state; destruction releases this handle while the underlying service stays
// alive as long as other handles to it exist.
class Service {
 public:
  Service() = default;

  [[nodiscard]] explicit operator bool() const noexcept {
    return static_cast<bool>(m_handle);
  }

  // Returns a new handle referring to the same underlying service.
  [[nodiscard]] Service Clone() const {
    OpenPitMarketDataService* raw =
        openpit_marketdata_service_clone(m_handle.Get());
    if (raw == nullptr) {
      throw Error("openpit_marketdata_service_clone failed");
    }
    return ::openpit::detail::FromNative<Service>(raw);
  }

  //----------------------------------------------------------------------------
  // Registration

  // Registers `instrument` with the service-wide default TTL.
  [[nodiscard]] RegisterResult Register(const model::Instrument& instrument) {
    const OpenPitInstrument raw = ::openpit::detail::Native(instrument);
    OpenPitMarketDataInstrumentId id = 0;
    OpenPitSharedString* error = nullptr;
    const OpenPitMarketDataRegisterStatus status =
        openpit_marketdata_service_register(m_handle.Get(), &raw, &id, &error);
    return MapRegister(status, error, "openpit_marketdata_service_register",
                       &id, instrument, std::nullopt);
  }

  // Registers `instrument` with a per-instrument TTL override.
  [[nodiscard]] RegisterResult Register(const model::Instrument& instrument,
                                        const QuoteTtl& ttl) {
    const OpenPitInstrument raw = ::openpit::detail::Native(instrument);
    OpenPitMarketDataInstrumentId id = 0;
    OpenPitSharedString* error = nullptr;
    const OpenPitMarketDataRegisterStatus status =
        openpit_marketdata_service_register_with_ttl(
            m_handle.Get(), &raw, ::openpit::detail::Native(ttl), &id, &error);
    return MapRegister(status, error,
                       "openpit_marketdata_service_register_with_ttl", &id,
                       instrument, std::nullopt);
  }

  // Registers `instrument` under the caller-supplied `id` with the service-wide
  // default TTL.
  [[nodiscard]] RegisterResult Register(const model::Instrument& instrument,
                                        InstrumentId id) {
    const OpenPitInstrument raw = ::openpit::detail::Native(instrument);
    OpenPitMarketDataInstrumentId resolved = 0;
    OpenPitSharedString* error = nullptr;
    const OpenPitMarketDataRegisterStatus status =
        openpit_marketdata_service_register_with_id(
            m_handle.Get(), &raw, ::openpit::detail::Native(id), &resolved,
            &error);
    return MapRegister(status, error,
                       "openpit_marketdata_service_register_with_id", &resolved,
                       instrument, id);
  }

  // Registers `instrument` under the caller-supplied `id` with a per-instrument
  // TTL override.
  [[nodiscard]] RegisterResult Register(const model::Instrument& instrument,
                                        InstrumentId id, const QuoteTtl& ttl) {
    const OpenPitInstrument raw = ::openpit::detail::Native(instrument);
    OpenPitMarketDataInstrumentId resolved = 0;
    OpenPitSharedString* error = nullptr;
    const OpenPitMarketDataRegisterStatus status =
        openpit_marketdata_service_register_with_id_and_ttl(
            m_handle.Get(), &raw, ::openpit::detail::Native(id),
            ::openpit::detail::Native(ttl), &resolved, &error);
    return MapRegister(status, error,
                       "openpit_marketdata_service_register_with_id_and_ttl",
                       &resolved, instrument, id);
  }

  // Resolves `instrument` to its registered id, returning `std::nullopt` when
  // it is not registered by name.
  [[nodiscard]] std::optional<InstrumentId> Resolve(
      const model::Instrument& instrument) const {
    const OpenPitInstrument raw = ::openpit::detail::Native(instrument);
    OpenPitMarketDataInstrumentId id = 0;
    if (!openpit_marketdata_service_resolve(m_handle.Get(), &raw, &id)) {
      return std::nullopt;
    }
    return InstrumentId(id);
  }

  //----------------------------------------------------------------------------
  // Pushing

  // Publishes `quote` for `instrumentId`, replacing the entire stored snapshot
  // in the default ("everyone-else") bucket.
  [[nodiscard]] RegisterStatus Push(InstrumentId instrumentId,
                                    const Quote& quote) {
    OpenPitSharedString* error = nullptr;
    const OpenPitMarketDataRegisterStatus status =
        openpit_marketdata_service_push(
            m_handle.Get(), ::openpit::detail::Native(instrumentId),
            ::openpit::detail::Native(quote), &error);
    return MapPush(status, error, "openpit_marketdata_service_push");
  }

  // Publishes a partial update for `instrumentId`, merging it into the stored
  // snapshot in the default bucket.
  [[nodiscard]] RegisterStatus PushPatch(InstrumentId instrumentId,
                                         const Quote& quote) {
    OpenPitSharedString* error = nullptr;
    const OpenPitMarketDataRegisterStatus status =
        openpit_marketdata_service_push_patch(
            m_handle.Get(), ::openpit::detail::Native(instrumentId),
            ::openpit::detail::Native(quote), &error);
    return MapPush(status, error, "openpit_marketdata_service_push_patch");
  }

  // Publishes `quote` for `instrumentId` into the per-account bucket of every
  // account in `accountIds` and the per-group bucket of every group in
  // `accountGroupIds`, replacing each target's snapshot. At least one target
  // must be supplied; both lists empty yields `RegisterStatus::NoTarget`. Pass
  // `param::DefaultAccountGroup` in `accountGroupIds` to target the default
  // bucket directly.
  [[nodiscard]] RegisterStatus PushFor(
      InstrumentId instrumentId, const Quote& quote,
      const std::vector<param::AccountId>& accountIds,
      const std::vector<param::AccountGroupId>& accountGroupIds) {
    const std::vector<OpenPitParamAccountId> accounts = RawAccounts(accountIds);
    const std::vector<OpenPitParamAccountGroupId> groups =
        RawGroups(accountGroupIds);
    OpenPitSharedString* error = nullptr;
    const OpenPitMarketDataRegisterStatus status =
        openpit_marketdata_service_push_for(
            m_handle.Get(), ::openpit::detail::Native(instrumentId),
            ::openpit::detail::Native(quote), DataOrNull(accounts),
            accounts.size(), DataOrNull(groups), groups.size(), &error);
    return MapPush(status, error, "openpit_marketdata_service_push_for");
  }

  // Publishes a partial update for `instrumentId` into each target bucket,
  // merging independently into each existing snapshot. See `PushFor`.
  [[nodiscard]] RegisterStatus PushForPatch(
      InstrumentId instrumentId, const Quote& quote,
      const std::vector<param::AccountId>& accountIds,
      const std::vector<param::AccountGroupId>& accountGroupIds) {
    const std::vector<OpenPitParamAccountId> accounts = RawAccounts(accountIds);
    const std::vector<OpenPitParamAccountGroupId> groups =
        RawGroups(accountGroupIds);
    OpenPitSharedString* error = nullptr;
    const OpenPitMarketDataRegisterStatus status =
        openpit_marketdata_service_push_for_patch(
            m_handle.Get(), ::openpit::detail::Native(instrumentId),
            ::openpit::detail::Native(quote), DataOrNull(accounts),
            accounts.size(), DataOrNull(groups), groups.size(), &error);
    return MapPush(status, error, "openpit_marketdata_service_push_for_patch");
  }

  // Publishes `quote` for `instrument`, replacing the stored snapshot, and
  // returns the instrument's id. If `instrument` is unregistered, a named slot
  // is created with the service-default TTL.
  [[nodiscard]] InstrumentId PushByInstrument(
      const model::Instrument& instrument, const Quote& quote) {
    return PushByInstrumentImpl(
        instrument, quote, openpit_marketdata_service_push_by_instrument,
        "openpit_marketdata_service_push_by_instrument");
  }

  // Publishes a partial update for `instrument`, merging it into the stored
  // snapshot, and returns the instrument's id.
  [[nodiscard]] InstrumentId PushByInstrumentPatch(
      const model::Instrument& instrument, const Quote& quote) {
    return PushByInstrumentImpl(
        instrument, quote, openpit_marketdata_service_push_by_instrument_patch,
        "openpit_marketdata_service_push_by_instrument_patch");
  }

  // Hides the current quote for `instrumentId` across all buckets without
  // unregistering it. A no-op if `instrumentId` is not registered.
  void Clear(InstrumentId instrumentId) {
    openpit_marketdata_service_clear(m_handle.Get(),
                                     ::openpit::detail::Native(instrumentId));
  }

  //----------------------------------------------------------------------------
  // Reading

  // Reads the latest quote for `instrumentId` with account-aware resolution.
  // `accountInfo` supplies the reading account's group lazily — the core
  // invokes its `AccountGroup()` only when the fallback chain reaches the
  // per-group bucket. `resolution` selects the fallback chain.
  template <typename AccountInfo>
  [[nodiscard]] GetResult Get(InstrumentId instrumentId,
                              param::AccountId accountId,
                              const AccountInfo& accountInfo,
                              QuoteResolution resolution) const {
    OpenPitMarketDataQuote raw{};
    ::openpit::detail::ClearPendingCallbackException();
    const OpenPitMarketDataGetStatus status = openpit_marketdata_service_get(
        m_handle.Get(), ::openpit::detail::Native(instrumentId),
        ::openpit::detail::Native(accountId),
        &AccountGroupResolverTrampoline<AccountInfo>,
        // The trampoline borrows `accountInfo` only for this call; the const
        // cast is required by the native runtime `void*` user-data parameter.
        const_cast<AccountInfo*>(&accountInfo), detail::ToNative(resolution),
        &raw);
    ::openpit::detail::ThrowIfPendingCallbackException();
    if (status == OpenPitMarketDataGetStatus_Error) {
      throw ::openpit::Error("invalid market-data quote resolution");
    }
    GetResult result;
    result.status = static_cast<GetStatus>(status);
    if (result.Found() || result.Expired()) {
      result.quote = ::openpit::detail::FromNative<Quote>(raw);
    }
    return result;
  }

  // Reads the latest quote for `instrumentId`, returning `std::nullopt` for
  // every non-`Found` outcome (unknown instrument, unavailable quote, or
  // expired quote). Use `Get` when those cases or a stale quote must be
  // inspected.
  template <typename AccountInfo>
  [[nodiscard]] std::optional<Quote> Find(InstrumentId instrumentId,
                                          param::AccountId accountId,
                                          const AccountInfo& accountInfo,
                                          QuoteResolution resolution) const {
    GetResult result = Get(instrumentId, accountId, accountInfo, resolution);
    if (!result.Found()) {
      return std::nullopt;
    }
    return result.quote;
  }

  //----------------------------------------------------------------------------
  // TTL overrides

  // Updates the instrument-level TTL for an already-registered instrument.
  [[nodiscard]] RegisterStatus SetInstrumentTtl(InstrumentId instrumentId,
                                                const QuoteTtl& ttl) {
    return static_cast<RegisterStatus>(
        openpit_marketdata_service_set_instrument_ttl(
            m_handle.Get(), ::openpit::detail::Native(instrumentId),
            ::openpit::detail::Native(ttl)));
  }

  // Reverts the instrument-level TTL for `instrumentId` back to "inherit".
  [[nodiscard]] RegisterStatus ClearInstrumentTtl(InstrumentId instrumentId) {
    return static_cast<RegisterStatus>(
        openpit_marketdata_service_clear_instrument_ttl(
            m_handle.Get(), ::openpit::detail::Native(instrumentId)));
  }

  // Pins the service-level TTL for `accountId`.
  void SetAccountTtl(param::AccountId accountId, const QuoteTtl& ttl) {
    openpit_marketdata_service_set_account_ttl(
        m_handle.Get(), ::openpit::detail::Native(accountId),
        ::openpit::detail::Native(ttl));
  }

  // Reverts the service-level TTL for `accountId` back to "inherit".
  void ClearAccountTtl(param::AccountId accountId) {
    openpit_marketdata_service_clear_account_ttl(
        m_handle.Get(), ::openpit::detail::Native(accountId));
  }

  // Pins the service-level TTL for `accountGroupId`. Pass
  // `param::DefaultAccountGroup` to target the default-group TTL.
  void SetAccountGroupTtl(param::AccountGroupId accountGroupId,
                          const QuoteTtl& ttl) {
    openpit_marketdata_service_set_account_group_ttl(
        m_handle.Get(), ::openpit::detail::Native(accountGroupId),
        ::openpit::detail::Native(ttl));
  }

  // Reverts the service-level TTL for `accountGroupId` back to "inherit".
  void ClearAccountGroupTtl(param::AccountGroupId accountGroupId) {
    openpit_marketdata_service_clear_account_group_ttl(
        m_handle.Get(), ::openpit::detail::Native(accountGroupId));
  }

  // Pins the highest-priority instrument x account TTL cell.
  [[nodiscard]] RegisterStatus SetInstrumentAccountTtl(
      InstrumentId instrumentId, param::AccountId accountId,
      const QuoteTtl& ttl) {
    return static_cast<RegisterStatus>(
        openpit_marketdata_service_set_instrument_account_ttl(
            m_handle.Get(), ::openpit::detail::Native(instrumentId),
            ::openpit::detail::Native(accountId),
            ::openpit::detail::Native(ttl)));
  }

  // Reverts the instrument x account TTL cell back to "inherit".
  [[nodiscard]] RegisterStatus ClearInstrumentAccountTtl(
      InstrumentId instrumentId, param::AccountId accountId) {
    return static_cast<RegisterStatus>(
        openpit_marketdata_service_clear_instrument_account_ttl(
            m_handle.Get(), ::openpit::detail::Native(instrumentId),
            ::openpit::detail::Native(accountId)));
  }

  // Pins the instrument x group TTL cell. Pass `param::DefaultAccountGroup` for
  // the instrument's default-group cell.
  [[nodiscard]] RegisterStatus SetInstrumentAccountGroupTtl(
      InstrumentId instrumentId, param::AccountGroupId accountGroupId,
      const QuoteTtl& ttl) {
    return static_cast<RegisterStatus>(
        openpit_marketdata_service_set_instrument_account_group_ttl(
            m_handle.Get(), ::openpit::detail::Native(instrumentId),
            ::openpit::detail::Native(accountGroupId),
            ::openpit::detail::Native(ttl)));
  }

  // Reverts the instrument x group TTL cell back to "inherit". Pass
  // `param::DefaultAccountGroup` for the instrument's default-group cell.
  [[nodiscard]] RegisterStatus ClearInstrumentAccountGroupTtl(
      InstrumentId instrumentId, param::AccountGroupId accountGroupId) {
    return static_cast<RegisterStatus>(
        openpit_marketdata_service_clear_instrument_account_group_ttl(
            m_handle.Get(), ::openpit::detail::Native(instrumentId),
            ::openpit::detail::Native(accountGroupId)));
  }

 private:
  // The native runtime borrows `accountInfo` only for the enclosing `Get` call.
  template <typename AccountInfo>
  static bool AccountGroupResolverTrampoline(
      void* userData, OpenPitParamAccountGroupId* outAccountGroupId) noexcept {
    try {
      const auto* accountInfo = static_cast<const AccountInfo*>(userData);
      const std::optional<param::AccountGroupId> group =
          accountInfo->AccountGroup();
      if (!group.has_value()) {
        return false;
      }
      *outAccountGroupId = ::openpit::detail::Native(*group);
      return true;
    } catch (...) {
      ::openpit::detail::CaptureCurrentCallbackException();
      return false;
    }
  }

  using PushByInstrumentFn = bool (*)(const OpenPitMarketDataService*,
                                      const OpenPitInstrument*,
                                      OpenPitMarketDataQuote,
                                      OpenPitMarketDataInstrumentId*,
                                      OpenPitOutError);

  [[nodiscard]] InstrumentId PushByInstrumentImpl(
      const model::Instrument& instrument, const Quote& quote,
      PushByInstrumentFn fn, const char* fallback) {
    const OpenPitInstrument raw = ::openpit::detail::Native(instrument);
    OpenPitMarketDataInstrumentId id = 0;
    OpenPitSharedString* error = nullptr;
    if (!fn(m_handle.Get(), &raw, ::openpit::detail::Native(quote), &id,
            &error)) {
      ::openpit::detail::ThrowFromSharedString(error, fallback);
    }
    return InstrumentId(id);
  }

  // Maps a register-family status, throwing on the `Error` boundary case and
  // carrying the resolved id on `Ok`.
  [[nodiscard]] static RegisterResult MapRegister(
      OpenPitMarketDataRegisterStatus status, OpenPitSharedString* error,
      const char* fallback, const OpenPitMarketDataInstrumentId* id,
      const model::Instrument& instrument,
      std::optional<InstrumentId> requestedId) {
    if (status == OpenPitMarketDataRegisterStatus_Error) {
      ::openpit::detail::ThrowFromSharedString(error, fallback);
    }
    RegisterResult result;
    result.status = static_cast<RegisterStatus>(status);
    if (status == OpenPitMarketDataRegisterStatus_Ok) {
      result.instrumentId = InstrumentId(*id);
    } else if (status == OpenPitMarketDataRegisterStatus_AlreadyRegistered ||
               status == OpenPitMarketDataRegisterStatus_DuplicateInstrument) {
      result.conflictingInstrument = instrument;
    } else if (status == OpenPitMarketDataRegisterStatus_DuplicateId) {
      result.conflictingInstrumentId = requestedId;
    }
    return result;
  }

  // Maps a push-family status, throwing on the `Error` boundary case.
  [[nodiscard]] static RegisterStatus MapPush(
      OpenPitMarketDataRegisterStatus status, OpenPitSharedString* error,
      const char* fallback) {
    if (status == OpenPitMarketDataRegisterStatus_Error) {
      ::openpit::detail::ThrowFromSharedString(error, fallback);
    }
    return static_cast<RegisterStatus>(status);
  }

  [[nodiscard]] static std::vector<OpenPitParamAccountId> RawAccounts(
      const std::vector<param::AccountId>& accounts) {
    std::vector<OpenPitParamAccountId> raw;
    raw.reserve(accounts.size());
    for (const param::AccountId& account : accounts) {
      raw.push_back(::openpit::detail::Native(account));
    }
    return raw;
  }

  [[nodiscard]] static std::vector<OpenPitParamAccountGroupId> RawGroups(
      const std::vector<param::AccountGroupId>& groups) {
    std::vector<OpenPitParamAccountGroupId> raw;
    raw.reserve(groups.size());
    for (const param::AccountGroupId& group : groups) {
      raw.push_back(::openpit::detail::Native(group));
    }
    return raw;
  }

  template <typename T>
  [[nodiscard]] static const T* DataOrNull(
      const std::vector<T>& values) noexcept {
    return values.empty() ? nullptr : values.data();
  }

  friend class ::openpit::detail::NativeAccess;

  explicit Service(detail::RawService* handle) noexcept : m_handle(handle) {}

  [[nodiscard]] detail::RawService* Native() const noexcept {
    return m_handle.Get();
  }

  ::openpit::detail::Handle<detail::RawService, detail::ServiceDeleter>
      m_handle;
};

//------------------------------------------------------------------------------
// Builder

// The synchronization mode of a market-data service.
//
// `Service` is normally obtained from the engine builder, which derives this
// mode from the engine's sync policy. A no-sync engine builds a `Builder`; call
// `FullSync()` to upgrade it when a background producer must publish quotes
// concurrently with the engine. A full-sync engine builds a `Builder` already
// fixed to `Full`, which cannot be downgraded.
enum class SyncPolicy : std::uint8_t {
  None = 0,
  Full = 1,
};

// Builds a market-data `Service` with a fixed default TTL and a chosen
// synchronization mode.
//
// The mode starts from the engine's sync policy (use `Engine`-builder wiring to
// obtain a builder with the correct starting mode) and can only be upgraded
// from `None` to `Full`, never downgraded.
class Builder {
 public:
  // Constructs a builder with the given default TTL and synchronization mode.
  Builder(QuoteTtl defaultTtl, SyncPolicy syncPolicy) noexcept
      : m_defaultTtl(defaultTtl), m_syncPolicy(syncPolicy) {}

  // Constructs a builder whose starting sync mode is derived from an engine's
  // sync policy, mirroring how the engine builder hands out a market-data
  // builder: a `None` engine starts no-sync; a `Full` or `Account` engine
  // starts full-sync (and must not be downgraded). Use `FullSync()` afterwards
  // to upgrade a no-sync builder when a background producer needs concurrent
  // access.
  [[nodiscard]] static Builder FromEngineSyncPolicy(
      QuoteTtl defaultTtl, ::openpit::SyncPolicy enginePolicy) noexcept {
    const SyncPolicy mode = enginePolicy == ::openpit::SyncPolicy::None
                                ? SyncPolicy::None
                                : SyncPolicy::Full;
    return Builder(defaultTtl, mode);
  }

  // Upgrades the builder to full synchronization, making the resulting service
  // safe for concurrent access. Always valid.
  Builder& FullSync() noexcept {
    m_syncPolicy = SyncPolicy::Full;
    return *this;
  }

  // Selects no synchronization. Valid only before any upgrade; a service built
  // for a multi-threaded engine must not be downgraded.
  Builder& NoSync() noexcept {
    m_syncPolicy = SyncPolicy::None;
    return *this;
  }

  // Constructs the market-data service. Throws `openpit::Error` on a boundary
  // failure (e.g. an invalid mode).
  [[nodiscard]] Service Build() const {
    OpenPitSharedString* error = nullptr;
    OpenPitMarketDataService* raw = openpit_create_marketdata_service(
        static_cast<std::uint8_t>(m_syncPolicy),
        ::openpit::detail::Native(m_defaultTtl), &error);
    if (raw == nullptr) {
      ::openpit::detail::ThrowFromSharedString(
          error, "openpit_create_marketdata_service failed");
    }
    return ::openpit::detail::FromNative<Service>(raw);
  }

 private:
  QuoteTtl m_defaultTtl;
  SyncPolicy m_syncPolicy;
};

}  // namespace openpit::marketdata
