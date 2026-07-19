# Marketdata

<!-- markdownlint-disable MD013 MD024 -->

[Back to index](index.md)

## `OpenPitInstrumentId`

Stable instrument identifier for all FFI payloads.

```c
typedef uint64_t OpenPitInstrumentId;
```

## `OpenPitMarketDataInstrumentId`

Backwards-compatible market-data spelling of [`OpenPitInstrumentId`].

```c
typedef OpenPitInstrumentId OpenPitMarketDataInstrumentId;
```

## `OpenPitMarketDataQuote`

Market snapshot value passed across the FFI boundary.

Every field is optional (`is_set == false` means the producer did not publish
that field). Mirrors [`Quote`].

```c
typedef struct OpenPitMarketDataQuote {
    OpenPitParamPriceOptional mark;
    OpenPitParamPriceOptional bid;
    OpenPitParamPriceOptional ask;
} OpenPitMarketDataQuote;
```

## `openpit_create_marketdata_quote`

Returns an empty quote with every field unset.

This function never fails.

```c
OpenPitMarketDataQuote openpit_create_marketdata_quote(void);
```

## `OpenPitMarketDataQuoteTtl`

Service-wide / per-instrument quote lifetime for FFI payloads.

`is_infinite == true` means quotes never expire on their own. Otherwise the
quote expires `secs` + `nanos` after the push that wrote it. Mirrors
[`QuoteTtl`].

```c
typedef struct OpenPitMarketDataQuoteTtl {
    uint64_t secs;
    uint32_t nanos;
    bool is_infinite;
} OpenPitMarketDataQuoteTtl;
```

## `openpit_create_marketdata_quote_ttl_infinite`

Builds an infinite quote lifetime.

This function never fails.

```c
OpenPitMarketDataQuoteTtl openpit_create_marketdata_quote_ttl_infinite(void);
```

## `openpit_create_marketdata_quote_ttl_within`

Builds a finite quote lifetime of `secs` seconds plus `nanos` nanoseconds.

This function never fails.

```c
OpenPitMarketDataQuoteTtl openpit_create_marketdata_quote_ttl_within(
    uint64_t secs,
    uint32_t nanos
);
```

## `OpenPitMarketDataQuoteResolution`

Selects which quote buckets a read will consult, in order.

When the more-specific bucket has no quote, the read falls through to the next
bucket permitted by this value.

```c
typedef uint8_t OpenPitMarketDataQuoteResolution;
```

## `OPENPIT_MARKET_DATA_QUOTE_RESOLUTION_ACCOUNT_ONLY`

Consult only the per-account bucket for the reading account.

```c
#define OPENPIT_MARKET_DATA_QUOTE_RESOLUTION_ACCOUNT_ONLY \
    ((OpenPitMarketDataQuoteResolution) 0)
```

## `OPENPIT_MARKET_DATA_QUOTE_RESOLUTION_ACCOUNT_THEN_GROUP`

Consult the per-account bucket, then fall back to the account's group bucket
when the account bucket has no quote.

```c
#define OPENPIT_MARKET_DATA_QUOTE_RESOLUTION_ACCOUNT_THEN_GROUP \
    ((OpenPitMarketDataQuoteResolution) 1)
```

## `OPENPIT_MARKET_DATA_QUOTE_RESOLUTION_ACCOUNT_THEN_GROUP_THEN_DEFAULT`

Consult the per-account bucket, then the account's group bucket, then the
default-group ("everyone-else") bucket, in that order.

```c
#define OPENPIT_MARKET_DATA_QUOTE_RESOLUTION_ACCOUNT_THEN_GROUP_THEN_DEFAULT \
    ((OpenPitMarketDataQuoteResolution) 2)
```

## `OpenPitMarketDataGetStatus`

Result of a market-data read.

