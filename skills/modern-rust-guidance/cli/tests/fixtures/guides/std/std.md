---
name: std
description: Pick current standard-library idioms for everyday Rust code.
rust-version: "1.70"
clippy-lints: []
sources:
  - "fixture"
---

# Standard library

Prefer the newest std idiom that the project's MSRV allows.

## Guides

- `early-return`: leave a function early without nesting.
