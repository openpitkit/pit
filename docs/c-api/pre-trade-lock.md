# Pre Trade Lock

<!-- markdownlint-disable MD013 MD024 -->

[Back to index](index.md)

## `OpenPitPretradePreTradeLock`

Opaque pre-trade lock handle.

```c
typedef struct OpenPitPretradePreTradeLock OpenPitPretradePreTradeLock;
```

## `OpenPitPretradePreTradeLockPricesView`

```c
typedef struct OpenPitPretradePreTradeLockPricesView {
    const OpenPitParamPrice * ptr;
    size_t len;
} OpenPitPretradePreTradeLockPricesView;
```

## `OpenPitPretradePreTradeLockPrices`

Caller-owned list of prices stored under a lock group.

```c
typedef struct OpenPitPretradePreTradeLockPrices
    OpenPitPretradePreTradeLockPrices;
```

## `OpenPitPretradePreTradeLockPricesStatus`

```c
typedef uint8_t OpenPitPretradePreTradeLockPricesStatus;
#define OpenPitPretradePreTradeLockPricesStatus_Error \
    ((OpenPitPretradePreTradeLockPricesStatus) 0)
#define OpenPitPretradePreTradeLockPricesStatus_Empty \
    ((OpenPitPretradePreTradeLockPricesStatus) 1)
#define OpenPitPretradePreTradeLockPricesStatus_One \
    ((OpenPitPretradePreTradeLockPricesStatus) 2)
#define OpenPitPretradePreTradeLockPricesStatus_List \
    ((OpenPitPretradePreTradeLockPricesStatus) 3)
```

## `OpenPitPretradePreTradeLockEntry`

A single `(policy_group_id, price)` record exchanged across the C boundary.

```c
typedef struct OpenPitPretradePreTradeLockEntry {
    uint16_t policy_group_id;
    OpenPitParamPrice price;
} OpenPitPretradePreTradeLockEntry;
```

## `openpit_create_pretrade_pre_trade_lock`

Allocates an empty lock.

Success:

- always returns a non-null caller-owned handle.

Cleanup:

- the caller MUST release the returned handle with
  `openpit_destroy_pretrade_pre_trade_lock` exactly once.

```c
OpenPitPretradePreTradeLock * openpit_create_pretrade_pre_trade_lock(void);
```

## `openpit_destroy_pretrade_pre_trade_lock`

Releases a lock handle.

Contract:

- passing null is allowed;
- after this call the pointer is invalid;
- this function always succeeds.

```c
void openpit_destroy_pretrade_pre_trade_lock(
    OpenPitPretradePreTradeLock * handle
);
```

## `openpit_pretrade_pre_trade_lock_clone`

Returns a deep copy of `lock`.

Success:

- returns a non-null caller-owned handle independent of `lock`.

Error:

- returns null when `lock` is null.

Cleanup:

- the caller MUST release the returned handle with
  `openpit_destroy_pretrade_pre_trade_lock` exactly once.

```c
OpenPitPretradePreTradeLock * openpit_pretrade_pre_trade_lock_clone(
    const OpenPitPretradePreTradeLock * lock
);
```

## `openpit_pretrade_pre_trade_lock_len`

Total number of stored prices across all groups.

`lock` must be a valid non-null handle. Passing null aborts the process.

```c
size_t openpit_pretrade_pre_trade_lock_len(
    const OpenPitPretradePreTradeLock * lock
);
```

## `openpit_pretrade_pre_trade_lock_is_empty`

Returns `true` when the lock carries no price records.

`lock` must be a valid non-null handle. Passing null aborts the process.

```c
bool openpit_pretrade_pre_trade_lock_is_empty(
    const OpenPitPretradePreTradeLock * lock
);
```

## `openpit_pretrade_pre_trade_lock_push`

Appends `price` under `policy_group_id`.

Success:

- returns `true`; the lock now carries one extra record for `policy_group_id`.

Error:

- returns `false` when `lock` is null or when `price` fails domain validation;
- if `out_error` is not null, writes a caller-owned `OpenPitSharedString`
  error handle that MUST be released with `openpit_destroy_shared_string`.

```c
bool openpit_pretrade_pre_trade_lock_push(
    OpenPitPretradePreTradeLock * lock,
    uint16_t policy_group_id,
    OpenPitParamPrice price,
    OpenPitOutError out_error
);
```

