---
name: std
description: Pick current standard-library idioms for Rust code: lazy globals, early returns, Option and Result, integer math, errors, collections, strings and paths, lints, and edition 2024.
rust-version: "1.70"
clippy-lints: []
sources:
  - "Rust release notes 1.65 to 1.100 (MIT/Apache-2.0)"
  - "std docs, stable API attributes in rust-src (MIT/Apache-2.0)"
  - "Clippy msrvs table and lint docs (MIT/Apache-2.0)"
  - "Rust Edition Guide, Rust 2024 (MIT/Apache-2.0)"
---

# Standard Library Idioms

Std grew a lot after most training data was frozen, and the same old patterns keep coming back: `lazy_static!`, `map_or(false, …)`, `(a + b - 1) / b`, `io::Error::new(ErrorKind::Other, …)`, `#[allow]` without a reason. Check the project's MSRV first. Every guide below states the version each API needs and has an `## Older MSRV` section when it needs more than 1.70.

## Defaults

- Use edition 2024 for new code. It needs Rust 1.85 (see `edition-2024-changes`).
- Reach for std before a crate. Where a crate is the usual answer (`thiserror`, `anyhow`, `once_cell`, `lazy_static`), the guide says so in a note and still gives the std route.
- Prefer a named method over a hand-rolled formula, and a combinator over a `match` that only reshapes a value.
- Return `Result` for anything the caller can cause. Panic only for bugs.
- Let `cargo clippy` find the old patterns. Each guide lists the lints that detect them.

## What changed that agents miss

| Instead of | Use | Since |
| --- | --- | --- |
| `lazy_static!`, `once_cell::sync::Lazy` | `LazyLock`; `OnceLock` for runtime input | 1.80; 1.70 |
| `match … { None => return … }` | `let … else` | 1.65 |
| nested `if let { if cond { … } }` | let chains (edition 2024 only) | 1.88 |
| `opt.map_or(false, f)`, `map_or(true, f)` | `is_some_and`, `is_none_or` | 1.70, 1.82 |
| a `map` closure that only logs and returns its argument | `inspect`, `inspect_err` | 1.76 |
| `(a + b - 1) / b`, `n % d == 0`, `(a + b) / 2` | `div_ceil` (unsigned), `is_multiple_of` (unsigned), `midpoint` | 1.73, 1.87, 1.85/1.87 |
| `io::Error::new(ErrorKind::Other, e)` | `io::Error::other(e)` | 1.74 |
| `if v.last().is_some_and(p) { v.pop(); }` | `Vec::pop_if` | 1.86 |
| manual removal loops | `Vec::extract_if` (`HashMap` 1.88, `BTreeMap` 1.91) | 1.87 |
| `split_at_mut` index gymnastics | `get_disjoint_mut` | 1.86 |
| `&s[..n]` on arbitrary `n` | `floor_char_boundary` | 1.91 |
| `#![warn(…)]` blocks, bare `#[allow]` | `[lints]` in `Cargo.toml`, `#[expect(…, reason)]` | 1.74, 1.81 |
| `extern "C" {}`, `#[no_mangle]`, `static mut` refs | `unsafe extern`, `#[unsafe(no_mangle)]`, atomics or `LazyLock` | edition 2024 |

## Still unstable: do not suggest

On stable Rust 1.100 these do not compile: `OnceLock::get_or_try_init`, signed `div_ceil` and `next_multiple_of`, `HashMap::try_insert`, `BinaryHeap::pop_if`, `VecDeque::extract_if`, and `Iterator::intersperse`, `array_chunks`, `map_windows`, `try_collect`, `collect_into`. If one looks perfect for the job, write the std-stable equivalent.

## Guides

- `lazy-initialization`: global state, a lazily computed value, a runtime-initialized singleton, `thread_local!`, or replacing `lazy_static`, `once_cell`, or `static mut`.
- `early-return-patterns`: flatten nested `match`/`if let`, bail out on `None` or `Err`, use `let … else`, `?`, `matches!`, or let chains.
- `option-result-combinators`: test, convert, or default an `Option`/`Result`; `is_some_and`, `inspect`, `ok_or_else`, `transpose`, eager versus lazy arguments.
- `integer-arithmetic`: overflow, rounding up, differences, averages, multiples, logs, clamping, or numeric casts and `TryFrom`.
- `error-values`: define a custom error type, make `?` convert errors, report errors in `main`, wrap with `io::Error::other`, decide between `Result` and panic.
- `collection-editing`: filter, remove, dedup, sort, search, or update `Vec`, `HashMap`, `BTreeMap`, `VecDeque` in place; entry API; `extract_if`.
- `string-and-path-handling`: parse or split text, strip prefixes, slice by character, build or inspect file paths, choose `&str`/`AsRef`/`Cow` parameters.
- `lint-configuration`: set up `[lints]` or `[workspace.lints]`, Clippy MSRV, or replace `#[allow]` with `#[expect]`.
- `edition-2024-changes`: migrate to edition 2024 or fix an error after switching: temporaries, `impl Trait` capture, `unsafe extern`, `set_var`, `gen`.

For allocation, copies, I/O, and release profiles see the `performance` hub. For test layout, assertions, and isolation see the `testing` hub.

## Verify

- `cargo clippy --all-targets -- -D warnings` plus the lints listed in each guide you applied.
- If a suggested API fails with "unstable library feature", it is nightly-only here. Replace it rather than adding `#![feature]`.
