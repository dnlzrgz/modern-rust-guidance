---
name: allocation-reuse
description: Reuse buffers and reserve capacity to avoid repeated heap allocations.
rust-version: "1.70"
clippy-lints: [needless_range_loop]
sources:
  - "fixture"
---

# Allocation reuse

Clear and refill one `Vec` instead of allocating a new one per iteration.

```rust
pub fn squares(n: u32, out: &mut Vec<u32>) {
    out.clear();
    out.extend((0..n).map(|i| i * i));
}
```

## Avoid

```rust,legacy
// lint: needless_range_loop
pub fn total(v: &[u32]) -> u32 {
    let mut sum = 0;
    for i in 0..v.len() {
        sum += v[i];
    }
    sum
}
```

## Verify

Run `cargo clippy -- -W clippy::needless_range_loop`.
