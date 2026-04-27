# Instrument

[Back to index](index.md)

## `PitInstrument`

Trading instrument view.

The caller owns the memory referenced by both string views.

Semantics:

- both string views set: instrument is present;
- both string views not set: instrument is absent;
- only one string view set: invalid payload.

```c
typedef struct PitInstrument {
    PitStringView underlying_asset;
    PitStringView settlement_asset;
} PitInstrument;
```
