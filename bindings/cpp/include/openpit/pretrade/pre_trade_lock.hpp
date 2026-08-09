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

#include "openpit/bytes.hpp"
#include "openpit/detail/handle.hpp"
#include "openpit/error.hpp"
#include "openpit/param/param.hpp"
#include "openpit/string.hpp"

#include <openpit.h>

#include <cstddef>
#include <cstdint>
#include <string>
#include <utility>
#include <vector>

// Pre-trade lock: the hot-path handle accumulating reserved-price records.
//
// `PreTradeLock` is a copyable RAII value with deep-copy semantics. Copying a
// moved-from lock yields another moved-from lock. Its mutating hot-path
// operations
// (`Push`, `PushMany`, `Len`, `IsEmpty`, `Merge`, `PricesOf`) allocate no C++
// containers; only snapshots (`Entries`/`Prices`) and serialization helpers
// materialize owned data. Every operation that uses the native lock requires a
// live handle and throws `openpit::Error` otherwise; `operator bool()` reports
// handle presence. A lock has no reject channel - it only stores prices.

namespace openpit::pretrade {

// One `(policy_group_id, price)` record stored in a lock.
struct LockEntry {
  std::uint16_t policyGroupId = 0;
  ::openpit::param::Price price;

  LockEntry(std::uint16_t group, ::openpit::param::Price entryPrice)
      : policyGroupId(group), price(entryPrice) {}

 private:
  friend class ::openpit::detail::NativeAccess;

  [[nodiscard]] static LockEntry FromRaw(
      const OpenPitPretradePreTradeLockEntry& raw) {
    return LockEntry(
        raw.policy_group_id,
        ::openpit::detail::FromNative<::openpit::param::Price>(raw.price));
  }

  [[nodiscard]] OpenPitPretradePreTradeLockEntry Native() const noexcept {
    OpenPitPretradePreTradeLockEntry raw{};
    raw.policy_group_id = policyGroupId;
    raw.price = ::openpit::detail::Native(price);
    return raw;
  }
};

namespace detail {

using RawPreTradeLock = ::OpenPitPretradePreTradeLock;

struct PreTradeLockDeleter {
  void operator()(OpenPitPretradePreTradeLock* handle) const noexcept {
    openpit_destroy_pretrade_pre_trade_lock(handle);
  }
};

struct PreTradeLockPricesDeleter {
  void operator()(OpenPitPretradePreTradeLockPrices* handle) const noexcept {
    openpit_destroy_pretrade_pre_trade_lock_prices(handle);
  }
};

struct PreTradeLockEntriesDeleter {
  void operator()(OpenPitPretradePreTradeLockEntries* handle) const noexcept {
    openpit_destroy_pretrade_pre_trade_lock_entries(handle);
  }
};

}  // namespace detail

// RAII pre-trade lock with deep-copy value semantics. Copying a moved-from lock
// yields another moved-from lock; destruction releases the native handle.
class PreTradeLock {
 public:
  // Allocates an empty lock. The C constructor always succeeds.
  PreTradeLock() : m_handle(openpit_create_pretrade_pre_trade_lock()) {}

  PreTradeLock(PreTradeLock&&) noexcept = default;
  PreTradeLock& operator=(PreTradeLock&&) noexcept = default;

  PreTradeLock(const PreTradeLock& other) : PreTradeLock(CopyOf(other)) {}

  PreTradeLock& operator=(const PreTradeLock& other) {
    if (this != &other) {
      *this = CopyOf(other);
    }
    return *this;
  }

  [[nodiscard]] explicit operator bool() const noexcept {
    return static_cast<bool>(m_handle);
  }

  // Returns an independent deep copy of this lock.
  [[nodiscard]] PreTradeLock Clone() const {
    OpenPitPretradePreTradeLock* raw =
        openpit_pretrade_pre_trade_lock_clone(Native());
    if (raw == nullptr) {
      throw Error("openpit_pretrade_pre_trade_lock_clone failed");
    }
    return ::openpit::detail::FromNative<PreTradeLock>(raw);
  }

  // Total number of stored prices across all groups.
  [[nodiscard]] std::size_t Len() const {
    return openpit_pretrade_pre_trade_lock_len(Native());
  }

  [[nodiscard]] bool IsEmpty() const {
    return openpit_pretrade_pre_trade_lock_is_empty(Native());
  }