```c
typedef uint8_t OpenPitMarketDataGetStatus;
/**
 * A usable quote was found; `out_quote` was written.
 */
#define OpenPitMarketDataGetStatus_Found ((OpenPitMarketDataGetStatus) 0)
/**
 * The instrument is registered but no usable quote is available (never pushed
 * or cleared).
 */
#define OpenPitMarketDataGetStatus_Unavailable ((OpenPitMarketDataGetStatus) 1)
/**
 * The instrument id is not registered with the service.
 */
#define OpenPitMarketDataGetStatus_UnknownInstrument \
    ((OpenPitMarketDataGetStatus) 2)
/**
 * The selected quote exists but aged past its effective TTL; the stale quote
 * was written to `out_quote`.
 */
#define OpenPitMarketDataGetStatus_QuoteExpired ((OpenPitMarketDataGetStatus) 3)
/**
 * The supplied quote-resolution selector is invalid.
 */
#define OpenPitMarketDataGetStatus_Error ((OpenPitMarketDataGetStatus) 255)
```

## `OpenPitMarketDataRegisterStatus`

Result of a market-data registration or update.

Each operation returns only the subset of variants it can produce; see the
per-function contract for the variants it may report.

```c
typedef uint8_t OpenPitMarketDataRegisterStatus;
/**
 * The operation succeeded; any associated output was written.
 */
#define OpenPitMarketDataRegisterStatus_Ok ((OpenPitMarketDataRegisterStatus) 0)
/**
 * The instrument is already registered with the service.
 */
#define OpenPitMarketDataRegisterStatus_AlreadyRegistered \
    ((OpenPitMarketDataRegisterStatus) 1)
/**
 * The supplied id is already registered with the service.
 */
#define OpenPitMarketDataRegisterStatus_DuplicateId \
    ((OpenPitMarketDataRegisterStatus) 2)
/**
 * The supplied instrument is already registered under a different id.
 */
#define OpenPitMarketDataRegisterStatus_DuplicateInstrument \
    ((OpenPitMarketDataRegisterStatus) 3)
/**
 * The supplied instrument id is not registered with the service.
 */
#define OpenPitMarketDataRegisterStatus_UnknownInstrument \
    ((OpenPitMarketDataRegisterStatus) 4)
/**
 * A boundary failure occurred (null pointer or an invalid payload); when
 * `out_error` is not null, a caller-owned error string was written.
 */
#define OpenPitMarketDataRegisterStatus_Error \
    ((OpenPitMarketDataRegisterStatus) 5)
/**
 * A targeted push (`push_for` / `push_for_patch`) was called with both the
 * account list and the group list empty.
 */
#define OpenPitMarketDataRegisterStatus_NoTarget \
    ((OpenPitMarketDataRegisterStatus) 6)
```

## `OpenPitMarketDataService`

Opaque shared market-data service handle.

Duplicate it with `openpit_marketdata_service_clone` to hand the same service to
both a policy and a feed.

```c
typedef struct OpenPitMarketDataService OpenPitMarketDataService;
```

## `OpenPitMarketDataAccountGroupResolver`

Resolves the reading account's group on demand.

Returns `true` and writes the group id to `out_account_group_id` when the
account belongs to a group; returns `false` when it has none. Invoked lazily by
`openpit_marketdata_service_get` — only when the resolution mode would consult
the group or default-group bucket and the per-account bucket has no quote.

The function pointer must not be null; see the contract on
`openpit_marketdata_service_get`.

```c
typedef bool (*OpenPitMarketDataAccountGroupResolver)(
    void * user_data,
    OpenPitParamAccountGroupId * out_account_group_id
);
```

## `openpit_create_marketdata_service`

Creates a market-data service with the chosen synchronization mode.

`mode` uses the same byte convention as `openpit_create_engine_builder`:

- `0` = `None` (no internal synchronization: no-op locks, zero overhead,
  single-threaded use only);
- `1` = `Full` (full synchronization: real `RwLock`, safe for a concurrent
  quote feed).

Only `None` (0) and `Full` (1) are valid for a market-data service. Passing `2`
(`Account`) or any other byte is an error.

Success:

- returns a non-null caller-owned `OpenPitMarketDataService` handle.

Error:

- returns null when `mode` is not `0` or `1`; if `out_error` is not null,
  writes a caller-owned `OpenPitSharedString` error handle that MUST be
  released with `openpit_destroy_shared_string`.

Cleanup:

- the returned service handle MUST be released with
  `openpit_destroy_marketdata_service` exactly once.

