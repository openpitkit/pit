# Runtime and Errors

[Back to index](index.md)

## `pit_get_runtime_version`

Returns the Pit runtime version string.

This function never fails.

The returned view is read-only, never null, and remains valid for the
entire process lifetime. The caller must not release it.

```c
PitStringView pit_get_runtime_version(void);
```
