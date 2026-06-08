# Bytes

<!-- markdownlint-disable MD013 MD024 -->

[Back to index](index.md)

## `OpenPitBytesView`

Non-owning byte slice view.

Lifetime contract:

- `ptr` points to `len` readable bytes;
- the memory is valid while the original object is alive;
- the caller must not free or mutate memory behind `ptr`;
- if the caller needs to retain the bytes beyond that announced lifetime, the
  caller must copy them.

```c
typedef struct OpenPitBytesView {
    const uint8_t * ptr;
    size_t len;
} OpenPitBytesView;
```

## `OpenPitSharedBytes`

Owning shared-bytes handle.

Use this type when an FFI function needs to hand a binary payload to the caller
whose lifetime must extend beyond the single FFI call.

Ownership contract:

- every non-null `*mut OpenPitSharedBytes` returned through FFI is owned by
  the caller;
- the caller MUST release it with `openpit_destroy_shared_bytes` when no
  longer needed; failing to do so leaks the underlying allocation.

```c
typedef struct OpenPitSharedBytes OpenPitSharedBytes;
```

## `openpit_destroy_shared_bytes`

Releases a `OpenPitSharedBytes` handle.

Null input is a no-op.

```c
void openpit_destroy_shared_bytes(
    OpenPitSharedBytes * handle
);
```

## `openpit_shared_bytes_view`

Borrows a read-only view of the bytes stored in the handle.

Returns an unset view (`ptr == null`, `len == 0`) when `handle` is null.

```c
OpenPitBytesView openpit_shared_bytes_view(
    const OpenPitSharedBytes * handle
);
```
