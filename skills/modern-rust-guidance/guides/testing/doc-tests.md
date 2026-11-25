---
name: doc-tests
description: Write runnable rustdoc examples (doctests) that use ? instead of unwrap, hide setup lines, document Errors and Panics, and keep working under edition 2024 combined doctests.
rust-version: "1.70"
clippy-lints: [test_attr_in_doctest, needless_doctest_main]
sources:
  - "rustdoc book: Documentation tests (MIT/Apache-2.0)"
  - "Edition Guide, Rust 2024: Rustdoc combined tests (MIT/Apache-2.0)"
  - "Cargo Book: cargo test, Cargo Targets (MIT/Apache-2.0)"
  - "Clippy lint docs: test_attr_in_doctest, needless_doctest_main, missing_errors_doc, missing_panics_doc (MIT/Apache-2.0)"
  - "Comprehensive Rust: Other Types of Tests (CC-BY-4.0 prose, Apache-2.0 code)"
  - "Apollo Rust Best Practices handbook, ch. 5.2 (Apache-2.0): doctests are not run by nextest"
---

# Doc Tests

Every public function with non-obvious use gets an `# Examples` section whose code block `cargo test` compiles and runs. Write the example the way a caller would: `?` instead of `unwrap`, a hidden line for setup, and `# Errors` / `# Panics` sections for the contract. In edition 2024 (1.85+) rustdoc merges examples into one binary, so avoid examples that depend on their own position in the file. For unit tests of private items see `test-organization`.

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

- (none): runs and must not panic. The default.
- `no_run`: compiled, never executed (network, real files, long loops).
- `should_panic`: a documented panic. Pair it with a `# Panics` section.
- `compile_fail`: shows what the type system rejects. Passes on any compile error, so keep the snippet minimal. Don't add an error code (`compile_fail,E0382`): stable rustdoc does not check it.
- `ignore`: almost never. It is never compiled and reports "ignored" forever. Use `no_run` for code that can't run and a `text` fence for output or shell.

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

When the whole crate is on edition 2024 (1.85+; use 1.85.1+, which fixed merging), rustdoc compiles compatible examples into one binary, which cuts compile time. Each example still runs in its own process, so statics do not leak between examples. Before edition 2024, every example is its own crate. Examples that use `compile_fail`, an `edition` tag, crate attributes such as `#![feature]`, `#[global_allocator]`, or a macro that uses `$crate` are never merged.

DO NOT assert on `std::panic::Location` (line numbers shift when examples merge) or on `std::any::type_name` (the module path differs). If an example must be built alone, tag it:

```text
/// ```standalone_crate
/// let line = std::panic::Location::caller().line();
/// ```
```

After moving to edition 2024, run `cargo test --doc`; a failure that disappears with `standalone_crate` means the example depended on its own layout.

## Avoid

```rust,legacy
// lint: needless_doctest_main, test_attr_in_doctest
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
```

Instead of the above, use `?` and no wrapper. Also avoid:

- `unwrap()` throughout examples: readers copy it.
- No `# Examples` on public functions, or examples with `ignore`: they rot, because nothing compiles them.
- Indented code in doc comments: rustdoc treats four-space-indented blocks as Rust and compiles them. Fence shell or output with `text`.
- Running only `cargo test --all-targets`: it does not run doctests. Add `cargo test --doc` (see `testing`); a runner such as `cargo-nextest` (out of scope) does not run them either.

## Verify

- `cargo clippy` flags the two legacy patterns above by default. Opt-in, pedantic and public items only: `-W clippy::missing_errors_doc -W clippy::missing_panics_doc`.
- `cargo test --doc` lists each example as `src/lib.rs - item (line N)`. Look for `ignored` entries and justify or remove each.
- `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps` catches broken intra-doc links such as `[`ParseIntError`]`.