  // Appends `price` under `policyGroupId`. Throws `openpit::Error` when the
  // price fails domain validation.
  void Push(std::uint16_t policyGroupId, ::openpit::param::Price price) {
    OpenPitSharedString* error = nullptr;
    if (!openpit_pretrade_pre_trade_lock_push(Native(), policyGroupId,
                                              ::openpit::detail::Native(price),
                                              &error)) {
      ::openpit::detail::ThrowFromSharedString(
          error, "openpit_pretrade_pre_trade_lock_push failed");
    }
  }

  // Appends every record from `entries`, in order. On the first invalid price
  // nothing is appended and `openpit::Error` is thrown.
  void PushMany(const std::vector<LockEntry>& entries) {
    std::vector<OpenPitPretradePreTradeLockEntry> raw;
    raw.reserve(entries.size());
    for (const LockEntry& entry : entries) {
      raw.push_back(::openpit::detail::Native(entry));
    }
    OpenPitSharedString* error = nullptr;
    if (!openpit_pretrade_pre_trade_lock_push_many(Native(), raw.data(),
                                                   raw.size(), &error)) {
      ::openpit::detail::ThrowFromSharedString(
          error, "openpit_pretrade_pre_trade_lock_push_many failed");
    }
  }

  // Appends every record from `src` into this lock, leaving `src` unchanged.
  void Merge(const PreTradeLock& src) {
    OpenPitSharedString* error = nullptr;
    if (!openpit_pretrade_pre_trade_lock_merge(Native(), src.Native(),
                                               &error)) {
      ::openpit::detail::ThrowFromSharedString(
          error, "openpit_pretrade_pre_trade_lock_merge failed");
    }
  }

  // A snapshot of every `(policy_group_id, price)` record, in iteration order
  // (default-group records first, then each non-default group in insertion
  // order). Off the hot path: materializes owned values.
  [[nodiscard]] std::vector<LockEntry> Entries() const {
    ::openpit::detail::Handle<OpenPitPretradePreTradeLockEntries,
                              detail::PreTradeLockEntriesDeleter>
        entries(openpit_pretrade_pre_trade_lock_entries(Native()));
    const OpenPitPretradePreTradeLockEntriesView view =
        openpit_pretrade_pre_trade_lock_entries_view(entries.Get());
    std::vector<LockEntry> out;
    if (view.ptr == nullptr || view.len == 0) {
      return out;
    }
    out.reserve(view.len);
    for (std::size_t index = 0; index < view.len; ++index) {
      out.push_back(::openpit::detail::FromNative<LockEntry>(view.ptr[index]));
    }
    return out;
  }

  // Every stored price, in the same iteration order as `Entries`. Off the hot
  // path.
  [[nodiscard]] std::vector<::openpit::param::Price> Prices() const {
    ::openpit::detail::Handle<OpenPitPretradePreTradeLockEntries,
                              detail::PreTradeLockEntriesDeleter>
        entries(openpit_pretrade_pre_trade_lock_entries(Native()));
    const OpenPitPretradePreTradeLockEntriesView view =
        openpit_pretrade_pre_trade_lock_entries_view(entries.Get());
    std::vector<::openpit::param::Price> out;
    out.reserve(view.len);
    for (std::size_t index = 0; index < view.len; ++index) {
      out.push_back(::openpit::detail::FromNative<::openpit::param::Price>(
          view.ptr[index].price));
    }
    return out;
  }

  // Prices stored under `policyGroupId`, in insertion order. Throws
  // `openpit::Error` on a boundary failure.
  [[nodiscard]] std::vector<::openpit::param::Price> PricesOf(
      std::uint16_t policyGroupId) const {
    OpenPitParamPrice singlePrice{};
    OpenPitPretradePreTradeLockPrices* listRaw = nullptr;
    OpenPitSharedString* error = nullptr;
    const OpenPitPretradePreTradeLockPricesStatus status =
        openpit_pretrade_pre_trade_lock_prices_of(
            Native(), policyGroupId, &singlePrice, &listRaw, &error);
    switch (status) {
      case OpenPitPretradePreTradeLockPricesStatus_Empty:
        return {};
      case OpenPitPretradePreTradeLockPricesStatus_One:
        return {::openpit::detail::FromNative<::openpit::param::Price>(
            singlePrice)};
      case OpenPitPretradePreTradeLockPricesStatus_List: {
        ::openpit::detail::Handle<OpenPitPretradePreTradeLockPrices,
                                  detail::PreTradeLockPricesDeleter>
            list(listRaw);
        return PricesFromView(
            openpit_pretrade_pre_trade_lock_prices_view(list.Get()));
      }
      default:
        ::openpit::detail::ThrowFromSharedString(
            error, "openpit_pretrade_pre_trade_lock_prices_of failed");
    }
  }

