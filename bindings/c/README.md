# OpenPit (Pre-trade Integrity Toolkit) for C

<!-- markdownlint-disable MD013 -->
[![Verify](https://github.com/openpitkit/pit/actions/workflows/verify.yml/badge.svg)](https://github.com/openpitkit/pit/actions/workflows/verify.yml) [![Release](https://github.com/openpitkit/pit/actions/workflows/release.yml/badge.svg)](https://github.com/openpitkit/pit/actions/workflows/release.yml) [![C API](https://img.shields.io/badge/C%20API-blue)](../../docs/c-api/index.md) [![License](https://img.shields.io/badge/license-Apache%202.0-blue)](../../LICENSE)
<!-- markdownlint-enable MD013 -->

`openpit.h` is an embeddable pre-trade risk SDK entrypoint for integrating
policy-driven risk checks into trading systems from C and other environments
that consume a C ABI.

For an overview and links to all resources, see the project website [openpit.dev](https://openpit.dev/).
For full project documentation, see [the repository README](https://github.com/openpitkit/pit/blob/main/README.md).
For conceptual and architectural pages, see [the project wiki](https://github.com/openpitkit/pit/wiki).
For the split C reference manual, see [the C API docs](https://github.com/openpitkit/pit/blob/main/docs/c-api/index.md).

## Versioning Policy (Pre‑1.0)

Before the `1.0` release OpenPit follows a relaxed Semantic Versioning:

- `PATCH` releases carry bug fixes and small internal corrections.
- `MINOR` releases may introduce new features **and may also change the
  public interface**.

Breaking API changes can appear in minor releases before `1.0`. Pick
version constraints that tolerate API evolution during the pre-stable
phase.

## Getting Started

Visit the [C API documentation](https://github.com/openpitkit/pit/blob/main/docs/c-api/index.md).

## Install

For normal end-user installation, use the published [GitHub release assets](https://github.com/openpitkit/pit/releases):

- `openpit.h`
- `libopenpit_ffi.so` on Linux
- `libopenpit_ffi.dylib` on macOS
- `openpit_ffi.dll` on Windows
- `LICENSE`
- `OWNERS`

If you need local development/debugging, clone this repository and generate the
header plus the reference docs.

<details>
<summary>POSIX (Linux, macOS, etc)</summary>

Dependencies: [Rust](https://rustup.rs/) and
[Python 3.10+](https://www.python.org/downloads/). [Just](https://just.systems/)
is optional but recommended.

With [Just](https://just.systems/):

```bash
just gen-api-c
```

Manual:

```bash
python3 scripts/generate_api_c.py
```

If you need a workspace build:

With [Just](https://just.systems/):

```bash
just build-debug
```

Manual:

```bash
cargo build --workspace
```

</details>

<details>
<summary>Windows</summary>

Install [rustup](https://rustup.rs/), target `x86_64-pc-windows-msvc`, and
[Python 3.10+](https://www.python.org/downloads/). [Just](https://just.systems/)
is optional but recommended.

With [Just](https://just.systems/):

```powershell
rustup target add x86_64-pc-windows-msvc
just gen-api-c
just build-debug
```

Manual:

```powershell
rustup target add x86_64-pc-windows-msvc
python scripts\generate_api_c.py
cargo build --workspace --target x86_64-pc-windows-msvc
```

</details>

If you need only the release C runtime library:

```bash
cargo build -p openpit-ffi --release
```

## Engine

### Overview

The engine evaluates an order through a deterministic pre-trade pipeline:

- `openpit_engine_start_pre_trade(...)` runs start-stage policies and creates a
  deferred request
- `openpit_pretrade_pre_trade_request_execute(...)` runs main-stage check
  policies
- `openpit_engine_execute_pre_trade(...)` runs the start and main stages in a
  single call
- `openpit_pretrade_pre_trade_reservation_commit(...)` applies reserved state
- `openpit_pretrade_pre_trade_reservation_rollback(...)` reverts reserved state
- `openpit_engine_apply_execution_report(...)` updates post-trade policy state
- `openpit_engine_apply_account_adjustment(...)` validates and applies account
  adjustments

Start-stage policies aggregate rejects from all registered policies. Main-stage
policies aggregate rejects and run rollback mutations in reverse order when any
reject is produced.

Built-in policies:

- [Spot Funds](https://github.com/openpitkit/pit/wiki/Spot-Funds) - per-account
  solvency gate over spendable funds.
- [Order Validation](https://github.com/openpitkit/pit/wiki/Policies#ordervalidationpolicy)
  \- structural integrity checks on every order.
- [Rate Limit](https://github.com/openpitkit/pit/wiki/Policies#ratelimitpolicy)
  \- throttle order flow per broker, asset, or account.
- [Order Size Limit](https://github.com/openpitkit/pit/wiki/Policies#ordersizelimitpolicy)
  \- fat-finger caps on quantity and notional.
- [P&L Kill Switch](https://github.com/openpitkit/pit/wiki/Policies#pnlboundskillswitchpolicy)
  \- halt an account when realized P&L breaches bounds.
- plus your own via the [policy SDK](https://github.com/openpitkit/pit/wiki/Policy-API).

The primary integration model is to build project-specific policies against the
public C API: [the C API docs](../../docs/c-api/index.md).

Two types of rejections are supported: a full kill switch for the account and a
rejection of only the current request. Kill switches are intended for
algorithmic trading where automatic order submission must be halted until the
situation is analyzed.

Built-in policies that maintain state across calls use the SDK's [Storage](https://github.com/openpitkit/pit/wiki/Storage)
abstraction internally. The runtime library handles the necessary memory
synchronization for policy state; the C consumer is responsible only for the
threading contract on the SDK handle.

Policies that need live prices (such as spot funds) read quotes from a
market-data service built with `openpit_create_marketdata_builder(...)` and
updated through the `openpit_marketdata_service_*` functions; one service handle
can be shared between a quote feed and the engine.

## Usage

<!-- Test mirror: none; C snippets are intentionally not mirrored. -->
```c
#include <stdbool.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "openpit.h"

static int report_last_error(const char *context) {
    fprintf(stderr, "%s failed\n", context);
    return 1;
}

/* Prints the error message, then destroys the handle. */
static int report_out_error(const char *context, OpenPitSharedString *error) {
    char buf[512];
    OpenPitStringView view = openpit_shared_string_view(error);
    size_t n = view.len < sizeof(buf) - 1 ? view.len : sizeof(buf) - 1;
    if (view.ptr != NULL && n > 0) {
        memcpy(buf, view.ptr, n);
    }
    buf[n] = '\0';
    fprintf(stderr, "%s: %s\n", context, buf);
    openpit_destroy_shared_string(error);
    return 1;
}

static int make_quantity(
    int64_t mantissa,
    int32_t scale,
    OpenPitParamQuantity *out
) {
    if (!openpit_create_param_quantity(
            (OpenPitParamDecimal){
                .mantissa_lo = mantissa,
                .mantissa_hi = mantissa < 0 ? -1 : 0,
                .scale = scale},
            out,
            NULL)) {
        return report_last_error("openpit_create_param_quantity");
    }
    return 0;
}

static int make_volume(
    int64_t mantissa,
    int32_t scale,
    OpenPitParamVolume *out
) {
    if (!openpit_create_param_volume(
            (OpenPitParamDecimal){
                .mantissa_lo = mantissa,
                .mantissa_hi = mantissa < 0 ? -1 : 0,
                .scale = scale},
            out,
            NULL)) {
        return report_last_error("openpit_create_param_volume");
    }
    return 0;
}

static int make_price(int64_t mantissa, int32_t scale, OpenPitParamPrice *out) {
    if (!openpit_create_param_price(
            (OpenPitParamDecimal){
                .mantissa_lo = mantissa,
                .mantissa_hi = mantissa < 0 ? -1 : 0,
                .scale = scale},
            out,
            NULL)) {
        return report_last_error("openpit_create_param_price");
    }
    return 0;
}

static int make_pnl(int64_t mantissa, int32_t scale, OpenPitParamPnl *out) {
    if (!openpit_create_param_pnl(
            (OpenPitParamDecimal){
                .mantissa_lo = mantissa,
                .mantissa_hi = mantissa < 0 ? -1 : 0,
                .scale = scale},
            out,
            NULL)) {
        return report_last_error("openpit_create_param_pnl");
    }
    return 0;
}

static int make_fee(int64_t mantissa, int32_t scale, OpenPitParamFee *out) {
    if (!openpit_create_param_fee(
            (OpenPitParamDecimal){
                .mantissa_lo = mantissa,
                .mantissa_hi = mantissa < 0 ? -1 : 0,
                .scale = scale},
            out,
            NULL)) {
        return report_last_error("openpit_create_param_fee");
    }
    return 0;
}

static OpenPitStringView make_string_view(const char *s) {
    return (OpenPitStringView){
        .ptr = (const uint8_t *)s,
        .len = s != NULL ? strlen(s) : 0};
}

static const char *view_to_cstr(OpenPitStringView view, char *buf, size_t cap) {
    size_t n = view.len < cap - 1 ? view.len : cap - 1;
    if (buf == NULL || cap == 0) {
        return "";
    }
    if (view.ptr == NULL || view.len == 0) {
        buf[0] = '\0';
        return buf;
    }
    memcpy(buf, view.ptr, n);
    buf[n] = '\0';
    return buf;
}

static void print_reject(const OpenPitPretradeReject *reject) {
    char policy[128];
    char reason[256];
    char details[256];
    fprintf(
        stderr,
        "%s [%u]: %s: %s\n",
        view_to_cstr(reject->policy, policy, sizeof(policy)),
        (unsigned)reject->code,
        view_to_cstr(reject->reason, reason, sizeof(reason)),
        view_to_cstr(reject->details, details, sizeof(details)));
}

static void print_reject_list(const OpenPitPretradeRejectList *rejects) {
    size_t i = 0;
    size_t len = openpit_pretrade_reject_list_len(rejects);
    for (i = 0; i < len; ++i) {
        OpenPitPretradeReject item = {0};
        if (openpit_pretrade_reject_list_get(rejects, i, &item)) {
            print_reject(&item);
        }
    }
}

int main(void) {
    OpenPitSharedString *error = NULL;
    int rc = 1;
    bool reservation_committed = false;

    /* 1. Configure policies. */
    OpenPitPretradePoliciesPnlBoundsBarrier pnl_barrier = {0};
    OpenPitPretradePoliciesRateLimitBrokerBarrier rate_limit = {0};
    OpenPitPretradePoliciesOrderSizeBrokerBarrier order_size_limit = {0};
    OpenPitEngineBuilder *builder = NULL;
    OpenPitEngine *engine = NULL;
    OpenPitPretradePreTradeRequest *request = NULL;
    OpenPitPretradePreTradeReservation *reservation = NULL;
    OpenPitPretradeRejectList *start_rejects = NULL;
    OpenPitPretradeRejectList *execute_rejects = NULL;
    OpenPitPretradeAccountBlockList *account_blocks = NULL;
    OpenPitOrder order = {0};
    OpenPitExecutionReport report = {0};

    pnl_barrier.settlement_asset = make_string_view("USD");
    if (make_pnl(-1000, 0, &pnl_barrier.lower_bound.value) != 0) {
        goto cleanup;
    }
    pnl_barrier.lower_bound.is_set = true;

    rate_limit.max_orders = 100;
    rate_limit.window_nanoseconds = 1000000000;

    if (make_quantity(500, 0, &order_size_limit.limit.max_quantity) != 0) {
        goto cleanup;
    }
    if (make_volume(100000, 0, &order_size_limit.limit.max_notional) != 0) {
        goto cleanup;
    }

    /* 2. Build the engine once during platform initialization. */
    builder = openpit_create_engine_builder(OpenPitSyncPolicy_Full, &error);
    if (builder == NULL) {
        report_out_error("openpit_create_engine_builder", error);
        error = NULL;
        goto cleanup;
    }

    if (!openpit_engine_builder_add_builtin_order_validation_policy(
            builder, 0, &error)) {  /* group id 0 = default group */
        report_out_error(
            "openpit_engine_builder_add_builtin_order_validation_policy",
            error);
        error = NULL;
        goto cleanup;
    }
    if (!openpit_engine_builder_add_builtin_pnl_bounds_killswitch_policy(
            builder, 0, &pnl_barrier, 1, NULL, 0, &error)) {
        report_out_error(
            "openpit_engine_builder_add_builtin_pnl_bounds_killswitch_policy",
            error);
        error = NULL;
        goto cleanup;
    }
    if (!openpit_engine_builder_add_builtin_rate_limit_policy(
            builder, 0, &rate_limit, NULL, 0, NULL, 0, NULL, 0, &error)) {
        report_out_error(
            "openpit_engine_builder_add_builtin_rate_limit_policy",
            error);
        error = NULL;
        goto cleanup;
    }
    if (!openpit_engine_builder_add_builtin_order_size_limit_policy(
            builder, 0, &order_size_limit, NULL, 0, NULL, 0, &error)) {
        report_out_error(
            "openpit_engine_builder_add_builtin_order_size_limit_policy",
            error);
        error = NULL;
        goto cleanup;
    }

    /*
     * On a domain build failure (for example, a duplicate policy name or group
     * id) the structured error is written to out_build_error and out_error is
     * left untouched. This example passes NULL for out_build_error to stay
     * minimal, so it must not read error on failure; use a generic message.
     */
    engine = openpit_engine_builder_build(builder, NULL, &error);
    if (engine == NULL) {
        report_last_error("openpit_engine_builder_build");
        if (error != NULL) {
            openpit_destroy_shared_string(error);
            error = NULL;
        }
        goto cleanup;
    }

    /* 3. Build one order as a POD payload. */
    OpenPitParamQuantity qty = {0};
    OpenPitParamPrice px = {0};
    if (make_quantity(100, 0, &qty) != 0 || make_price(185, 0, &px) != 0) {
        goto cleanup;
    }
    order.operation.is_set = true;
    order.operation.value.instrument.underlying_asset = make_string_view("AAPL");
    order.operation.value.instrument.settlement_asset = make_string_view("USD");
    order.operation.value.side = OpenPitParamSide_Buy;
    order.operation.value.trade_amount.kind = OpenPitParamTradeAmountKind_Quantity;
    order.operation.value.trade_amount.value = qty._0;
    order.operation.value.price.value = px;
    order.operation.value.price.is_set = true;

    /* 4. Start-stage checks. */
    error = NULL;
    request = NULL;
    start_rejects = NULL;
    OpenPitPretradeStatus start_status = openpit_engine_start_pre_trade(
        engine, &order, &request, &start_rejects, &error);
    if (start_status == OpenPitPretradeStatus_Error) {
        report_out_error("openpit_engine_start_pre_trade", error);
        error = NULL;
        goto cleanup;
    }
    if (start_status == OpenPitPretradeStatus_Rejected &&
        start_rejects != NULL) {
        print_reject_list(start_rejects);
        goto cleanup;
    }

    /* 5. Main-stage checks. */
    error = NULL;
    execute_rejects = NULL;
    reservation = NULL;
    OpenPitPretradeStatus exec_status = openpit_pretrade_pre_trade_request_execute(
        request, &reservation, &execute_rejects, &error);
    if (exec_status == OpenPitPretradeStatus_Error) {
        report_out_error("openpit_pretrade_pre_trade_request_execute", error);
        error = NULL;
        goto cleanup;
    }
    if (exec_status == OpenPitPretradeStatus_Rejected &&
        execute_rejects != NULL) {
        print_reject_list(execute_rejects);
        goto cleanup;
    }

    /* 6. If the order was sent successfully, commit the reservation. */
    if (reservation == NULL) {
        report_last_error(
            "openpit_pretrade_pre_trade_request_execute: no rejects and no reservation");
        goto cleanup;
    }
    openpit_pretrade_pre_trade_reservation_commit(reservation);
    reservation_committed = true;

    /* 7. Feed back one execution report as a by-value POD payload. */
    report.operation.is_set = true;
    report.operation.value.instrument.underlying_asset = make_string_view("AAPL");
    report.operation.value.instrument.settlement_asset = make_string_view("USD");
    report.operation.value.side = OpenPitParamSide_Buy;

    OpenPitParamPnl pnl = {0};
    OpenPitParamFee fee = {0};
    if (make_pnl(-50, 0, &pnl) != 0 || make_fee(34, 1, &fee) != 0) {
        goto cleanup;
    }
    report.financial_impact.is_set = true;
    report.financial_impact.value.pnl.is_set = true;
    report.financial_impact.value.pnl.value = pnl;
    report.financial_impact.value.fee.is_set = true;
    report.financial_impact.value.fee.value = fee;

    if (!openpit_engine_apply_execution_report(
            engine, &report, &account_blocks, NULL, &error)) {
        report_out_error("openpit_engine_apply_execution_report", error);
        error = NULL;
        goto cleanup;
    }

    /* 8. After each execution report, kill-switch state may change. */
    if (account_blocks != NULL) {
        fprintf(stderr, "kill switch triggered\n");
        openpit_pretrade_destroy_account_block_list(account_blocks);
        account_blocks = NULL;
    }

    rc = 0;

cleanup:
    if (reservation != NULL) {
        if (!reservation_committed) {
            openpit_pretrade_pre_trade_reservation_rollback(reservation);
        }
        openpit_destroy_pretrade_pre_trade_reservation(reservation);
    }
    if (request != NULL) {
        openpit_destroy_pretrade_pre_trade_request(request);
    }
    if (execute_rejects != NULL) {
        openpit_pretrade_destroy_reject_list(execute_rejects);
    }
    if (start_rejects != NULL) {
        openpit_pretrade_destroy_reject_list(start_rejects);
    }
    openpit_destroy_engine(engine);
    openpit_destroy_engine_builder(builder);
    return rc;
}
```

Example flow:

1. configure built-in policies
2. build the engine
3. create an order as `OpenPitOrder` POD payload
4. run start-stage checks
5. run main-stage checks
6. commit the reservation after the venue accepts the order
7. apply one execution report built as a POD payload
8. inspect whether post-trade state triggered a kill switch

For the full type and ownership reference, use the C manual:
[docs/c-api/index.md](https://github.com/openpitkit/pit/blob/main/docs/c-api/index.md).

## Errors

Business rejects are returned through `OpenPitPretradeRejectList*` and
related APIs such as `openpit_engine_start_pre_trade(...)` and
`openpit_pretrade_pre_trade_request_execute(...)`.

Input validation errors and API misuse are reported through two channels:

- thread-local: call `openpit_get_last_error()` after a function returns a failure
  status
- out-pointer: pass `OpenPitSharedString **out_error` where supported; read the
  message with `openpit_shared_string_view()`, then release with
  `openpit_destroy_shared_string()`
- read `OpenPitPretradeRejectCode` for stable machine-readable business reject categories
- use the C docs for ownership and lifetime rules of every returned pointer

The example above uses both channels:

- `report_last_error(...)` for APIs that only use `openpit_get_last_error()`
- `report_out_error(...)` for APIs that expose `OpenPitSharedString **out_error`
- `print_reject(...)` and `print_reject_list(...)` for business-level rejects

## Local Testing

<details>
<summary>POSIX (Linux, macOS, etc)</summary>

Dependencies: a C compiler and
[Python 3.10+](https://www.python.org/downloads/). [Just](https://just.systems/)
is optional but recommended.

Recommended local flow:

With [Just](https://just.systems/):

```bash
just gen-api-c
just build-debug
cc -xc -fsyntax-only -include bindings/c/openpit.h /dev/null
```

Manual:

```bash
python3 scripts/generate_api_c.py
cargo build --workspace
cc -xc -fsyntax-only -include bindings/c/openpit.h /dev/null
```

</details>

<details>
<summary>Windows</summary>

Install [rustup](https://rustup.rs/), target `x86_64-pc-windows-msvc`,
[Python 3.10+](https://www.python.org/downloads/), and
[LLVM](https://github.com/llvm/llvm-project/releases) for `clang`.
[Just](https://just.systems/) is optional but recommended.

With [Just](https://just.systems/):

```powershell
rustup target add x86_64-pc-windows-msvc
just gen-api-c
just build-debug
just test-c-examples
```

Manual:

```powershell
rustup target add x86_64-pc-windows-msvc
python scripts\generate_api_c.py
cargo build --workspace --target x86_64-pc-windows-msvc
clang -x c -fsyntax-only -include bindings/c/openpit.h NUL
```

</details>

For full build/test command matrix (manual and `just`), see [the repository README](https://github.com/openpitkit/pit/blob/main/README.md).
