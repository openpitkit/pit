# String

[Back to index](index.md)

## `PitStringView`

Non-owning UTF-8 string view.

This type never owns memory. It borrows bytes from another object.

Lifetime contract:

- `ptr` points to `len` readable bytes;
- the memory is valid while the original object is alive and the source

string has not been modified;

- the caller must not free or mutate memory behind `ptr`.
- if the caller needs to retain the string beyond that announced lifetime,

the caller must copy the bytes.

```c
typedef struct PitStringView {
    const uint8_t * ptr;
    size_t len;
} PitStringView;
```

## `PitSharedString`

Owning shared-string handle.

Use this type when an FFI function needs to hand a string to the caller
whose lifetime must extend beyond the single FFI call and whose storage
must not depend on thread-local state remaining intact on the reader side.

Ownership contract:

- every non-null `*mut PitSharedString` returned through FFI is owned by

the caller;

- the caller MUST release it with `pit_destroy_shared_string` when no

longer needed; failing to do so leaks the underlying allocation;

- the handle internally holds a reference-counted copy of the string, so

multiple live handles pointing at the same original value are safe and
independent; destroying one handle does not affect the others.

Read contract:

- read the bytes with `pit_shared_string_view`;
- the returned `PitStringView` is valid while this specific handle is

alive and must not outlive the call to `pit_destroy_shared_string`
for this handle.

Threading contract:

- the handle itself is safe to move to and read from any thread once the

caller has received it; no thread-local state is consulted when reading
or destroying.

```c
typedef struct PitSharedString PitSharedString;
```

## `pit_destroy_shared_string`

Releases a `PitSharedString` handle.

Null input is a no-op.

After this call, the handle and any `PitStringView` previously obtained
from it are invalid and must not be used.

```c
void pit_destroy_shared_string(
    PitSharedString * handle
);
```

## `pit_shared_string_view`

Borrows a read-only view of the bytes stored in the handle.

Returns an unset view (`ptr == null`, `len == 0`) when `handle` is null.

The returned view is valid only while `handle` remains alive. The caller
must copy the bytes if they must outlive the handle.

```c
PitStringView pit_shared_string_view(
    const PitSharedString * handle
);
```
