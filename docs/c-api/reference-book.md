# Reference Book

<!-- markdownlint-disable MD013 MD024 -->

[Back to index](index.md)

## `OpenPitSettlementUnit`

Raw settlement-unit code for FFI payloads.

The value is validated before it is converted into the Rust [`SettlementUnit`].

```c
typedef uint8_t OpenPitSettlementUnit;
```

## `OPENPIT_SETTLEMENT_UNIT_BUSINESS_DAYS`

Business-day settlement delay.

```c
#define OPENPIT_SETTLEMENT_UNIT_BUSINESS_DAYS ((OpenPitSettlementUnit) 0)
```

## `OPENPIT_SETTLEMENT_UNIT_CALENDAR_DAYS`

Calendar-day settlement delay.

```c
#define OPENPIT_SETTLEMENT_UNIT_CALENDAR_DAYS ((OpenPitSettlementUnit) 1)
```

## `OpenPitSettlementLag`

Flat settlement delay payload.

```c
typedef struct OpenPitSettlementLag {
    uint64_t n;
    OpenPitSettlementUnit unit;
} OpenPitSettlementLag;
```

## `OpenPitSettlementScheme`

Flat settlement payload with independent delivery and payment legs.

```c
typedef struct OpenPitSettlementScheme {
    OpenPitSettlementLag delivery;
    OpenPitSettlementLag payment;
} OpenPitSettlementScheme;
```

## `OpenPitReferenceBookRegisterStatus`

Registration result for a reference book.

```c
typedef uint8_t OpenPitReferenceBookRegisterStatus;
/**
 * The instrument was registered and `out_id` was populated.
 */
#define OpenPitReferenceBookRegisterStatus_Ok \
    ((OpenPitReferenceBookRegisterStatus) 0)
/**
 * The supplied ID is already registered.
 */
#define OpenPitReferenceBookRegisterStatus_DuplicateId \
    ((OpenPitReferenceBookRegisterStatus) 1)
/**
 * The supplied instrument is already registered.
 */
#define OpenPitReferenceBookRegisterStatus_DuplicateInstrument \
    ((OpenPitReferenceBookRegisterStatus) 2)
/**
 * The input payload or handle was invalid.
 */
#define OpenPitReferenceBookRegisterStatus_Error \
    ((OpenPitReferenceBookRegisterStatus) 255)
```

## `OpenPitReferenceBookStatus`

Result for reference-book attribute updates.

```c
typedef uint8_t OpenPitReferenceBookStatus;
/**
 * The operation completed successfully.
 */
#define OpenPitReferenceBookStatus_Ok ((OpenPitReferenceBookStatus) 0)
/**
 * The requested instrument ID is not registered.
 */
#define OpenPitReferenceBookStatus_UnknownInstrument \
    ((OpenPitReferenceBookStatus) 1)
/**
 * The input payload or handle was invalid.
 */
#define OpenPitReferenceBookStatus_Error ((OpenPitReferenceBookStatus) 255)
```

## `OpenPitReferenceBook`

Opaque handle to a core instrument reference book.

```c
typedef struct OpenPitReferenceBook OpenPitReferenceBook;
```

## `openpit_create_reference_book`

Creates an empty core instrument reference book.

The returned handle is caller-owned and must be released with
[`openpit_destroy_reference_book`].

```c
OpenPitReferenceBook * openpit_create_reference_book(void);
```

## `openpit_destroy_reference_book`

Releases a caller-owned reference-book handle.

Passing null is allowed and has no effect.

```c
void openpit_destroy_reference_book(
    OpenPitReferenceBook * book
);
```

## `openpit_reference_book_register`

Registers `instrument` under the next available reference-book ID.

`out_id` receives the assigned ID on success. The function reports duplicate
registrations through its return status; malformed inputs use `out_error`.

```c
OpenPitReferenceBookRegisterStatus openpit_reference_book_register(
    OpenPitReferenceBook * book,
    const OpenPitInstrument * instrument,
    OpenPitInstrumentId * out_id,
    OpenPitOutError out_error
);
```

## `openpit_reference_book_register_with_id`

Registers `instrument` under a caller-assigned `instrument_id`.

`out_id` receives the same ID on success. The supplied ID can be reused in an
independent market-data registration.

```c
OpenPitReferenceBookRegisterStatus openpit_reference_book_register_with_id(
    OpenPitReferenceBook * book,
    const OpenPitInstrument * instrument,
    OpenPitInstrumentId instrument_id,
    OpenPitInstrumentId * out_id,
    OpenPitOutError out_error
);
```

## `openpit_reference_book_resolve`

Resolves an instrument to its reference-book ID.

Returns `true` and populates `out_id` when the instrument is registered. Returns
`false` when it is absent or an input pointer is null.

```c
bool openpit_reference_book_resolve(
    const OpenPitReferenceBook * book,
    const OpenPitInstrument * instrument,
    OpenPitInstrumentId * out_id
);
```

## `openpit_reference_book_set_settlement_scheme`

Sets the settlement scheme for a registered instrument.

Invalid raw settlement-unit codes are rejected before a core enum is
constructed. `UnknownInstrument` means the book has no matching ID.

```c
OpenPitReferenceBookStatus openpit_reference_book_set_settlement_scheme(
    OpenPitReferenceBook * book,
    OpenPitInstrumentId instrument_id,
    OpenPitSettlementScheme settlement_scheme,
    OpenPitOutError out_error
);
```

## `openpit_reference_book_clear_settlement_scheme`

Clears the settlement scheme for a registered instrument.

```c
OpenPitReferenceBookStatus openpit_reference_book_clear_settlement_scheme(
    OpenPitReferenceBook * book,
    OpenPitInstrumentId instrument_id,
    OpenPitOutError out_error
);
```

## `openpit_reference_book_get_settlement_scheme`

Retrieves the settlement scheme for a registered instrument.

`Ok` with `out_is_set == false` means that the instrument is registered but has
no settlement scheme. `UnknownInstrument` means that `instrument_id` is not
registered. On `Ok` with `out_is_set == true`, `out_scheme` receives the
configured value.

```c
OpenPitReferenceBookStatus openpit_reference_book_get_settlement_scheme(
    const OpenPitReferenceBook * book,
    OpenPitInstrumentId instrument_id,
    OpenPitSettlementScheme * out_scheme,
    bool * out_is_set,
    OpenPitOutError out_error
);
```
