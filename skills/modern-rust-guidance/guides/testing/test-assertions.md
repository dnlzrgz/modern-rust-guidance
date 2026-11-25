---
name: test-assertions
description: Write assertions that explain their own failure, including should_panic with expected, ignored tests with a reason, Result-returning tests, matches!, and float comparisons.
rust-version: "1.70"
clippy-lints: [should_panic_without_expect, ignore_without_reason, bool_assert_comparison, assertions_on_result_states]
sources:
  - "The Rust Book, ch. 11 Writing Tests and Running Tests (MIT/Apache-2.0)"
  - "std docs: assert_matches!, core::panic::Location (#[track_caller]) (MIT/Apache-2.0)"
  - "Clippy lint docs and lint configuration (MIT/Apache-2.0)"
  - "Apollo Rust Best Practices handbook, ch. 5 (Apache-2.0): failure messages, one behavior per test"
  - "massimodeluisa rust-best-practices, quality.md (MIT)"
---

# Test Assertions

A failing test should say what was wrong without a debugger. Compare whole values with `assert_eq!`, pin panics to their message, return `Result` from tests that use `?`, and give every `#[ignore]` a reason.

## Compare values, not states

```rust
#[derive(Debug, PartialEq)]
pub enum ParseError {
    Empty,
    BadDigit(char),
}

pub fn parse_digit(s: &str) -> Result<u8, ParseError> {
    let c = s.chars().next().ok_or(ParseError::Empty)?;
    c.to_digit(10)
        .and_then(|d| u8::try_from(d).ok())
        .ok_or(ParseError::BadDigit(c))
}

/// Halves `n`. Panics if `n` is zero.
pub fn half_of_nonzero(n: u32) -> u32 {
    assert!(n != 0, "n must be non-zero");
    n / 2
}

#[cfg(test)]
mod tests {
    use super::*;

    // A failure prints both sides, `Err` included.
    #[test]
    fn empty_input_is_an_error() {
        assert_eq!(parse_digit(""), Err(ParseError::Empty));
    }

    // No PartialEq, or only the variant matters: `matches!` plus a message
    // (the message belongs to `assert!`).
    #[test]
    fn non_digit_names_the_character() {
        let err = parse_digit("x").unwrap_err();
        assert!(matches!(err, ParseError::BadDigit('x')), "got {err:?}");
    }

    // `?` keeps setup short; a failure prints `Error: <Debug of the Err>`.
    #[test]
    fn parses_a_digit() -> Result<(), ParseError> {
        assert_eq!(parse_digit("7")?, 7);
        Ok(())
    }

    #[test]
    #[should_panic(expected = "n must be non-zero")]
    fn zero_is_rejected() {
        half_of_nonzero(0);
    }

    #[test]
    #[ignore = "needs the 4 GB fixture"]
    fn huge_input() {}
}
```

```rust,legacy
// lint: should_panic_without_expect, ignore_without_reason, bool_assert_comparison, assertions_on_result_states
pub fn half(n: u32) -> u32 { assert!(n != 0); n / 2 }
pub fn parse(s: &str) -> Result<u8, std::num::ParseIntError> { s.parse() }

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[should_panic] // passes for ANY panic
    fn zero_is_rejected() { half(0); }
    #[test]
    #[ignore] // why? for how long?
    fn slow() {}
    #[test]
    fn parses() {
        assert!(parse("7").is_ok()); // hides the error value on failure
        assert_eq!(half(4) == 2, true); // use assert!(half(4) == 2)
    }
}
```

- `expected` is a substring match. Pick the stable part of the message. Use `should_panic` only when the panic is the documented contract; return `Result` for errors.
- `assert!(r.is_ok())` throws away the error: use `r.unwrap()`, `?`, or `assert_eq!(r, Ok(..))`. A `?` failure shows no line number, so with many fallible steps use `.expect("what was being done")`.
- `assert!(r.is_err())` passes for any error, so a wrong failure still passes. Assert which error: `assert!(matches!(r, Err(ParseError::BadDigit('x'))), "got {r:?}")`, or for `io::Error` and other types with a `kind()`, `assert_eq!(r.unwrap_err().kind(), ErrorKind::NotFound)`. Clippy's `assertions_on_result_states` (under Verify) flags both `is_ok()` and `is_err()` forms.
- Never `#[ignore]` a flaky test to get green: fix it (see `test-isolation`) or delete it. `cargo test -- --ignored` runs only ignored tests, `-- --include-ignored` all.

## Add context where the value alone is unclear

`assert_eq!(got, want, "input {input:?}")` takes a format string. Use it in loops and tables so the failing row is named (see `meaningful-tests`).

`std::assert_matches!` (needs 1.96; not in the prelude) prints the actual value and accepts guards and a message: `assert_matches!(got, Err(c) if c.is_alphabetic())`. On older toolchains use `assert!(matches!(..), "got {got:?}")` as above; `clippy::incompatible_msrv` flags `assert_matches!` when the project's MSRV is older.

## Compare floats with a tolerance, from a helper

Exact `==` on computed floats fails on rounding, and `f64::EPSILON` (the gap near 1.0) is the wrong tolerance for large values: pick one that matches the computation. `#[track_caller]` makes a failure point at the calling test. A NaN never passes, because `NaN <= tol` is false.

```rust
#[cfg(test)]
mod tests {
    #[track_caller]
    fn assert_close(actual: f64, expected: f64, tol: f64) {
        assert!((actual - expected).abs() <= tol, "{actual} is not within {tol} of {expected}");
    }

    #[test]
    fn sum_of_tenths() {
        assert_close((0..10).map(|_| 0.1).sum::<f64>(), 1.0, 1e-9);
    }
}
```

## Let tests unwrap, keep production strict

If you enable `unwrap_used`, `expect_used`, `panic`, or `indexing_slicing` (all restriction lints), exempt test code in `clippy.toml` instead of sprinkling `#[allow]`:

```toml
allow-unwrap-in-tests = true
allow-expect-in-tests = true
allow-panic-in-tests = true
allow-indexing-slicing-in-tests = true
```

`allow-panic-in-tests` needs clippy 1.80+ and `allow-indexing-slicing-in-tests` 1.85+. Helpers outside `#[cfg(test)]` code, such as those in `tests/`, are still linted: return `Result` or use `expect` there.

## Avoid

- Bare `#[should_panic]` and `#[ignore]`, `assert!(a == b)` on `Debug` values, and `println!` as a check (output is captured and the test passes).

## Verify

- `cargo clippy --workspace --all-targets -- -W clippy::should_panic_without_expect -W clippy::ignore_without_reason -W clippy::assertions_on_result_states`. All three are opt-in (`should_panic_without_expect` and `ignore_without_reason` are pedantic, `assertions_on_result_states` is restriction); `bool_assert_comparison` is on by default. `ignore_without_reason` needs clippy 1.88+. Optional, also pedantic: `clippy::manual_assert` (`if c { panic!() }` becomes `assert!`).
- Break the code under test on purpose once and read the failure output: it should name the input and both values.
