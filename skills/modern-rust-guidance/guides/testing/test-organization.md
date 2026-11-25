---
name: test-organization
description: Organize unit tests, integration tests in tests/, and a testable binary (lib.rs plus main.rs) in a Cargo package, and run all of them.
rust-version: "1.70"
clippy-lints: [items_after_test_module, redundant_test_prefix]
sources:
  - "The Rust Book, ch. 11 Test Organization (MIT/Apache-2.0)"
  - "Cargo Book: Cargo Targets, cargo test (MIT/Apache-2.0)"
  - "Microsoft Pragmatic Rust Guidelines: M-INTEGRATION-TESTS, M-TEST-UTIL (MIT)"
  - "Comprehensive Rust: Unit Tests, Other Types of Tests (CC-BY-4.0 prose, Apache-2.0 code)"
  - "Clippy lint docs: items_after_test_module, redundant_test_prefix (MIT/Apache-2.0)"
---

# Test Organization

Unit tests go in a `#[cfg(test)] mod tests` at the end of the file they test. Tests that only use the public API go in `tests/`. Anything a test must reach lives in `src/lib.rs`, never in `main.rs`. For doc examples see `doc-tests`; for fakes and injected dependencies see `test-seams`. The commands that run everything are in `testing`. Don't reorganize an existing test layout unless asked.

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
// lint: items_after_test_module, redundant_test_prefix
pub fn double(x: u32) -> u32 { x * 2 }

#[test] // outside any #[cfg(test)] module (tests_outside_test_module)
fn test_double() { assert_eq!(double(2), 4); }

#[cfg(test)]
mod tests {
    #[test]
    fn test_double_again() { assert_eq!(super::double(3), 6); } // redundant prefix
}

pub fn triple(x: u32) -> u32 { x * 3 } // item after the test module
```

## Use one integration-test target for new suites

Every `.rs` file directly in `tests/` is compiled and linked as its own crate and binary, and Cargo runs these binaries serially. For a new suite of more than about two files, use one target instead: a directory with a `main.rs` that declares modules. Cargo discovers `tests/<name>/main.rs` as the test target `<name>`.

```text
tests/
└── it/
    ├── main.rs      # mod common; mod parsing; mod cli;
    ├── common.rs    # helpers shared by the modules; one crate, so no dead_code noise
    ├── parsing.rs
    └── cli.rs
```

- `cargo test --test it parsing::` runs one module. Integration tests see only `pub` items. If a test needs a private item, make it a unit test; don't widen visibility for tests.
- With several separate files, put shared helpers in `tests/common/mod.rs`. A `tests/common.rs` becomes a test target of its own with an empty `running 0 tests` section, and every file that does `mod common;` gets `dead_code` warnings for helpers it doesn't use.
- Exception: a test that exercises process-global state (a `static`, the environment) gets its own `tests/<name>.rs`, because a separate binary is a separate process (see `test-isolation`).

## Keep logic out of `main.rs`

A crate with only `src/main.rs` cannot be imported from `tests/`, and its doc examples never run. Put the work in `src/lib.rs` as a function over `Read`/`Write` and keep `main` as glue. For a complete testable function and its unit test, see `test-seams`.

```rust,ignore
// src/lib.rs
pub fn run(input: impl BufRead, output: impl Write) -> io::Result<()> { /* ... */ }

// src/main.rs: nothing left to test except wiring
fn main() -> std::process::ExitCode {
    match my_tool::run(std::io::stdin().lock(), std::io::stdout().lock()) {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(e) => { eprintln!("error: {e}"); std::process::ExitCode::FAILURE }
    }
}
```

To test the real executable (exit codes, argument parsing), start it from an integration test. Cargo builds the binary and sets `CARGO_BIN_EXE_<name>` for `tests/`. The name is the bin target's name verbatim, hyphens included (`my-tool`), while the crate path is `my_tool`:

```rust,ignore
// tests/it/cli.rs
let out = std::process::Command::new(env!("CARGO_BIN_EXE_my-tool")).output().unwrap();
assert!(out.status.success());
```

## Share test-only code deliberately

`#[cfg(test)]` is set only when the crate itself is built as a test. When `tests/` builds your library it is **not** set, so a `#[cfg(test)]` helper is invisible there. Use `tests/it/common.rs` when the helper needs only the public API. Fakes or hooks that must live in the library go behind a `test-util` feature (`M-TEST-UTIL`) so production builds never contain them, enabled for tests by a dev-dependency on the package itself:

```toml
[features]
test-util = [] # fakes and inspection hooks; not for production use

[dev-dependencies]
my_crate = { path = ".", features = ["test-util"] }
```

Benches and examples are not `cfg(test)` either, so the same `test-util` feature (or a similarly named one) is the legitimate way to give them fixtures and hooks. Dev-dependencies are available to benches and examples too, so the self dev-dependency above enables the feature for them. Don't gate such helpers on `#[cfg(test)]`: the bench would fail to compile.

`[[test]] required-features = ["test-util"]` also works, but a plain `cargo test` then silently skips that target.

## Avoid

- Tests in the middle of a file, a `test_` prefix, or a `#[test]` outside the `#[cfg(test)]` module in `src/`.
- `pub` items or `#[cfg(test)]` accessors added only so a test can reach them: write a unit test instead.
- Logic in `main.rs`, then no tests for it.

## Verify

- `cargo clippy --workspace --all-targets -- -W clippy::redundant_test_prefix`. It is opt-in and needs clippy 1.88+; `items_after_test_module` is on by default. Clippy only sees test code with `--all-targets` or `--tests`.
- `-W clippy::tests_outside_test_module` (restriction) is useful for `src/` only. It also fires on every test in `tests/*.rs`, which is the correct layout, so ignore those hits. That is why `mrg scan` does not enable it.
- `cargo test` output lists a `Running tests/it/main.rs` section and no empty `common` target.