## `openpit_pretrade_pre_trade_lock_push_many`

Appends every `(policy_group_id, price)` record from `entries` into `lock`.

`entries_ptr`/`entries_len` describe an array of
`OpenPitPretradePreTradeLockEntry`. A zero length is allowed and leaves the lock
unchanged regardless of `entries_ptr`.

Success:

- returns `true`; every record has been appended in input order.

Error:

- returns `false` when `lock` is null, when `entries_ptr` is null while
  `entries_len` is non-zero, or when any price fails domain validation; on the
  first invalid price no record is appended;
- if `out_error` is not null, writes a caller-owned `OpenPitSharedString`
  error handle that MUST be released with `openpit_destroy_shared_string`.

```c
bool openpit_pretrade_pre_trade_lock_push_many(
    OpenPitPretradePreTradeLock * lock,
    const OpenPitPretradePreTradeLockEntry * entries_ptr,
    size_t entries_len,
    OpenPitOutError out_error
);
```

## `openpit_create_pretrade_pre_trade_lock_from_entries`

Builds a new lock populated from the given `(policy_group_id, price)` records.

`entries_ptr`/`entries_len` describe an array of
`OpenPitPretradePreTradeLockEntry`. A zero length is allowed and yields an empty
lock regardless of `entries_ptr`.

Success:

- returns a non-null caller-owned lock handle.

Error:

- returns null when `entries_ptr` is null while `entries_len` is non-zero or
  when any price fails domain validation;
- if `out_error` is not null, writes a caller-owned `OpenPitSharedString`
  error handle that MUST be released with `openpit_destroy_shared_string`.

Cleanup:

- on success the caller MUST release the returned handle with
  `openpit_destroy_pretrade_pre_trade_lock` exactly once.

```c
OpenPitPretradePreTradeLock *
openpit_create_pretrade_pre_trade_lock_from_entries(
    const OpenPitPretradePreTradeLockEntry * entries_ptr,
    size_t entries_len,
    OpenPitOutError out_error
);
```

## `openpit_pretrade_pre_trade_lock_merge`

Appends every record from `src` into `dst`, leaving `src` unchanged.

Success:

- returns `true`; `dst` now also carries every record from `src`.

Error:

- returns `false` when `dst` or `src` is null;
- if `out_error` is not null, writes a caller-owned `OpenPitSharedString`
  error handle that MUST be released with `openpit_destroy_shared_string`.

```c
bool openpit_pretrade_pre_trade_lock_merge(
    OpenPitPretradePreTradeLock * dst,
    const OpenPitPretradePreTradeLock * src,
    OpenPitOutError out_error
);
```

## `openpit_destroy_pretrade_pre_trade_lock_prices`

Releases a caller-owned lock price list.

Contract:

- `handle` must be a valid non-null pointer;
- this function always succeeds.

```c
void openpit_destroy_pretrade_pre_trade_lock_prices(
    OpenPitPretradePreTradeLockPrices * handle
);
```

## `openpit_pretrade_pre_trade_lock_prices_view`

Borrows a read-only view of a lock price list.

`handle` must be a valid non-null pointer; violating this triggers a panic.

Returns an unset view (`ptr == null`, `len == 0`) when the list is empty. The
view remains valid only while `handle` is alive.

```c
OpenPitPretradePreTradeLockPricesView
openpit_pretrade_pre_trade_lock_prices_view(
    const OpenPitPretradePreTradeLockPrices * handle
);
```

## `openpit_pretrade_pre_trade_lock_prices_of`

Returns the prices stored under `policy_group_id`.

Single-price case:

- when the group holds exactly one price, it is written directly to
  `out_price`.

Status:

- `Error`: `lock`, `out_price`, or `out_prices` is null; `out_error` receives
  an error handle when provided.
- `Empty`: the call succeeded and the group has no prices; `out_prices` is set
  to null.
- `One`: the call succeeded and `out_price` contains the only stored price;
  `out_prices` is set to null.
- `List`: the call succeeded and `out_prices` contains a caller-owned list.

Cleanup:

- when status is `List`, the caller MUST release `*out_prices` with
  `openpit_destroy_pretrade_pre_trade_lock_prices` exactly once.