```c
OpenPitMarketDataService * openpit_create_marketdata_service(
    uint8_t mode,
    OpenPitMarketDataQuoteTtl default_ttl,
    OpenPitOutError out_error
);
```

## `openpit_destroy_marketdata_service`

Releases a market-data service handle.

Contract:

- passing null is allowed;
- releases this handle; the underlying service stays alive while other handles
  to it exist;
- after this call the pointer is invalid;
- this function always succeeds.

```c
void openpit_destroy_marketdata_service(
    OpenPitMarketDataService * service
);
```

## `openpit_marketdata_service_clone`

Returns a new handle referring to the same market-data service.

Use this to hand the same service to a policy and a feed.

Success:

- returns a non-null caller-owned handle to the same service.

Error:

- returns null when `service` is null.

Cleanup:

- the returned handle MUST be released with
  `openpit_destroy_marketdata_service` exactly once.

```c
OpenPitMarketDataService * openpit_marketdata_service_clone(
    const OpenPitMarketDataService * service
);
```

## `openpit_marketdata_service_register`

Registers `instrument` with the service-wide default TTL.

Status:

- `Ok`: registered; the auto-assigned id was written to `out_id`;
- `AlreadyRegistered`: the instrument is already registered;
- `Error`: `service`/`out_id` is null or the instrument payload is invalid; if
  `out_error` is not null, a caller-owned `OpenPitSharedString` error handle
  was written that MUST be released with `openpit_destroy_shared_string`.

```c
OpenPitMarketDataRegisterStatus openpit_marketdata_service_register(
    const OpenPitMarketDataService * service,
    const OpenPitInstrument * instrument,
    OpenPitMarketDataInstrumentId * out_id,
    OpenPitOutError out_error
);
```

## `openpit_marketdata_service_register_with_ttl`

Registers `instrument` with a per-instrument TTL override.

Behaves like `openpit_marketdata_service_register` otherwise.

```c
OpenPitMarketDataRegisterStatus openpit_marketdata_service_register_with_ttl(
    const OpenPitMarketDataService * service,
    const OpenPitInstrument * instrument,
    OpenPitMarketDataQuoteTtl ttl,
    OpenPitMarketDataInstrumentId * out_id,
    OpenPitOutError out_error
);
```

## `openpit_marketdata_service_register_with_id`

Registers `instrument` under the caller-supplied `instrument_id` with the
service-wide default TTL.

Status:

- `Ok`: registered; `instrument_id` was written to `out_id`;
- `DuplicateInstrument`: the instrument name is already registered under a
  different id;
- `DuplicateId`: `instrument_id` is already registered;
- `Error`: `service`/`out_id` is null or the instrument payload is invalid; if
  `out_error` is not null, a caller-owned `OpenPitSharedString` error handle
  was written.

```c
OpenPitMarketDataRegisterStatus openpit_marketdata_service_register_with_id(
    const OpenPitMarketDataService * service,
    const OpenPitInstrument * instrument,
    OpenPitMarketDataInstrumentId instrument_id,
    OpenPitMarketDataInstrumentId * out_id,
    OpenPitOutError out_error
);
```

## `openpit_marketdata_service_register_with_id_and_ttl`

Registers `instrument` under the caller-supplied `instrument_id` with a
per-instrument TTL override.

Behaves like `openpit_marketdata_service_register_with_id` otherwise.

```c
OpenPitMarketDataRegisterStatus
openpit_marketdata_service_register_with_id_and_ttl(
    const OpenPitMarketDataService * service,
    const OpenPitInstrument * instrument,
    OpenPitMarketDataInstrumentId instrument_id,
    OpenPitMarketDataQuoteTtl ttl,
    OpenPitMarketDataInstrumentId * out_id,
    OpenPitOutError out_error
);
```

## `openpit_marketdata_service_set_account_ttl`

Pins the service-level TTL for `account_id`.

Applies to every instrument for `account_id` that does not have a more specific
instrument × account TTL cell.

Contract:

- `service` must be a valid non-null handle; passing null aborts the call;
- this function never fails.

```c
void openpit_marketdata_service_set_account_ttl(
    const OpenPitMarketDataService * service,
    OpenPitParamAccountId account_id,
    OpenPitMarketDataQuoteTtl ttl
);
```

