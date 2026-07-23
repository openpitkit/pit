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

#include "openpit/param/param.hpp"

#include <openpit.h>

#include <chrono>
#include <cstdint>
#include <optional>

namespace openpit::marketdata {

//------------------------------------------------------------------------------
// Quote

// A market snapshot carrying an optional `mark`, `bid`, and `ask` price.
//
// Every field is optional: an unset field means the producer did not publish
// it. The `With*` builders return a copy with the field set, preserving
// value-type semantics.
class Quote {
 public:
  // An empty quote with every field unset.
  Quote() noexcept : m_value(openpit_create_marketdata_quote()) {}

  // Returns a copy with the mark price set.
  [[nodiscard]] Quote WithMark(const param::Price& mark) const noexcept {
    Quote out = *this;
    out.m_value.mark =
        param::detail::RawPriceOptional{::openpit::detail::Native(mark), true};
    return out;
  }

  // Returns a copy with the best-bid price set.
  [[nodiscard]] Quote WithBid(const param::Price& bid) const noexcept {
    Quote out = *this;
    out.m_value.bid =
        param::detail::RawPriceOptional{::openpit::detail::Native(bid), true};
    return out;
  }

  // Returns a copy with the best-ask price set.
  [[nodiscard]] Quote WithAsk(const param::Price& ask) const noexcept {
    Quote out = *this;
    out.m_value.ask =
        param::detail::RawPriceOptional{::openpit::detail::Native(ask), true};
    return out;
  }

  [[nodiscard]] std::optional<param::Price> Mark() const noexcept {
    return Read(m_value.mark);
  }

  [[nodiscard]] std::optional<param::Price> Bid() const noexcept {
    return Read(m_value.bid);
  }

  [[nodiscard]] std::optional<param::Price> Ask() const noexcept {
    return Read(m_value.ask);
  }

 private:
  friend class ::openpit::detail::NativeAccess;

  explicit Quote(OpenPitMarketDataQuote value) noexcept : m_value(value) {}

  [[nodiscard]] OpenPitMarketDataQuote Native() const noexcept {
    return m_value;
  }

  [[nodiscard]] static std::optional<param::Price> Read(
      const param::detail::RawPriceOptional& field) noexcept {
    if (!field.is_set) {
      return std::nullopt;
    }
    return ::openpit::detail::FromNative<param::Price>(field.value);
  }

  OpenPitMarketDataQuote m_value;
};

//------------------------------------------------------------------------------
// QuoteResolution

// Controls how `Service::Get` resolves a quote for a specific account: which
// buckets are consulted, in order, when the more-specific bucket has no quote.
enum class QuoteResolution : std::uint8_t {
  // Consults only the per-account bucket; no fallback.
  AccountOnly = 0,
  // Consults the per-account bucket, then the account's group bucket.
  AccountThenGroup = 1,
  // Consults the per-account bucket, then the account's group bucket, then the
  // default account-group ("everyone-else") bucket, in that order.
  AccountThenGroupThenDefault = 2,
};

namespace detail {

using RawQuoteResolution = ::OpenPitMarketDataQuoteResolution;

[[nodiscard]] inline RawQuoteResolution ToNative(
    QuoteResolution resolution) noexcept {
  return static_cast<RawQuoteResolution>(resolution);
}

}  // namespace detail

//------------------------------------------------------------------------------
// QuoteTtl

// A service-wide or per-instrument quote lifetime. An infinite TTL means quotes
// never expire on their own; a finite TTL expires a quote after the configured
// duration following the push that wrote it.
class QuoteTtl {
 public:
  // A lifetime under which quotes never expire on their own.
  [[nodiscard]] static QuoteTtl Infinite() noexcept {
    return QuoteTtl(openpit_create_marketdata_quote_ttl_infinite());
  }

  // A finite lifetime of `secs` seconds plus `nanos` nanoseconds.
  [[nodiscard]] static QuoteTtl Within(std::uint64_t secs,
                                       std::uint32_t nanos) noexcept {
    return QuoteTtl(openpit_create_marketdata_quote_ttl_within(secs, nanos));
  }

  // A finite lifetime taken from a `std::chrono` duration. Negative durations
  // are clamped to zero.
  [[nodiscard]] static QuoteTtl Within(
      std::chrono::nanoseconds duration) noexcept {
    if (duration < std::chrono::nanoseconds::zero()) {
      duration = std::chrono::nanoseconds::zero();
    }
    const auto secs =
        std::chrono::duration_cast<std::chrono::seconds>(duration);
    const auto nanos = duration - secs;
    return Within(static_cast<std::uint64_t>(secs.count()),
                  static_cast<std::uint32_t>(nanos.count()));
  }

 private:
  friend class ::openpit::detail::NativeAccess;

  explicit QuoteTtl(OpenPitMarketDataQuoteTtl value) noexcept
      : m_value(value) {}

  [[nodiscard]] OpenPitMarketDataQuoteTtl Native() const noexcept {
    return m_value;
  }

  OpenPitMarketDataQuoteTtl m_value;
};

}  // namespace openpit::marketdata
