# openpit-derive

Proc-macro derives for [`openpit`](https://crates.io/crates/openpit). This
crate provides the `RequestFields` derive macro, which the public `openpit`
crate re-exports under the `derive` feature.

End users normally depend on `openpit` directly:

```toml
openpit = { version = "X.X", features = ["derive"] }
```

A direct dependency on `openpit-derive` is usually unnecessary.
