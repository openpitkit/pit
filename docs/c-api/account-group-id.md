# Account Group Id

<!-- markdownlint-disable MD013 MD024 -->

[Back to index](index.md)

## `OpenPitParamAccountGroupId`

Stable account-group identifier type for FFI payloads.

WARNING: Use exactly one account-group-id source model per runtime:

- either purely numeric IDs
  (`openpit_create_param_account_group_id_from_uint32`),
- or purely string-derived IDs
  (`openpit_create_param_account_group_id_from_string`).

Do not mix both models in the same runtime state. A hashed string value can
coincide with a direct numeric ID, collapsing two distinct groups into one key.

```c
typedef uint32_t OpenPitParamAccountGroupId;
```

## `OpenPitParamAccountGroupIdOptional`

```c
typedef struct OpenPitParamAccountGroupIdOptional {
    OpenPitParamAccountGroupId value;
    bool is_set;
} OpenPitParamAccountGroupIdOptional;
```

## `OPENPIT_DEFAULT_ACCOUNT_GROUP`

The reserved default account-group identifier. Every account belongs to this
group until it is registered into another one, so no constructor may produce it.
Mirrors `openpit::param::DEFAULT_ACCOUNT_GROUP`.

```c
#define OPENPIT_DEFAULT_ACCOUNT_GROUP ((OpenPitParamAccountGroupId) 0)
```

## `openpit_create_param_account_group_id_from_uint32`

Constructs an account-group identifier from a 32-bit integer.

This is a direct numeric mapping with no collision risk.

The value `0` is reserved for the default account group
(`OPENPIT_DEFAULT_ACCOUNT_GROUP`) and is rejected: every account already belongs
to that group implicitly, so no external input may name it.

WARNING: Do not mix IDs produced by this function with IDs produced by
`openpit_create_param_account_group_id_from_string` in the same runtime state.

Contract:

- returns `true` and writes a stable account-group identifier to `out` on
  success;
- returns `false` on the reserved value (`0`) and optionally writes an error
  message to `out_error`.

### Safety

`out` must be either null or a valid writable pointer.

```c
bool openpit_create_param_account_group_id_from_uint32(
    uint32_t value,
    OpenPitParamAccountGroupId * out,
    OpenPitOutError out_error
);
```

## `openpit_create_param_account_group_id_from_string`

Constructs an account-group identifier from a UTF-8 byte sequence using FNV-1a
32-bit hashing.

The bytes are read only for the duration of the call. No trailing zero byte is
required.

Collision note:

- different group strings can map to the same identifier;
- for `n` distinct group strings the probability of at least one collision is
  approximately `n^2 / (2 * 2^32)`.
- if collision risk is unacceptable, keep your own collision-free
  string-to-integer mapping and use
  `openpit_create_param_account_group_id_from_uint32`.

WARNING: Do not mix IDs produced by this function with IDs produced by
`openpit_create_param_account_group_id_from_uint32` in the same runtime state.

Contract:

- returns `true` and writes a stable account-group identifier to `out` on
  success;
- returns `false` on invalid input (empty string) and optionally writes an
  error message to `out_error`.

### Safety

`value.ptr` must be non-null and point to at least `value.len` readable UTF-8
bytes when `value.len > 0`.

```c
bool openpit_create_param_account_group_id_from_string(
    OpenPitStringView value,
    OpenPitParamAccountGroupId * out,
    OpenPitOutError out_error
);
```