## `openpit_marketdata_service_clear_account_ttl`

Reverts the service-level TTL for `account_id` back to "inherit".

Contract:

- `service` must be a valid non-null handle; passing null aborts the call;
- this function never fails.

```c
void openpit_marketdata_service_clear_account_ttl(
    const OpenPitMarketDataService * service,
    OpenPitParamAccountId account_id
);
```

## `openpit_marketdata_service_set_account_group_ttl`

Pins the service-level TTL for `account_group_id`.

Pass `OPENPIT_DEFAULT_ACCOUNT_GROUP` (`0`) to set the service-level
default-group TTL.

Contract:

- `service` must be a valid non-null handle; passing null aborts the call;
- this function never fails.

```c
void openpit_marketdata_service_set_account_group_ttl(
    const OpenPitMarketDataService * service,
    OpenPitParamAccountGroupId account_group_id,
    OpenPitMarketDataQuoteTtl ttl
);
```

## `openpit_marketdata_service_clear_account_group_ttl`

Reverts the service-level TTL for `account_group_id` back to "inherit".

Pass `OPENPIT_DEFAULT_ACCOUNT_GROUP` (`0`) to clear the default-group TTL.

Contract:

- `service` must be a valid non-null handle; passing null aborts the call;
- this function never fails.

```c
void openpit_marketdata_service_clear_account_group_ttl(
    const OpenPitMarketDataService * service,
    OpenPitParamAccountGroupId account_group_id
);
```

## `openpit_marketdata_service_set_instrument_ttl`

Updates the instrument-level TTL for an already-registered instrument.

This replaces the removed `openpit_marketdata_service_set_ttl`.

Status:

- `Ok`: updated; the new TTL takes effect on the next read;
- `UnknownInstrument`: `instrument_id` is not registered.

Contract:

- `service` must be a valid non-null handle; passing null aborts the call.

```c
OpenPitMarketDataRegisterStatus openpit_marketdata_service_set_instrument_ttl(
    const OpenPitMarketDataService * service,
    OpenPitMarketDataInstrumentId instrument_id,
    OpenPitMarketDataQuoteTtl ttl
);
```

## `openpit_marketdata_service_clear_instrument_ttl`

Reverts the instrument-level TTL for `instrument_id` back to "inherit".

Status:

- `Ok`: cleared;
- `UnknownInstrument`: `instrument_id` is not registered.

Contract:

- `service` must be a valid non-null handle; passing null aborts the call.

```c
OpenPitMarketDataRegisterStatus openpit_marketdata_service_clear_instrument_ttl(
    const OpenPitMarketDataService * service,
    OpenPitMarketDataInstrumentId instrument_id
);
```

## `openpit_marketdata_service_set_instrument_account_ttl`

Pins the instrument × account TTL cell for `(instrument_id, account_id)`.

This is the highest-priority TTL tier (overrides all group and instrument-level
cells for this account).

Status:

- `Ok`: pinned;
- `UnknownInstrument`: `instrument_id` is not registered.

Contract:

- `service` must be a valid non-null handle; passing null aborts the call.

```c
OpenPitMarketDataRegisterStatus
openpit_marketdata_service_set_instrument_account_ttl(
    const OpenPitMarketDataService * service,
    OpenPitMarketDataInstrumentId instrument_id,
    OpenPitParamAccountId account_id,
    OpenPitMarketDataQuoteTtl ttl
);
```

## `openpit_marketdata_service_clear_instrument_account_ttl`

Reverts the instrument × account TTL cell for `(instrument_id, account_id)` back
to "inherit".

Status:

- `Ok`: cleared;
- `UnknownInstrument`: `instrument_id` is not registered.

Contract:

- `service` must be a valid non-null handle; passing null aborts the call.

```c
OpenPitMarketDataRegisterStatus
openpit_marketdata_service_clear_instrument_account_ttl(
    const OpenPitMarketDataService * service,
    OpenPitMarketDataInstrumentId instrument_id,
    OpenPitParamAccountId account_id
);
```

## `openpit_marketdata_service_set_instrument_account_group_ttl`

Pins the instrument × group TTL cell for `(instrument_id, account_group_id)`.

