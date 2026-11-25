---
name: testing
description: Choose and structure tests with the std harness (unit, integration, doc, Miri) and find the right testing guide for the task.
rust-version: "1.70"
clippy-lints: []
sources:
  - "The Rust Book, ch. 11 Writing Automated Tests (MIT/Apache-2.0)"
  - "Cargo Book: cargo test, Cargo Targets (MIT/Apache-2.0)"
  - "Microsoft Pragmatic Rust Guidelines: M-INTEGRATION-TESTS, M-TAUTOLOGICAL-TESTS, M-STATIC-VERIFICATION (MIT)"
  - "massimodeluisa rust-best-practices, quality.md: test layers (MIT)"
  - "Comprehensive Rust, Testing chapters (CC-BY-4.0 prose, Apache-2.0 code)"
---

# Testing

Rust's built-in harness covers most testing: `#[test]` functions, `cargo test`, and rustdoc examples. Tests run in parallel threads of one process, and a failing test is one that panics or returns `Err`. Most bad tests come from sharing state, restating the implementation, or code that can't be reached from a test. Read the guides below that match the task, then run both test commands under Commands below.

## Choose the layer

| Layer | Where | Checks |
| --- | --- | --- |
| Unit | `#[cfg(test)] mod tests` beside the code | private logic, edge cases, every error variant |
| Integration | `tests/` | the public API, and the real binary via `CARGO_BIN_EXE_<name>` (the bin name verbatim: `my-tool`, hyphen included) |
| Doc | `///` examples on public items | that documented usage compiles and runs |
| Miri | `cargo +nightly miri test` | UB and data races in `unsafe` code |

Property loops, regression tests, and stress tests are ordinary `#[test]`s; see `meaningful-tests` and `unsafe-and-concurrency-testing`.

## Routing

- `test-organization`: where to put unit tests, `tests/`, and shared helpers; splitting `main.rs` from `lib.rs`; which `cargo test` commands run what.
- `test-assertions`: writing the assertions themselves: `should_panic(expected)`, `#[ignore = "reason"]`, `Result`-returning tests, `matches!`, float tolerance, `clippy.toml` test exemptions.
- `doc-tests`: writing `# Examples` that compile and run, `?` in examples, `# Errors` / `# Panics`, edition 2024 combined doctests, and why binaries have none.
- `test-seams`: code that reads files, the clock, or the environment and is hard to test; injecting `Read`/`Write`, time, and closures instead of mocking.
- `test-isolation`: tests that pass alone but fail together, flake, collide on files or ports, mutate the environment, or use `sleep`.
- `meaningful-tests`: deciding what to test; boundaries, tables, std-only property loops, regression tests; spotting tests that restate the code.
- `unsafe-and-concurrency-testing`: tests for `unsafe` code or shared-state concurrency; running Miri; scoped-thread stress tests.

Performance measurement is not testing: see `measure-before-optimizing`.

## Defaults that apply everywhere

- Name tests for the condition and result (`empty_input_has_no_word`), not `test_foo`. The module path already prints.
- One behavior per test. Share setup in a helper, but keep the action and the assertion in the test body.
- Unwrapping in tests is fine and gives a location. Production code returns errors (see `error-values`).
- A test must be able to fail. Break the code once and watch it fail before trusting a new test (`meaningful-tests` has the mental version).
- Tests do not sleep, share files or ports, or change the process environment (see `test-isolation`). `-- --test-threads=1` is a diagnostic, not a fix.
- Enable clippy on test code with `cargo clippy --workspace --all-targets`; most test lints are silent without it. The guides' opt-in lints (pedantic, restriction) are hand-run suggestions.

## Commands

```sh
cargo test --workspace --all-targets --no-fail-fast  # lib/bin unit tests, tests/, benches in test mode; examples built only; no doctests
cargo test --workspace --doc                          # doctests, library targets only
cargo test --lib tests::name -- --exact --nocapture   # run one test, with its output
cargo test -- --show-output                           # also print the output of passing tests
```

Without `--no-fail-fast`, Cargo stops at the first failing test binary and never reaches the rest, doctests included.

## What `cargo test` skips

Examples are built but not run. Targets with `test = false`, or whose `required-features` are off, are skipped silently. A `#[bench]` function needs nightly.

> [!NOTE]
> The crates the community usually adds are out of scope for these guides: `tempfile` (temporary files and directories), `cargo-nextest` (faster runner, no doctests), `proptest` (property tests), `insta` (snapshots), `rstest` (parameterized tests), `mockall` (mocks), `loom` (concurrency model checking), `cargo-mutants` (mutation testing), and `cargo-llvm-cov` (coverage). Each guide gives the std approach. Don't add one without asking the user.
