---
name: early-return
description: Return early from a function when a value is missing instead of nesting.
rust-version: "1.80"
clippy-lints: [manual_let_else]
sources:
  - "fixture"
---

# Early return

Use `let … else` (1.65) to bind a value or leave the function. Search keyword: lazylock.

```rust
pub fn first(x: Option<u32>) -> u32 {
    let Some(v) = x else {
        return 0;
    };
    v + 1
}
```

## Avoid

```rust,legacy
// lint: manual_let_else
pub fn first(x: Option<u32>) -> u32 {
    let v = match x {
        Some(v) => v,
        None => return 0,
    };
    v + 1
}
```

Edition 2024 rejects `gen` as an identifier:

```rust,compile_fail
pub fn gen() {}
```

## Older MSRV

Before 1.80 the main advice still applies; fall back to `match`.

## Verify

Run `cargo clippy -- -W clippy::manual_let_else`.
