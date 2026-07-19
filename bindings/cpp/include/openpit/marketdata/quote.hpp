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

#include "openpit/param.hpp"

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
// it. The `With*` builders return a copy with the field set, mirroring the
// value-type style. Wraps the native runtime POD
// `OpenPitMarketDataQuote`.
class Quote {
 public:
  // An empty quote with every field unset.
  Quote() noexcept : m_value(openpit_create_marketdata_quote()) {}

  // Adopts a native runtime quote value (e.g. read from a `Service::Get`).
  [[nodiscard]] static Quote FromRaw(OpenPitMarketDataQuote raw) noexcept {
    Quote out;
    out.m_value = raw;
    return out;
  }

  // Returns a copy with the mark price set.
  [[nodiscard]] Quote WithMark(const param::Price& mark) const noexcept {
    Quote out = *this;
    out.m_value.mark = param::PriceOptional{mark.Raw(), true};
    return out;
  }

  // Returns a copy with the best-bid price set.
  [[nodiscard]] Quote WithBid(const param::Price& bid) const noexcept {
    Quote out = *this;
    out.m_value.bid = param::PriceOptional{bid.Raw(), true};
    return out;
  }

  // Returns a copy with the best-ask price set.
  [[nodiscard]] Quote WithAsk(const param::Price& ask) const noexcept {
    Quote out = *this;
    out.m_value.ask = param::PriceOptional{ask.Raw(), true};
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

  [[nodiscard]] OpenPitMarketDataQuote Raw() const noexcept { return m_value; }

 private:
  [[nodiscard]] static std::optional<param::Price> Read(
      const param::PriceOptional& field) noexcept {
    if (!field.is_set) {
      return std::nullopt;
    }
    return param::Price::FromRaw(field.value);
  }

  OpenPitMarketDataQuote m_value;
};

//------------------------------------------------------------------------------
// QuoteResolution

// Controls how `Service::Get` resolves a quote for a specific account: which
// buckets are consulted, in order, when the more-specific bucket has no quote.
// Mirrors `OpenPitMarketDataQuoteResolution`.
enum class QuoteResolution : std::uint8_t {
  // Consults only the per-account bucket; no fallback.
  AccountOnly = OPENPIT_MARKET_DATA_QUOTE_RESOLUTION_ACCOUNT_ONLY,
  // Consults the per-account bucket, then the account's group bucket.
  AccountThenGroup = OPENPIT_MARKET_DATA_QUOTE_RESOLUTION_ACCOUNT_THEN_GROUP,
  // Consults the per-account bucket, then the account's group bucket, then the
  // default account-group ("everyone-else") bucket, in that order.
  AccountThenGroupThenDefault =
      OPENPIT_MARKET_DATA_QUOTE_RESOLUTION_ACCOUNT_THEN_GROUP_THEN_DEFAULT,
};

[[nodiscard]] inline OpenPitMarketDataQuoteResolution ToRaw(
    QuoteResolution resolution) noexcept {
  return static_cast<OpenPitMarketDataQuoteResolution>(resolution);
}

//------------------------------------------------------------------------------
// QuoteTtl

// A service-wide or per-instrument quote lifetime. An infinite TTL means quotes
// never expire on their own; a finite TTL expires a quote after the configured
// duration following the push that wrote it. Wraps the native runtime POD
// `OpenPitMarketDataQuoteTtl`.
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

  [[nodiscard]] OpenPitMarketDataQuoteTtl Raw() const noexcept {
    return m_value;
  }

 private:
  explicit QuoteTtl(OpenPitMarketDataQuoteTtl value) noexcept
      : m_value(value) {}

  OpenPitMarketDataQuoteTtl m_value;
};

}  // namespace openpit::marketdata
