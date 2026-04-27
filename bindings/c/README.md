# Pit (Pre-trade Integrity Toolkit) for C

<!-- markdownlint-disable MD013 -->
[![Verify](https://github.com/openpitkit/pit/actions/workflows/verify.yml/badge.svg)](https://github.com/openpitkit/pit/actions/workflows/verify.yml) [![Release](https://github.com/openpitkit/pit/actions/workflows/release.yml/badge.svg)](https://github.com/openpitkit/pit/actions/workflows/release.yml) [![C API](https://img.shields.io/badge/C%20API-header%20%2B%20docs-4b5563)](../../docs/c-api/index.md) [![License](https://img.shields.io/badge/license-Apache%202.0-blue)](../../LICENSE)
<!-- markdownlint-enable MD013 -->

`pit.h` is an embeddable pre-trade risk SDK entrypoint for integrating
policy-driven risk checks into trading systems from C and other environments
that consume a C ABI.

For an overview and links to all resources, see
the project website [openpit.dev](https://openpit.dev/).
For full project documentation, see
[the repository README](https://github.com/openpitkit/pit/blob/main/README.md).
For conceptual and architectural pages, see
[the project wiki](https://github.com/openpitkit/pit/wiki).
For the split C reference manual, see
[the C API docs](https://github.com/openpitkit/pit/blob/main/docs/c-api/index.md).

## Versioning Policy (Pre‑1.0)

Until Pit reaches a stable `1.0` release, the project follows a relaxed
interpretation of Semantic Versioning.

During this phase:

- `PATCH` releases are used for bug fixes and small internal corrections.
- `MINOR` releases may introduce new features **and may also change the public
  interface**.

This means that breaking API changes can appear in minor releases before `1.0`.
Consumers of the library should take this into account when declaring
dependencies and consider using version constraints that tolerate API
evolution during the pre‑stable phase.

## Getting Started

Visit the
[C API documentation](https://github.com/openpitkit/pit/blob/main/docs/c-api/index.md).

## Install

For normal end-user installation, use the published
[GitHub release assets](https://github.com/openpitkit/pit/releases):

- `pit.h`
- `libpit_ffi.so` on Linux
- `libpit_ffi.dylib` on macOS
- `pit_ffi.dll` on Windows
- `LICENSE`
- `OWNERS`

If you need local development/debugging, clone this repository and generate the
header plus the reference docs.

With [Just](https://just.systems/):

```bash
just ffi-c-api
```

Manual:

```bash
python3 scripts/generate_api_c.py
```

If you need a workspace build:

With [Just](https://just.systems/):

```bash
just build
```

Manual:

```bash
cargo build --workspace
```

If you need only the release C runtime library:

```bash
cargo build -p pit-ffi --release
```

## Engine

### Overview

The engine evaluates an order through a deterministic pre-trade pipeline:

- `pit_engine_start_pre_trade(...)` runs start-stage policies and creates a
  deferred request
- `pit_pretrade_request_execute(...)` runs main-stage check policies
- `pit_pretrade_reservation_commit(...)` applies reserved state
- `pit_pretrade_reservation_rollback(...)` reverts reserved state
- `pit_engine_apply_execution_report(...)` updates post-trade policy state

Start-stage policies stop on the first reject. Main-stage policies aggregate
rejects and run rollback mutations in reverse order when any reject is
produced.

Built-in policies currently include:

- order validation
- P&L kill switch
- sliding-window rate limit
- per-settlement order size limits

The primary integration model is to build project-specific policies against the
public C API described in the manual: [the C API docs](https://github.com/openpitkit/pit/blob/main/docs/c-api/index.md).

There are two types of rejections: a full kill switch for the account and a
rejection of only the current request. This is useful in algorithmic trading
when automatic order submission must be halted until the situation is analyzed.

## Usage

```c
#include <stdbool.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "pit.h"

static int report_last_error(const char *context) {
    const char *message = pit_get_last_error();
    fprintf(stderr, "%s: %s\n", context, message != NULL ? message : "<no error>");
    return 1;
}

/* Prints the error message from a PitSharedString, then destroys the handle. */
static int report_out_error(const char *context, PitSharedString *error) {
    char buf[512];
    PitStringView view = pit_shared_string_view(error);
    size_t n = view.len < sizeof(buf) - 1 ? view.len : sizeof(buf) - 1;
    if (view.ptr != NULL && n > 0) {
        memcpy(buf, view.ptr, n);
    }
    buf[n] = '\0';
    fprintf(stderr, "%s: %s\n", context, buf);
    pit_destroy_shared_string(error);
    return 1;
}

static int make_quantity(
    int64_t mantissa,
    int32_t scale,
    PitParamQuantity *out
) {
    if (!pit_create_param_quantity(
            (PitParamDecimal){
                .mantissa_lo = mantissa,
                .mantissa_hi = mantissa < 0 ? -1 : 0,
                .scale = scale},
            out,
            NULL)) {
        return report_last_error("pit_create_param_quantity");
    }
    return 0;
}

static int make_volume(int64_t mantissa, int32_t scale, PitParamVolume *out) {
    if (!pit_create_param_volume(
            (PitParamDecimal){
                .mantissa_lo = mantissa,
                .mantissa_hi = mantissa < 0 ? -1 : 0,
                .scale = scale},
            out,
            NULL)) {
        return report_last_error("pit_create_param_volume");
    }
    return 0;
}

static int make_price(int64_t mantissa, int32_t scale, PitParamPrice *out) {
    if (!pit_create_param_price(
            (PitParamDecimal){
                .mantissa_lo = mantissa,
                .mantissa_hi = mantissa < 0 ? -1 : 0,
                .scale = scale},
            out,
            NULL)) {
        return report_last_error("pit_create_param_price");
    }
    return 0;
}

static int make_pnl(int64_t mantissa, int32_t scale, PitParamPnl *out) {
    if (!pit_create_param_pnl(
            (PitParamDecimal){
                .mantissa_lo = mantissa,
                .mantissa_hi = mantissa < 0 ? -1 : 0,
                .scale = scale},
            out,
            NULL)) {
        return report_last_error("pit_create_param_pnl");
    }
    return 0;
}

static int make_fee(int64_t mantissa, int32_t scale, PitParamFee *out) {
    if (!pit_create_param_fee(
            (PitParamDecimal){
                .mantissa_lo = mantissa,
                .mantissa_hi = mantissa < 0 ? -1 : 0,
                .scale = scale},
            out,
            NULL)) {
        return report_last_error("pit_create_param_fee");
    }
    return 0;
}

static PitStringView make_string_view(const char *s) {
    return (PitStringView){
        .ptr = (const uint8_t *)s,
        .len = s != NULL ? strlen(s) : 0};
}

static const char *view_to_cstr(PitStringView view, char *buf, size_t cap) {
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

static void print_reject(const PitReject *reject) {
    char policy[128];
    char reason[256];
    char details[256];
    fprintf(
        stderr,
        "%s [%u]: %s: %s\n",
        view_to_cstr(pit_reject_get_policy(reject), policy, sizeof(policy)),
        (unsigned)pit_reject_get_code(reject),
        view_to_cstr(pit_reject_get_reason(reject), reason, sizeof(reason)),
        view_to_cstr(pit_reject_get_details(reject), details, sizeof(details)));
}

static void print_reject_list(const PitRejectList *rejects) {
    size_t i = 0;
    size_t len = pit_reject_list_len(rejects);
    for (i = 0; i < len; ++i) {
        const PitReject *item = pit_reject_list_get(rejects, i);
        if (item != NULL) {
            print_reject(item);
        }
    }
}

int main(void) {
    PitSharedString *error = NULL;
    int rc = 1;
    bool reservation_committed = false;

    /* 1. Configure policies. */
    PitPretradePoliciesPnlKillSwitchParam pnl_barrier = {0};
    PitPretradePoliciesOrderSizeLimitParam order_size_limit = {0};
    PitPretradeCheckPreTradeStartPolicy *validation_policy = NULL;
    PitPretradeCheckPreTradeStartPolicy *pnl_policy = NULL;
    PitPretradeCheckPreTradeStartPolicy *rate_limit_policy = NULL;
    PitPretradeCheckPreTradeStartPolicy *order_size_policy = NULL;
    PitEngineBuilder *builder = NULL;
    PitEngine *engine = NULL;
    PitPretradePreTradeRequest *request = NULL;
    PitPretradePreTradeReservation *reservation = NULL;
    PitRejectList *start_rejects = NULL;
    PitRejectList *execute_rejects = NULL;
    PitEngineApplyExecutionReportResult apply_result = {0};
    PitOrder order = {0};
    PitExecutionReport report = {0};

    pnl_barrier.settlement_asset = "USD";
    if (make_pnl(1000, 0, &pnl_barrier.barrier) != 0) {
        goto cleanup;
    }

    order_size_limit.settlement_asset = "USD";
    if (make_quantity(500, 0, &order_size_limit.max_quantity) != 0) {
        goto cleanup;
    }
    if (make_volume(100000, 0, &order_size_limit.max_notional) != 0) {
        goto cleanup;
    }

    validation_policy = pit_create_pretrade_order_validation_policy();
    if (validation_policy == NULL) {
        report_last_error("pit_create_pretrade_order_validation_policy");
        goto cleanup;
    }

    pnl_policy =
        pit_create_pretrade_pnl_killswitch_policy(&pnl_barrier, 1, &error);
    if (pnl_policy == NULL) {
        report_out_error("pit_create_pretrade_pnl_killswitch_policy", error);
        error = NULL;
        goto cleanup;
    }

    rate_limit_policy = pit_create_pretrade_rate_limit_policy(100, 1);
    if (rate_limit_policy == NULL) {
        report_last_error("pit_create_pretrade_rate_limit_policy");
        goto cleanup;
    }

    order_size_policy = pit_create_pretrade_order_size_limit_policy(
        &order_size_limit, 1, &error);
    if (order_size_policy == NULL) {
        report_out_error(
            "pit_create_pretrade_order_size_limit_policy", error);
        error = NULL;
        goto cleanup;
    }

    /* 2. Build the engine once during platform initialization. */
    builder = pit_create_engine_builder();
    if (builder == NULL) {
        report_last_error("pit_create_engine_builder");
        goto cleanup;
    }

    if (!pit_engine_builder_add_check_pre_trade_start_policy(
            builder, validation_policy, &error)) {
        report_out_error(
            "pit_engine_builder_add_check_pre_trade_start_policy (validation)",
            error);
        error = NULL;
        goto cleanup;
    }
    if (!pit_engine_builder_add_check_pre_trade_start_policy(
            builder, pnl_policy, &error)) {
        report_out_error(
            "pit_engine_builder_add_check_pre_trade_start_policy (pnl)",
            error);
        error = NULL;
        goto cleanup;
    }
    if (!pit_engine_builder_add_check_pre_trade_start_policy(
            builder, rate_limit_policy, &error)) {
        report_out_error(
            "pit_engine_builder_add_check_pre_trade_start_policy (rate limit)",
            error);
        error = NULL;
        goto cleanup;
    }
    if (!pit_engine_builder_add_check_pre_trade_start_policy(
            builder, order_size_policy, &error)) {
        report_out_error(
            "pit_engine_builder_add_check_pre_trade_start_policy (order size)",
            error);
        error = NULL;
        goto cleanup;
    }

    engine = pit_engine_builder_build(builder, &error);
    if (engine == NULL) {
        report_out_error("pit_engine_builder_build", error);
        error = NULL;
        goto cleanup;
    }

    /* 3. Build one order as a POD payload. */
    PitParamQuantity qty = {0};
    PitParamPrice px = {0};
    if (make_quantity(100, 0, &qty) != 0 || make_price(185, 0, &px) != 0) {
        goto cleanup;
    }
    order.operation.is_set = true;
    order.operation.value.instrument.underlying_asset = make_string_view("AAPL");
    order.operation.value.instrument.settlement_asset = make_string_view("USD");
    order.operation.value.side = PitParamSide_Buy;
    order.operation.value.trade_amount.kind = PitParamTradeAmountKind_Quantity;
    order.operation.value.trade_amount.value = qty._0;
    order.operation.value.price.value = px;
    order.operation.value.price.is_set = true;

    /* 4. Start-stage checks. */
    error = NULL;
    request = NULL;
    start_rejects = NULL;
    PitPretradeStatus start_status = pit_engine_start_pre_trade(
        engine, &order, &request, &start_rejects, &error);
    if (start_status == PitPretradeStatus_Error) {
        report_out_error("pit_engine_start_pre_trade", error);
        error = NULL;
        goto cleanup;
    }
    if (start_status == PitPretradeStatus_Rejected && start_rejects != NULL) {
        print_reject_list(start_rejects);
        goto cleanup;
    }

    /* 5. Main-stage checks. */
    error = NULL;
    execute_rejects = NULL;
    reservation = NULL;
    PitPretradeStatus exec_status = pit_pretrade_pre_trade_request_execute(
        request, &reservation, &execute_rejects, &error);
    if (exec_status == PitPretradeStatus_Error) {
        report_out_error("pit_pretrade_pre_trade_request_execute", error);
        error = NULL;
        goto cleanup;
    }
    if (exec_status == PitPretradeStatus_Rejected && execute_rejects != NULL) {
        print_reject_list(execute_rejects);
        goto cleanup;
    }

    /* 6. If the order was sent successfully, commit the reservation. */
    if (reservation == NULL) {
        report_last_error(
            "pit_pretrade_pre_trade_request_execute: no rejects and no reservation");
        goto cleanup;
    }
    pit_pretrade_pre_trade_reservation_commit(reservation);
    reservation_committed = true;

    /* 7. Feed back one execution report as a by-value POD payload. */
    report.operation.is_set = true;
    report.operation.value.instrument.underlying_asset = make_string_view("AAPL");
    report.operation.value.instrument.settlement_asset = make_string_view("USD");
    report.operation.value.side = PitParamSide_Buy;

    PitParamPnl pnl = {0};
    PitParamFee fee = {0};
    if (make_pnl(-50, 0, &pnl) != 0 || make_fee(34, 1, &fee) != 0) {
        goto cleanup;
    }
    report.financial_impact.is_set = true;
    report.financial_impact.value.pnl.is_set = true;
    report.financial_impact.value.pnl.value = pnl;
    report.financial_impact.value.fee.is_set = true;
    report.financial_impact.value.fee.value = fee;

    apply_result = pit_engine_apply_execution_report(engine, &report, &error);
    if (apply_result.is_error) {
        report_out_error("pit_engine_apply_execution_report", error);
        error = NULL;
        goto cleanup;
    }

    /* 8. After each execution report, kill-switch state may change. */
    if (apply_result.post_trade_result.kill_switch_triggered) {
        fprintf(stderr, "kill switch triggered\n");
    }

    rc = 0;

cleanup:
    if (reservation != NULL) {
        if (!reservation_committed) {
            pit_pretrade_pre_trade_reservation_rollback(reservation);
        }
        pit_destroy_pretrade_pre_trade_reservation(reservation);
    }
    if (request != NULL) {
        pit_destroy_pretrade_pre_trade_request(request);
    }
    if (execute_rejects != NULL) {
        pit_destroy_reject_list(execute_rejects);
    }
    if (start_rejects != NULL) {
        pit_destroy_reject_list(start_rejects);
    }
    pit_destroy_engine(engine);
    pit_destroy_engine_builder(builder);
    pit_destroy_pretrade_check_pre_trade_start_policy(order_size_policy);
    pit_destroy_pretrade_check_pre_trade_start_policy(rate_limit_policy);
    pit_destroy_pretrade_check_pre_trade_start_policy(pnl_policy);
    pit_destroy_pretrade_check_pre_trade_start_policy(validation_policy);
    return rc;
}
```

Example flow:

1. create built-in policies
2. build the engine
3. create an order as `PitOrder` POD payload
4. run start-stage checks
5. run main-stage checks
6. commit the reservation after the venue accepts the order
7. apply one execution report built as a POD payload
8. inspect whether post-trade state triggered a kill switch

For the full type and ownership reference, use the generated C manual:
[docs/c-api/index.md](https://github.com/openpitkit/pit/blob/main/docs/c-api/index.md).

## Errors

Business rejects are returned through `PitRejectList*` and related APIs such
as `pit_engine_start_pre_trade(...)` and
`pit_pretrade_pre_trade_request_execute(...)`.

Input validation errors and API misuse are reported through two channels:

- thread-local: call `pit_get_last_error()` after a function returns a failure
  status
- out-pointer: pass `PitSharedString **out_error` where supported; read the
  message with `pit_shared_string_view()`, then release with
  `pit_destroy_shared_string()`
- read `PitRejectCode` for stable machine-readable business reject categories
- use the generated C docs for ownership and lifetime rules of every returned
  pointer

The example above uses both channels:

- `report_last_error(...)` for APIs that only use `pit_get_last_error()`
- `report_out_error(...)` for APIs that expose `PitSharedString **out_error`
- `print_reject(...)` and `print_reject_list(...)` for business-level rejects

## Local Testing

Recommended local flow:

With [Just](https://just.systems/):

```bash
just ffi-c-api
just build
cc -xc -fsyntax-only -include bindings/c/pit.h /dev/null
```

Manual:

```bash
python3 scripts/generate_api_c.py
cargo build --workspace
cc -xc -fsyntax-only -include bindings/c/pit.h /dev/null
```

For full build/test command matrix (manual and `just`), see [the repository README](https://github.com/openpitkit/pit/blob/main/README.md).