Pass `OPENPIT_DEFAULT_ACCOUNT_GROUP` (`0`) for `account_group_id` to target the
instrument's default-group TTL cell.

Status:

- `Ok`: pinned;
- `UnknownInstrument`: `instrument_id` is not registered.

Contract:

- `service` must be a valid non-null handle; passing null aborts the call.

```c
OpenPitMarketDataRegisterStatus
openpit_marketdata_service_set_instrument_account_group_ttl(
    const OpenPitMarketDataService * service,
    OpenPitMarketDataInstrumentId instrument_id,
    OpenPitParamAccountGroupId account_group_id,
    OpenPitMarketDataQuoteTtl ttl
);
```

## `openpit_marketdata_service_clear_instrument_account_group_ttl`

Reverts the instrument × group TTL cell for `(instrument_id, account_group_id)`
back to "inherit".

Pass `OPENPIT_DEFAULT_ACCOUNT_GROUP` (`0`) for `account_group_id` to clear the
instrument's default-group TTL cell.

Status:

- `Ok`: cleared;
- `UnknownInstrument`: `instrument_id` is not registered.

Contract:

- `service` must be a valid non-null handle; passing null aborts the call.

```c
OpenPitMarketDataRegisterStatus
openpit_marketdata_service_clear_instrument_account_group_ttl(
    const OpenPitMarketDataService * service,
    OpenPitMarketDataInstrumentId instrument_id,
    OpenPitParamAccountGroupId account_group_id
);
```

## `openpit_marketdata_service_clear`

Clears the stored quote for `instrument_id`.

Contract:

- `service` must be a valid non-null handle; passing null aborts the call;
- a no-op if `instrument_id` is not registered;
- this function never fails.

```c
void openpit_marketdata_service_clear(
    const OpenPitMarketDataService * service,
    OpenPitMarketDataInstrumentId instrument_id
);
```

## `openpit_marketdata_service_push`

Publishes a quote for `instrument_id`, replacing the entire stored snapshot.

Status:

- `Ok`: the snapshot was replaced;
- `UnknownInstrument`: `instrument_id` is not registered;
- `Error`: `service` is null or `quote` carries an invalid price; if
  `out_error` is not null, a caller-owned `OpenPitSharedString` error handle
  was written.

```c
OpenPitMarketDataRegisterStatus openpit_marketdata_service_push(
    const OpenPitMarketDataService * service,
    OpenPitMarketDataInstrumentId instrument_id,
    OpenPitMarketDataQuote quote,
    OpenPitOutError out_error
);
```

## `openpit_marketdata_service_push_patch`

Publishes a partial update for `instrument_id`, merging it into the stored
snapshot.

Behaves like `openpit_marketdata_service_push` otherwise.

```c
OpenPitMarketDataRegisterStatus openpit_marketdata_service_push_patch(
    const OpenPitMarketDataService * service,
    OpenPitMarketDataInstrumentId instrument_id,
    OpenPitMarketDataQuote quote,
    OpenPitOutError out_error
);
```

## `openpit_marketdata_service_push_for`

Publishes a quote for `instrument_id` into the per-account bucket of every
account in `account_ids` and the per-group bucket of every group in
`account_group_ids`, replacing each target's snapshot.

A null pointer with a matching length of `0` is a valid empty list.

Status:

- `Ok`: all targets were written;
- `UnknownInstrument`: `instrument_id` is not registered;
- `NoTarget`: both `account_ids` and `account_group_ids` are empty; use
  `openpit_marketdata_service_push` to write the default bucket;
- `Error`: `service` is null or `quote` carries an invalid price; if
  `out_error` is not null, a caller-owned `OpenPitSharedString` error handle
  was written.

```c
OpenPitMarketDataRegisterStatus openpit_marketdata_service_push_for(
    const OpenPitMarketDataService * service,
    OpenPitMarketDataInstrumentId instrument_id,
    OpenPitMarketDataQuote quote,
    const OpenPitParamAccountId * account_ids,
    size_t account_ids_len,
    const OpenPitParamAccountGroupId * account_group_ids,
    size_t account_group_ids_len,
    OpenPitOutError out_error
);
```

## `openpit_marketdata_service_push_for_patch`

