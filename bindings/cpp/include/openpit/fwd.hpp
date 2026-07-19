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

#include <cstdint>

namespace openpit {

class Error;
class ConfigureError;
class EngineBuildError;
class Engine;
class EngineBuilder;
class InstrumentId;
class Order;
class ReferenceBook;
class ExecutionReport;

struct AdjustmentResult;
struct PostTradeResult;
struct PolicyConfigurationResult;
struct SettlementLag;
struct SettlementScheme;

enum class ConfigureErrorKind : std::uint32_t;
enum class SettlementUnit : std::uint8_t;
enum class SyncPolicy : std::uint8_t;
enum class EngineBuildErrorCode : std::uint8_t;

}  // namespace openpit

namespace openpit::param {

class AccountId;
class AccountGroupId;
class Asset;
class AdjustmentAmount;
class CashFlow;
class Fee;
class GroupId;
class Leverage;
class MonetaryAmount;
class Notional;
class Pnl;
class PositionSize;
class Price;
class Quantity;
class Volume;

enum class AdjustmentAmountKind : std::uint8_t;
enum class FillType : std::uint8_t;
enum class Kind : std::uint8_t;
enum class RoundingStrategy : std::uint8_t;

}  // namespace openpit::param

namespace openpit::model {

class Order;
class ExecutionReport;
class TradeAmount;

struct ExecutionReportOperation;
struct Fill;
struct FinancialImpact;
struct Instrument;
struct OrderMargin;
struct OrderOperation;
struct OrderPosition;
struct PositionImpact;
struct Trade;

enum class PositionEffect : std::uint8_t;
enum class PositionMode : std::uint8_t;
enum class PositionSide : std::uint8_t;
enum class Side : std::uint8_t;
enum class TradeAmountKind : std::uint8_t;

}  // namespace openpit::model

namespace openpit::pretrade {

class AccountOutcomes;
class Context;
class DryRunReport;
class PostTradeAdjustments;
class PostTradeContext;
class PostTradePnls;
class PreTradeLock;
class Request;
class Result;
class Reservation;

template <typename Handler>
class CustomPolicy;

struct ExecuteResult;
struct LockEntry;
struct PolicyAccountAdjustmentResult;
struct PolicyDecision;
struct Reject;
struct StartResult;

enum class RejectCode : std::uint16_t;
enum class RejectScope : std::uint8_t;

}  // namespace openpit::pretrade

namespace openpit::tx {

class Mutations;

}  // namespace openpit::tx

namespace openpit::pretrade::policies {

class OrderSizeLimitPolicy;
class OrderValidationPolicy;
class PnlBoundsKillSwitchPolicy;
class RateLimitPolicy;
class SpotFundsPnlBoundsGlobalBarrierUpdate;
class SpotFundsPnlBoundsKillSwitchPolicy;
class SpotFundsPolicy;

struct OrderSizeAccountAssetBarrier;
struct OrderSizeAssetBarrier;
struct OrderSizeBrokerBarrier;
struct OrderSizeLimit;
struct PnlBoundsAccountBarrier;
struct PnlBoundsAccountBarrierUpdate;
struct PnlBoundsBrokerBarrier;
struct RateLimit;
struct RateLimitAccountAssetBarrier;
struct RateLimitAccountBarrier;
struct RateLimitAssetBarrier;
struct RateLimitBrokerBarrier;
struct SpotFundsOverride;
struct SpotFundsPnlBoundsAccountBarrier;
struct SpotFundsPnlBoundsAccountGroupBarrier;
struct SpotFundsPnlBoundsBarrier;

enum class SpotFundsLimitMode : std::uint8_t;
enum class SpotFundsPricingSource : std::uint8_t;

}  // namespace openpit::pretrade::policies

namespace openpit {

class Configurator;

}  // namespace openpit

namespace openpit::accounts {

class AccountControl;
class Accounts;

struct AccountBlock;
struct AccountBlockError;
struct AccountGroupError;

enum class AccountBlockErrorKind : std::uint32_t;

}  // namespace openpit::accounts

namespace openpit::accountadjustment {

class AccountPnlOperation;
class BatchError;
class Context;
class Operation;
class OutcomeList;

struct AccountAdjustment;
struct AccountOutcomeEntry;
struct AccountPnlOutcome;
struct Amount;
struct BalanceOperation;
struct Bounds;
struct Outcome;
struct OutcomeAmount;
struct PnlOutcome;
struct PnlOutcomeAmount;
struct PositionOperation;

enum class PnlHaltReason : std::uint8_t;

}  // namespace openpit::accountadjustment

namespace openpit::marketdata {

class Builder;
class Quote;
class QuoteTtl;
class Service;

enum class GetStatus : std::uint8_t;
enum class QuoteResolution : std::uint8_t;
enum class RegisterStatus : std::uint8_t;
enum class SyncPolicy : std::uint8_t;

struct GetResult;
struct RegisterResult;

using InstrumentId = ::openpit::InstrumentId;

}  // namespace openpit::marketdata

namespace openpit::asyncengine {

class Error;
class EngineAdapter;
class OwnedTypedAsyncEngine;
class NoopObserver;
class Observer;

template <typename T>
class Future;
template <typename A, typename B>
class PairFuture;
template <typename T>
class Promise;
template <typename A, typename B>
class PairPromise;
template <typename T>
class Result;

template <typename Driver>
class AsyncEngine;
template <typename Driver>
class AsyncAccounts;
template <typename Driver>
class AsyncRequest;
template <typename Driver>
class AsyncReservation;
template <typename Driver>
class Builder;
template <typename Driver>
class DynamicBuilder;
template <typename Driver>
class ShardedBuilder;
template <typename Driver>
class TypedAsyncEngine;
template <typename Driver>
class TypedBuilder;
template <typename Driver>
class TypedDynamicBuilder;
template <typename Driver>
class TypedShardedBuilder;

struct AdjustmentOutcome;

template <typename Driver>
struct ExecuteOutcome;
template <typename Driver>
struct StartOutcome;

enum class ErrorCode : std::uint8_t;

}  // namespace openpit::asyncengine