```c
OpenPitPretradePreTradeLockPricesStatus
openpit_pretrade_pre_trade_lock_prices_of(
    const OpenPitPretradePreTradeLock * lock,
    uint16_t policy_group_id,
    OpenPitParamPrice * out_price,
    OpenPitPretradePreTradeLockPrices ** out_prices,
    OpenPitOutError out_error
);
```

## `OpenPitPretradePreTradeLockEntriesView`

Read-only view over a caller-owned lock entry snapshot.

```c
typedef struct OpenPitPretradePreTradeLockEntriesView {
    const OpenPitPretradePreTradeLockEntry * ptr;
    size_t len;
} OpenPitPretradePreTradeLockEntriesView;
```

## `OpenPitPretradePreTradeLockEntries`

Caller-owned snapshot of every `(policy_group_id, price)` record in a lock.

```c
typedef struct OpenPitPretradePreTradeLockEntries
    OpenPitPretradePreTradeLockEntries;
```

## `openpit_pretrade_pre_trade_lock_entries`

Returns a caller-owned snapshot of every `(policy_group_id, price)` record
stored in `lock`, in iteration order (default-group records first, then each
non-default group in insertion order).

`lock` must be a valid non-null handle. Passing null aborts the process.

Cleanup:

- the caller MUST release the returned handle with
  `openpit_destroy_pretrade_pre_trade_lock_entries` exactly once.

```c
OpenPitPretradePreTradeLockEntries * openpit_pretrade_pre_trade_lock_entries(
    const OpenPitPretradePreTradeLock * lock
);
```

## `openpit_destroy_pretrade_pre_trade_lock_entries`

Releases a caller-owned lock entry snapshot.

Contract:

- `handle` must be a valid non-null pointer;
- this function always succeeds.

```c
void openpit_destroy_pretrade_pre_trade_lock_entries(
    OpenPitPretradePreTradeLockEntries * handle
);
```

## `openpit_pretrade_pre_trade_lock_entries_view`

Borrows a read-only view of a lock entry snapshot.

`handle` must be a valid non-null pointer; violating this triggers a panic.

Returns an unset view (`ptr == null`, `len == 0`) when the snapshot is empty.
The view remains valid only while `handle` is alive.

```c
OpenPitPretradePreTradeLockEntriesView
openpit_pretrade_pre_trade_lock_entries_view(
    const OpenPitPretradePreTradeLockEntries * handle
);
```

## `openpit_pretrade_pre_trade_lock_to_msgpack`

Serializes the lock as MessagePack.

Success:

- returns a non-null caller-owned `OpenPitSharedBytes` carrying the
  MessagePack payload.

Error:

- returns null when `lock` is null or when the encoder fails;
- if `out_error` is not null, writes a caller-owned `OpenPitSharedString`
  error handle that MUST be released with `openpit_destroy_shared_string`.

Cleanup:

- on success the caller MUST release the returned handle with
  `openpit_destroy_shared_bytes` exactly once.

```c
OpenPitSharedBytes * openpit_pretrade_pre_trade_lock_to_msgpack(
    const OpenPitPretradePreTradeLock * lock,
    OpenPitOutError out_error
);
```

## `openpit_create_pretrade_pre_trade_lock_from_msgpack`

Builds a new lock from a MessagePack payload.

Success:

- returns a non-null caller-owned lock handle.

Error:

- returns null when `data_ptr` is null or when the payload cannot be decoded;
- if `out_error` is not null, writes a caller-owned `OpenPitSharedString`
  error handle that MUST be released with `openpit_destroy_shared_string`.

Cleanup:

- on success the caller MUST release the returned handle with
  `openpit_destroy_pretrade_pre_trade_lock` exactly once.

```c
OpenPitPretradePreTradeLock *
openpit_create_pretrade_pre_trade_lock_from_msgpack(
    const uint8_t * data_ptr,
    size_t data_len,
    OpenPitOutError out_error
);
```

## `openpit_pretrade_pre_trade_lock_to_json`

Serializes the lock as compact JSON.

Success:

- returns a non-null caller-owned `OpenPitSharedString` carrying the JSON
  payload.

Error:

