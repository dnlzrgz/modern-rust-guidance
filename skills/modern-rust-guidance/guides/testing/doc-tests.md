---
name: doc-tests
description: Write runnable rustdoc examples (doctests) that use ? instead of unwrap, hide setup lines, document Errors and Panics, and keep working under edition 2024 combined doctests.
rust-version: "1.85"
clippy-lints: [test_attr_in_doctest, missing_errors_doc, missing_panics_doc, needless_doctest_main]
sources:
  - "rustdoc book: Documentation tests (MIT/Apache-2.0)"
  - "Edition Guide, Rust 2024: Rustdoc combined tests (MIT/Apache-2.0)"
  - "Cargo Book: cargo test, Cargo Targets (MIT/Apache-2.0)"
  - "Clippy lint docs: test_attr_in_doctest, missing_errors_doc, missing_panics_doc, needless_doctest_main (MIT/Apache-2.0)"
  - "Comprehensive Rust: Other Types of Tests (CC-BY-4.0 prose, Apache-2.0 code)"
  - "Apollo Rust Best Practices handbook, ch. 5.2 (Apache-2.0): doctests are not run by nextest"
---

# Doc Tests

Every public function with non-obvious use gets an `# Examples` section whose code block `cargo test` compiles and runs. Write the example the way a caller would: `?` instead of `unwrap`, a hidden line for setup, and `# Errors` / `# Panics` sections for the contract. In edition 2024 (Rust 1.85) rustdoc merges examples into one binary, so avoid examples that depend on their own position in the file. Unit tests for private items are in `test-organization`.

## Use `?` and hide the plumbing

A doctest body is wrapped in `fn main()`. End it with `Ok::<(), E>(())` (no spaces inside `(())`) and `?` works. Lines starting with `# ` are compiled but not shown.

```rust
use std::num::ParseIntError;

/// Parses a TCP port.
///
/// # Errors
///
/// Returns [`ParseIntError`] if `s` is not a number in `0..=65535`.
///
/// # Examples
///
/// ```
/// let port = my_crate::parse_port("8080")?;
/// assert_eq!(port, 8080);
/// # Ok::<(), std::num::ParseIntError>(())
/// ```
pub fn parse_port(s: &str) -> Result<u16, ParseIntError> {
    s.parse()
}
```

For several `?` with different error types, use a hidden `main` instead: `# fn main() -> Result<(), Box<dyn std::error::Error>> {` first and `# Ok(())` / `# }` last. Clippy's `needless_doctest_main` does not flag a `main` that returns `Result`.

Examples reach your crate through its public path (`my_crate::...` or a hidden `# use my_crate::parse_port;`). They cannot use private items; test those with unit tests.

## Pick the block attribute that says what you mean

| Attribute | Use it for |
| --- | --- |
| (none) | Runs and must not panic. The default. |
| `no_run` | Compiled, never executed: network, real files, long loops. |
| `should_panic` | A documented panic. Pair it with a `# Panics` section. |
| `compile_fail` | Shows what the type system rejects. Passes on any compile error, and stable rustdoc does not check an error code, so keep the snippet minimal. |
| `ignore` | Almost never. It is never compiled and reports "ignored" forever. Use `no_run` for code that can't run and a `text` fence for output or shell. |

```rust
/// Returns the first `n` bytes of `data`.
///
/// # Panics
///
/// Panics if `n > data.len()`.
///
/// # Examples
///
/// ```
/// assert_eq!(my_crate::head(b"abc", 2), b"ab");
/// ```
///
/// ```should_panic
/// my_crate::head(b"abc", 9);
/// ```
pub fn head(data: &[u8], n: usize) -> &[u8] {
    assert!(n <= data.len(), "n out of range");
    &data[..n]
}

/// Reads the config file.
///
/// # Errors
///
/// Returns the I/O error from reading `path`.
///
/// ```no_run
/// let text = my_crate::load("/etc/my-app.conf")?;
/// # Ok::<(), std::io::Error>(())
/// ```
pub fn load(path: &str) -> std::io::Result<String> {
    std::fs::read_to_string(path)
}
```

## Doctests in a binary never run

Rustdoc extracts examples from the library target only. A package with only `src/main.rs` has no doctests: `cargo test` prints no `Doc-tests` section and `cargo test --doc` fails with "no library targets found". Move the code into `src/lib.rs` (see `test-organization`).

## Test the README too

```rust,ignore
// src/lib.rs: runs the README's code blocks as doctests, adds nothing to the rendered docs
#[doc = include_str!("../README.md")]
#[cfg(doctest)]
pub struct ReadmeDoctests;
```

## Edition 2024: merged doctests

From edition 2024 (Rust 1.85; use 1.85.1 or later, which fixed merging), rustdoc compiles compatible examples of a crate into one binary, which cuts compile time. Each example still runs in its own process, so statics do not leak between examples. Examples that use `compile_fail`, an `edition` tag, crate attributes such as `#![feature]`, or `#[global_allocator]` are not merged automatically.

DO NOT assert on `std::panic::Location` (line numbers shift when examples merge) or on `std::any::type_name` (the module path differs). If an example must be built alone, tag it:

```text
/// ```standalone_crate
/// let line = std::panic::Location::caller().line();
/// ```
```

After moving to edition 2024, run `cargo test --doc`; a failure that disappears with `standalone_crate` means the example depended on its own layout.

## Older MSRV

Before Rust 1.85, or in edition 2021 and earlier, every example is compiled and run as its own crate. Slower, but the authoring rules above are unchanged, and `standalone_crate` is unnecessary. The `Ok::<(), E>(())` trick for `?` works since Rust 1.34.

## Avoid

```rust,legacy
// lint: needless_doctest_main, test_attr_in_doctest, missing_errors_doc, missing_panics_doc
/// Parses a port.
///
/// ```
/// fn main() { // rustdoc already wraps the example in main
///     assert_eq!(my_crate::parse_port("80").unwrap(), 80);
/// }
/// ```
///
/// ```
/// #[test] // never runs inside a doctest
/// fn works() { assert!(my_crate::parse_port("80").is_ok()); }
/// ```
pub fn parse_port(s: &str) -> Result<u16, std::num::ParseIntError> { s.parse() }

/// Returns the first byte.
pub fn first(data: &[u8]) -> u8 { data.first().copied().unwrap() }
```

Instead of the above, use `?`, no wrapper, and `# Errors` / `# Panics` sections. Also avoid:

- `unwrap()` throughout examples: readers copy it.
- No `# Examples` on public functions, or examples with `ignore`: they rot, because nothing compiles them.
- Indented code in doc comments: rustdoc treats four-space-indented blocks as Rust and compiles them. Fence shell or output with `text`.
- Running only `cargo test --all-targets`: it does not run doctests. Add `cargo test --doc`.

> [!NOTE]
> A test runner other than `cargo test` (for example `cargo-nextest`, out of scope here) does not run doctests either. Keep a separate `cargo test --doc` step.

## Verify

- `cargo clippy -- -W clippy::missing_errors_doc -W clippy::missing_panics_doc -W clippy::test_attr_in_doctest -W clippy::needless_doctest_main`. The first two are pedantic and only look at public items.
- `cargo test --doc` lists each example as `src/lib.rs - item (line N)`. Look for `ignored` entries and justify or remove each.
- `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps` catches broken intra-doc links such as `[`ParseIntError`]`.

