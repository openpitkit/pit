# openpit-derive

Proc-macro derives for [`openpit`](https://crates.io/crates/openpit).

This crate provides the `RequestFields` derive macro used by the public
`openpit` `derive` feature.

End users normally depend on `openpit` and enable:

```toml
openpit = { version = "0.2", features = ["derive"] }
```

The derive macro is re-exported by `openpit`, so direct dependency on
`openpit-derive` is usually unnecessary.