  //----------------------------------------------------------------------------
  // Serialization conveniences. Off the hot path. The msgpack/cbor forms
  // produce bytes; json produces a UTF-8 string.

  [[nodiscard]] std::vector<std::uint8_t> ToMsgpack() const {
    return ToBytes(openpit_pretrade_pre_trade_lock_to_msgpack,
                   "openpit_pretrade_pre_trade_lock_to_msgpack failed");
  }

  [[nodiscard]] static PreTradeLock FromMsgpack(
      const std::vector<std::uint8_t>& payload) {
    return FromBytes(
        payload, openpit_create_pretrade_pre_trade_lock_from_msgpack,
        "openpit_create_pretrade_pre_trade_lock_from_msgpack failed");
  }

  [[nodiscard]] std::vector<std::uint8_t> ToCbor() const {
    return ToBytes(openpit_pretrade_pre_trade_lock_to_cbor,
                   "openpit_pretrade_pre_trade_lock_to_cbor failed");
  }

  [[nodiscard]] static PreTradeLock FromCbor(
      const std::vector<std::uint8_t>& payload) {
    return FromBytes(payload, openpit_create_pretrade_pre_trade_lock_from_cbor,
                     "openpit_create_pretrade_pre_trade_lock_from_cbor failed");
  }

  [[nodiscard]] std::string ToJson() const {
    OpenPitSharedString* error = nullptr;
    OpenPitSharedString* handle =
        openpit_pretrade_pre_trade_lock_to_json(Native(), &error);
    if (handle == nullptr) {
      ::openpit::detail::ThrowFromSharedString(
          error, "openpit_pretrade_pre_trade_lock_to_json failed");
    }
    return ::openpit::detail::FromNative<::openpit::SharedString>(handle)
        .ToString();
  }

  [[nodiscard]] static PreTradeLock FromJson(std::string_view payload) {
    OpenPitSharedString* error = nullptr;
    OpenPitPretradePreTradeLock* raw =
        openpit_create_pretrade_pre_trade_lock_from_json(
            reinterpret_cast<const std::uint8_t*>(payload.data()),
            payload.size(), &error);
    if (raw == nullptr) {
      ::openpit::detail::ThrowFromSharedString(
          error, "openpit_create_pretrade_pre_trade_lock_from_json failed");
    }
    return ::openpit::detail::FromNative<PreTradeLock>(raw);
  }

 private:
  friend class ::openpit::detail::NativeAccess;

  explicit PreTradeLock(detail::RawPreTradeLock* handle) noexcept
      : m_handle(handle) {}

  [[nodiscard]] detail::RawPreTradeLock* Native() const {
    if (!m_handle) {
      throw Error("pre-trade lock has no handle (was moved from)");
    }
    return m_handle.Get();
  }

  [[nodiscard]] static PreTradeLock CopyOf(const PreTradeLock& other) {
    if (!other) {
      return PreTradeLock(nullptr);
    }
    return other.Clone();
  }

  [[nodiscard]] static std::vector<::openpit::param::Price> PricesFromView(
      const OpenPitPretradePreTradeLockPricesView& view) {
    std::vector<::openpit::param::Price> out;
    if (view.ptr == nullptr || view.len == 0) {
      return out;
    }
    out.reserve(view.len);
    for (std::size_t index = 0; index < view.len; ++index) {
      out.push_back(::openpit::detail::FromNative<::openpit::param::Price>(
          view.ptr[index]));
    }
    return out;
  }

  template <typename Encoder>
  [[nodiscard]] std::vector<std::uint8_t> ToBytes(Encoder encoder,
                                                  const char* fallback) const {
    OpenPitSharedString* error = nullptr;
    OpenPitSharedBytes* handle = encoder(Native(), &error);
    if (handle == nullptr) {
      ::openpit::detail::ThrowFromSharedString(error, fallback);
    }
    auto bytes = ::openpit::detail::FromNative<::openpit::SharedBytes>(handle);
    return bytes.ToVector();
  }

  template <typename Decoder>
  [[nodiscard]] static PreTradeLock FromBytes(
      const std::vector<std::uint8_t>& payload, Decoder decoder,
      const char* fallback) {
    OpenPitSharedString* error = nullptr;
    OpenPitPretradePreTradeLock* raw =
        decoder(payload.data(), payload.size(), &error);
    if (raw == nullptr) {
      ::openpit::detail::ThrowFromSharedString(error, fallback);
    }
    return ::openpit::detail::FromNative<PreTradeLock>(raw);
  }

  ::openpit::detail::Handle<detail::RawPreTradeLock,
                            detail::PreTradeLockDeleter>
      m_handle;
};

}  // namespace openpit::pretrade
