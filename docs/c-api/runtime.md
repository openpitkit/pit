# Runtime and Errors

[Back to index](index.md)

## `PitOutError`

Error out-pointer used by fallible FFI calls.

```c
typedef PitSharedString ** PitOutError;
```

## `PitParamErrorCode`

Parameter error code transported through FFI.

```c
typedef uint32_t PitParamErrorCode;
/**
 * Error code is not specified.
 */
#define PitParamErrorCode_Unspecified ((PitParamErrorCode) 0)
/**
 * Value must be non-negative.
 */
#define PitParamErrorCode_Negative ((PitParamErrorCode) 1)
/**
 * Division by zero.
 */
#define PitParamErrorCode_DivisionByZero ((PitParamErrorCode) 2)
/**
 * Arithmetic overflow.
 */
#define PitParamErrorCode_Overflow ((PitParamErrorCode) 3)
/**
 * Arithmetic underflow.
 */
#define PitParamErrorCode_Underflow ((PitParamErrorCode) 4)
/**
 * Invalid float value.
 */
#define PitParamErrorCode_InvalidFloat ((PitParamErrorCode) 5)
/**
 * Invalid textual format.
 */
#define PitParamErrorCode_InvalidFormat ((PitParamErrorCode) 6)
/**
 * Invalid price value.
 */
#define PitParamErrorCode_InvalidPrice ((PitParamErrorCode) 7)
/**
 * Invalid leverage value.
 */
#define PitParamErrorCode_InvalidLeverage ((PitParamErrorCode) 8)
/**
 * Catch-all code for unknown cases.
 */
#define PitParamErrorCode_Other ((PitParamErrorCode) 4294967295)
```

## `PitParamError`

Caller-owned parameter error container.

```c
typedef struct PitParamError {
    PitParamErrorCode code;
    PitSharedString * message;
} PitParamError;
```

## `PitOutParamError`

Parameter error out-pointer used by fallible param FFI calls.

```c
typedef PitParamError ** PitOutParamError;
```

## `pit_destroy_param_error`

Releases a caller-owned parameter error container.

### Safety

`handle` must be either null or a pointer returned by this library through
`PitOutParamError`. The handle must be destroyed at most once.

```c
void pit_destroy_param_error(
    PitParamError * handle
);
```

## `pit_get_runtime_version`

Returns the Pit runtime version string.

This function never fails.

The returned view is read-only, never null, and remains valid for the
entire process lifetime. The caller must not release it.

```c
PitStringView pit_get_runtime_version(void);
```