Publishes a partial update for `instrument_id` into the per-account bucket of
every account in `account_ids` and the per-group bucket of every group in
`account_group_ids`, merging independently into each target's existing snapshot.

Behaves like `openpit_marketdata_service_push_for` otherwise.

```c
OpenPitMarketDataRegisterStatus openpit_marketdata_service_push_for_patch(
    const OpenPitMarketDataService * service,
    OpenPitMarketDataInstrumentId instrument_id,
    OpenPitMarketDataQuote quote,
    const OpenPitParamAccountId * account_ids,
    size_t account_ids_len,
    const OpenPitParamAccountGroupId * account_group_ids,
    size_t account_group_ids_len,
    OpenPitOutError out_error
);
```

## `openpit_marketdata_service_push_by_instrument`

Publishes a quote for `instrument`, replacing the stored snapshot.

If `instrument` is unregistered, a named slot is created with the
service-default TTL.

Success:

- returns `true` and writes the instrument's id to `out_id`.

Error:

- returns `false` when `service`/`out_id` is null, the instrument payload is
  invalid, or `quote` carries an invalid price;
- if `out_error` is not null, writes a caller-owned `OpenPitSharedString`
  error handle.

```c
bool openpit_marketdata_service_push_by_instrument(
    const OpenPitMarketDataService * service,
    const OpenPitInstrument * instrument,
    OpenPitMarketDataQuote quote,
    OpenPitMarketDataInstrumentId * out_id,
    OpenPitOutError out_error
);
```

## `openpit_marketdata_service_push_by_instrument_patch`

Publishes a partial update for `instrument`, merging it into the stored
snapshot.

Behaves like `openpit_marketdata_service_push_by_instrument` otherwise.

```c
bool openpit_marketdata_service_push_by_instrument_patch(
    const OpenPitMarketDataService * service,
    const OpenPitInstrument * instrument,
    OpenPitMarketDataQuote quote,
    OpenPitMarketDataInstrumentId * out_id,
    OpenPitOutError out_error
);
```

## `openpit_marketdata_service_get`

Reads the latest quote for `(instrument_id, account_id)` under the given
resolution.

`resolve_account_group` is a **required** callback that supplies the reading
account's group **lazily** — it is invoked only when the resolution mode would
consult a group or default-group bucket and the per-account bucket has no quote.
The callback receives the caller-supplied `user_data` context pointer and, when
the account belongs to a group, writes the group id to `out_account_group_id`
and returns `true`; when the account has no group it returns `false`. Pass
`OPENPIT_DEFAULT_ACCOUNT_GROUP` (`0`) to target the default group bucket.

`resolution` controls which buckets are consulted, in order, when the
more-specific bucket has no quote.

Status:

- `Found`: a usable quote was written to `out_quote`;
- `Unavailable`: registered but no usable quote (never pushed or cleared);
- `UnknownInstrument`: `instrument_id` is not registered;
- `QuoteExpired`: selected quote aged past TTL; the stale quote was written to
  `out_quote`;
- `Error`: `resolution` is not one of the documented selector constants.

Contract:

- `service`, `resolve_account_group`, and `out_quote` must be valid non-null
  pointers; passing null for any of them aborts the call.

```c
OpenPitMarketDataGetStatus openpit_marketdata_service_get(
    const OpenPitMarketDataService * service,
    OpenPitMarketDataInstrumentId instrument_id,
    OpenPitParamAccountId account_id,
    OpenPitMarketDataAccountGroupResolver resolve_account_group,
    void * user_data,
    OpenPitMarketDataQuoteResolution resolution,
    OpenPitMarketDataQuote * out_quote
);
```

## `openpit_marketdata_service_resolve`

Resolves `instrument` to its registered id.

Success:

- returns `true` and writes the id to `out_id` when `instrument` is registered
  by name;
- returns `false` (without writing `out_id`) when the instrument is not
  registered, the instrument payload is invalid, or `service`/`out_id` is
  null.

This call does not use `out_error`: a `false` result simply means "not
resolved".

```c
bool openpit_marketdata_service_resolve(
    const OpenPitMarketDataService * service,
    const OpenPitInstrument * instrument,
    OpenPitMarketDataInstrumentId * out_id
);
```