- returns null when `lock` is null or when the encoder fails;
- if `out_error` is not null, writes a caller-owned `OpenPitSharedString`
  error handle that MUST be released with `openpit_destroy_shared_string`.

Cleanup:

- on success the caller MUST release the returned handle with
  `openpit_destroy_shared_string` exactly once.

```c
OpenPitSharedString * openpit_pretrade_pre_trade_lock_to_json(
    const OpenPitPretradePreTradeLock * lock,
    OpenPitOutError out_error
);
```

## `openpit_create_pretrade_pre_trade_lock_from_json`

Builds a new lock from a JSON payload produced by
`openpit_pretrade_pre_trade_lock_to_json` (or any compatible serializer).

`text_ptr`/`text_len` describe a UTF-8 byte sequence.

Success:

- returns a non-null caller-owned lock handle.

Error:

- returns null when `text_ptr` is null or when the payload cannot be decoded
  (invalid UTF-8 or invalid lock JSON);
- if `out_error` is not null, writes a caller-owned `OpenPitSharedString`
  error handle that MUST be released with `openpit_destroy_shared_string`.

Cleanup:

- on success the caller MUST release the returned handle with
  `openpit_destroy_pretrade_pre_trade_lock` exactly once.

```c
OpenPitPretradePreTradeLock * openpit_create_pretrade_pre_trade_lock_from_json(
    const uint8_t * text_ptr,
    size_t text_len,
    OpenPitOutError out_error
);
```

## `openpit_pretrade_pre_trade_lock_to_cbor`

Serializes the lock as CBOR.

Success:

- returns a non-null caller-owned `OpenPitSharedBytes` carrying the CBOR
  payload.

Error:

- returns null when `lock` is null or when the encoder fails;
- if `out_error` is not null, writes a caller-owned `OpenPitSharedString`
  error handle that MUST be released with `openpit_destroy_shared_string`.

Cleanup:

- on success the caller MUST release the returned handle with
  `openpit_destroy_shared_bytes` exactly once.

```c
OpenPitSharedBytes * openpit_pretrade_pre_trade_lock_to_cbor(
    const OpenPitPretradePreTradeLock * lock,
    OpenPitOutError out_error
);
```

## `openpit_create_pretrade_pre_trade_lock_from_cbor`

Builds a new lock from a CBOR payload.

Success:

- returns a non-null caller-owned lock handle.

Error:

- returns null when `data_ptr` is null or when the payload cannot be decoded;
- if `out_error` is not null, writes a caller-owned `OpenPitSharedString`
  error handle that MUST be released with `openpit_destroy_shared_string`.

Cleanup:

- on success the caller MUST release the returned handle with
  `openpit_destroy_pretrade_pre_trade_lock` exactly once.

```c
OpenPitPretradePreTradeLock * openpit_create_pretrade_pre_trade_lock_from_cbor(
    const uint8_t * data_ptr,
    size_t data_len,
    OpenPitOutError out_error
);
```

## `openpit_pretrade_pre_trade_lock_to_raw`

Serializes the lock using the in-process binary-stable raw layout.

`lock` must be a valid non-null handle; violating this triggers a panic.

Success:

- always returns a non-null caller-owned `OpenPitSharedBytes` carrying the raw
  payload.

Cleanup:

- the caller MUST release the returned handle with
  `openpit_destroy_shared_bytes` exactly once.

```c
OpenPitSharedBytes * openpit_pretrade_pre_trade_lock_to_raw(
    const OpenPitPretradePreTradeLock * lock
);
```

## `openpit_create_pretrade_pre_trade_lock_from_raw`

Builds a new lock from a raw payload produced by
`openpit_pretrade_pre_trade_lock_to_raw`.

Success:

- returns a non-null caller-owned lock handle.

Error:

- returns null when `data_ptr` is null or when the payload cannot be decoded;
- if `out_error` is not null, writes a caller-owned `OpenPitSharedString`
  error handle that MUST be released with `openpit_destroy_shared_string`.

Cleanup:

- on success the caller MUST release the returned handle with
  `openpit_destroy_pretrade_pre_trade_lock` exactly once.

```c
OpenPitPretradePreTradeLock * openpit_create_pretrade_pre_trade_lock_from_raw(
    const uint8_t * data_ptr,
    size_t data_len,
    OpenPitOutError out_error
);
```
