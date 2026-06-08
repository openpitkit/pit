# Runtime and Errors

<!-- markdownlint-disable MD013 MD024 -->

[Back to index](index.md)

## `OpenPitOutError`

Error out-pointer used by fallible FFI calls.

```c
typedef OpenPitSharedString ** OpenPitOutError;
```

## `OpenPitParamErrorCode`

Parameter error code transported through FFI.

```c
typedef uint32_t OpenPitParamErrorCode;
/**
 * Error code is not specified.
 */
#define OpenPitParamErrorCode_Unspecified ((OpenPitParamErrorCode) 0)
/**
 * Value must be non-negative.
 */
#define OpenPitParamErrorCode_Negative ((OpenPitParamErrorCode) 1)
/**
 * Division by zero.
 */
#define OpenPitParamErrorCode_DivisionByZero ((OpenPitParamErrorCode) 2)
/**
 * Arithmetic overflow.
 */
#define OpenPitParamErrorCode_Overflow ((OpenPitParamErrorCode) 3)
/**
 * Arithmetic underflow.
 */
#define OpenPitParamErrorCode_Underflow ((OpenPitParamErrorCode) 4)
/**
 * Invalid float value.
 */
#define OpenPitParamErrorCode_InvalidFloat ((OpenPitParamErrorCode) 5)
/**
 * Invalid textual format.
 */
#define OpenPitParamErrorCode_InvalidFormat ((OpenPitParamErrorCode) 6)
/**
 * Invalid price value.
 */
#define OpenPitParamErrorCode_InvalidPrice ((OpenPitParamErrorCode) 7)
/**
 * Invalid leverage value.
 */
#define OpenPitParamErrorCode_InvalidLeverage ((OpenPitParamErrorCode) 8)
/**
 * Asset identifier is empty.
 */
#define OpenPitParamErrorCode_AssetEmpty ((OpenPitParamErrorCode) 9)
/**
 * Account identifier string is empty.
 */
#define OpenPitParamErrorCode_AccountIdEmpty ((OpenPitParamErrorCode) 10)
/**
 * Catch-all code for unknown cases.
 */
#define OpenPitParamErrorCode_Other ((OpenPitParamErrorCode) 4294967295)
```

## `OpenPitParamError`

Caller-owned parameter error container.

```c
typedef struct OpenPitParamError {
    OpenPitParamErrorCode code;
    OpenPitSharedString * message;
} OpenPitParamError;
```

## `OpenPitOutParamError`

Parameter error out-pointer used by fallible param FFI calls.

```c
typedef OpenPitParamError ** OpenPitOutParamError;
```

## `openpit_destroy_param_error`

Releases a caller-owned parameter error container.

### Safety

`handle` must be either null or a pointer returned by this library through
`OpenPitOutParamError`. The handle must be destroyed at most once.

```c
void openpit_destroy_param_error(
    OpenPitParamError * handle
);
```

## `openpit_get_runtime_version`

Returns the OpenPit runtime version string.

This function never fails.

The returned view is read-only, never null, and remains valid for the entire
process lifetime. The caller must not release it.

```c
OpenPitStringView openpit_get_runtime_version(void);
```

## `openpit_get_runtime_build_profile`

Returns the build profile of the linked OpenPit runtime.

This function never fails.

The value is a stable, machine-parseable `key=value;`-delimited string (keys
`version`, `profile`, `opt_level`, `debug_assertions`, `target`, `target_cpu`,
`lto`). It lets a consumer reliably distinguish a debug core from a release
core, for example to refuse latency-sensitive work on a debug build. The
`target_cpu` and `lto` fields report the literal `unknown` when they cannot be
determined at build time.

The returned view is read-only, never null, and remains valid for the entire
process lifetime. The caller must not release it.

```c
OpenPitStringView openpit_get_runtime_build_profile(void);
```
