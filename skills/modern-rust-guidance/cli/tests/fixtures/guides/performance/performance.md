---
name: performance
description: Measure first, then reduce allocations and copies.
rust-version: "1.70"
clippy-lints: []
sources:
  - "fixture"
---

# Performance

Measure before changing code.

## Guides

- `allocation-reuse`: keep buffers alive between iterations.
