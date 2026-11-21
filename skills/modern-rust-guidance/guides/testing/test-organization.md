---
name: test-organization
description: Organize unit tests, integration tests in tests/, and a testable binary (lib.rs plus main.rs) in a Cargo package, and run all of them.
rust-version: "1.70"
clippy-lints: [items_after_test_module, tests_outside_test_module, redundant_test_prefix]
sources:
  - "The Rust Book, ch. 11 Test Organization (MIT/Apache-2.0)"
  - "Cargo Book: Cargo Targets, cargo test (MIT/Apache-2.0)"
  - "Microsoft Pragmatic Rust Guidelines: M-INTEGRATION-TESTS, M-TEST-UTIL (MIT)"
  - "Comprehensive Rust: Unit Tests, Other Types of Tests (CC-BY-4.0 prose, Apache-2.0 code)"
  - "Clippy lint docs: items_after_test_module, tests_outside_test_module, redundant_test_prefix (MIT/Apache-2.0)"
---

# Test Organization

Unit tests go in a `#[cfg(test)] mod tests` at the end of the file they test. Tests that only use the public API go in `tests/`, in a single target. Anything a test must reach lives in `src/lib.rs`, never in `main.rs`. For doc examples see `doc-tests`; for fakes and injected dependencies see `test-seams`.

## Put unit tests last, in one `tests` module

```rust
pub fn first_word(text: &str) -> &str {
    text.split_whitespace().next().unwrap_or("")
}

// Last item in the file, so nothing hides below it.
#[cfg(test)]
mod tests {
    use super::*; // private items are visible here

    // Name the condition and the outcome. No `test_` prefix: the output already
    // reads `tests::empty_input_has_no_word`.
    #[test]
    fn empty_input_has_no_word() {
        assert_eq!(first_word(""), "");
    }

    #[test]
    fn leading_whitespace_is_skipped() {
        assert_eq!(first_word("  hello world"), "hello");
    }
}
```

```rust,legacy
// lint: items_after_test_module, tests_outside_test_module, redundant_test_prefix
pub fn double(x: u32) -> u32 { x * 2 }

#[test] // outside any #[cfg(test)] module
fn test_double() { assert_eq!(double(2), 4); }

#[cfg(test)]
mod tests {
    #[test]
    fn test_double_again() { assert_eq!(super::double(3), 6); } // redundant prefix
}

pub fn triple(x: u32) -> u32 { x * 3 } // item after the test module
```

## Use one integration-test target

Every `.rs` file directly in `tests/` is compiled and linked as its own crate and binary, and Cargo runs these binaries serially. Ten small files cost ten links. Use one target instead: a directory with a `main.rs` that declares modules. Cargo discovers `tests/<name>/main.rs` as the test target `<name>`.

```text
tests/
└── it/
    ├── main.rs      # mod common; mod parsing; mod cli;
    ├── common.rs    # helpers shared by the modules; one crate, so no dead_code noise
    ├── parsing.rs
    └── cli.rs
```

- `cargo test --test it parsing::` runs one module. Integration tests see only `pub` items. If a test needs a private item, make it a unit test; don't widen visibility for tests.
- If you keep several files, put shared helpers in `tests/common/mod.rs`. A `tests/common.rs` becomes a test target of its own with an empty `running 0 tests` section. Every file that does `mod common;` also gets `dead_code` warnings for helpers it doesn't use.

## Keep logic out of `main.rs`

A crate with only `src/main.rs` cannot be imported from `tests/`, and its doc examples never run. Put the work in `src/lib.rs` as a function over `Read`/`Write` (see `test-seams`), and keep `main` as glue.

```rust
use std::io::{self, BufRead, Write};

/// Counts the lines of `input` and writes the total to `output`.
///
/// # Errors
///
/// Returns any I/O error from reading `input` or writing `output`.
pub fn run(input: impl BufRead, mut output: impl Write) -> io::Result<()> {
    let mut count = 0;
    for line in input.lines() {
        line?;
        count += 1;
    }
    writeln!(output, "{count}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_lines() {
        let mut out = Vec::new();
        run(&b"a\nb\n"[..], &mut out).unwrap();
        assert_eq!(out, b"2\n");
    }
}
```

```rust,ignore
// src/main.rs: nothing left to test except wiring
use std::process::ExitCode;

fn main() -> ExitCode {
    match my_tool::run(std::io::stdin().lock(), std::io::stdout().lock()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => { eprintln!("error: {e}"); ExitCode::FAILURE }
    }
}
```

To test the real executable (exit codes, argument parsing), start it from an integration test. Cargo builds the binary and sets `CARGO_BIN_EXE_<name>` for `tests/`:

```rust,ignore
// tests/it/cli.rs
let out = std::process::Command::new(env!("CARGO_BIN_EXE_my_tool")).output().unwrap();
assert!(out.status.success());
```

## Share test-only code deliberately

`#[cfg(test)]` is set only when the crate itself is built as a test. When `tests/` builds your library, it is **not** set, so a `#[cfg(test)]` helper is invisible to integration tests. Use `tests/it/common.rs` when the helper needs only the public API. For fakes or hooks that must live in the library, gate them behind a feature named `test-util` so production builds never contain them (`M-TEST-UTIL`), and enable it for tests with a dev-dependency on the package itself:

```toml
[features]
test-util = [] # fakes and inspection hooks; not for production use

[dev-dependencies]
my_crate = { path = ".", features = ["test-util"] }
```

`[[test]] required-features = ["test-util"]` also works, but a plain `cargo test` then silently skips that target.

## Run every target

```sh
cargo test --workspace --all-targets   # lib and bin unit tests, tests/, examples (built); not doctests
cargo test --workspace --doc           # doctests, library targets only
```

The first failing test binary stops the rest, including doctests. Add `--no-fail-fast` in CI to see every failure.

## Avoid

- Tests in the middle of a file, a `test_` prefix, or a `#[test]` outside the module: fix with the layout above.
- One `tests/*.rs` per topic, each with `mod common;`: merge into `tests/it/`.
- `pub` items or `#[cfg(test)]` accessors added only so a test can reach them: write a unit test instead.
- Logic in `main.rs`, then no tests for it.

## Verify

- `cargo clippy --workspace --all-targets -- -W clippy::tests_outside_test_module -W clippy::redundant_test_prefix -W clippy::items_after_test_module`. Clippy only sees test code with `--all-targets` or `--tests`. `redundant_test_prefix` needs clippy 1.88, `items_after_test_module` 1.71.
- `cargo test --workspace --all-targets` and `cargo test --workspace --doc` both pass.
- `cargo test` output lists a `Running tests/it/main.rs` section and no empty `common` target.
