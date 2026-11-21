---
name: test-assertions
description: Write assertions that explain failures, including expected panics.
rust-version: "1.81"
clippy-lints: [should_panic_without_expect]
sources:
  - "fixture"
---

# Test assertions

Give `#[should_panic]` an `expected` message.

```rust
#[test]
#[should_panic(expected = "boom")]
fn panics() {
    panic!("boom");
}
```

## Avoid

```rust,legacy
// lint: should_panic_without_expect
#[test]
#[should_panic]
fn panics() {
    panic!("boom");
}
```

## Older MSRV

`#[expect]` needs 1.81; use `#[allow]` before that.

## Verify

Run `cargo test`.
