---
name: test-assertions
description: Write assertions that explain their own failure, including should_panic with expected, ignored tests with a reason, Result-returning tests, matches!, and float comparisons.
rust-version: "1.70"
clippy-lints: [should_panic_without_expect, ignore_without_reason, bool_assert_comparison, assertions_on_result_states, manual_assert]
sources:
  - "The Rust Book, ch. 11 Writing Tests and Running Tests (MIT/Apache-2.0)"
  - "std docs: assert_matches!, core::panic::Location (#[track_caller]) (MIT/Apache-2.0)"
  - "Clippy lint docs and lint configuration (MIT/Apache-2.0)"
  - "Apollo Rust Best Practices handbook, ch. 5 (Apache-2.0): failure messages, one behavior per test"
  - "massimodeluisa rust-best-practices, quality.md (MIT)"
---

# Test Assertions

A failing test should say what was wrong without a debugger. Compare whole values with `assert_eq!`, pin panics to their message, return `Result` from tests that use `?`, and give every `#[ignore]` a reason. Unwrapping in test code is fine; the habits below are about the failure output.

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

/// Halves `n`.
///
/// # Panics
///
/// Panics if `n` is zero.
pub fn half_of_nonzero(n: u32) -> u32 {
    assert!(n != 0, "n must be non-zero");
    n / 2
}

#[cfg(test)]
mod tests {
    use super::*;

    // Whole-value comparison: a failure prints both sides, `Err` included.
    #[test]
    fn empty_input_is_an_error() {
        assert_eq!(parse_digit(""), Err(ParseError::Empty));
    }

    // Error type without PartialEq, or only the variant matters: matches! plus a
    // message. The message belongs to `assert!`, not to `matches!`.
    #[test]
    fn non_digit_names_the_character() {
        let err = parse_digit("x").unwrap_err();
        assert!(matches!(err, ParseError::BadDigit('x')), "got {err:?}");
    }

    // `?` keeps setup short. The test fails with `Error: <Debug of the Err>`.
    #[test]
    fn parses_a_digit() -> Result<(), ParseError> {
        assert_eq!(parse_digit("7")?, 7);
        Ok(())
    }

    // Pin the message, or any other panic (a typo'd index) also passes.
    #[test]
    #[should_panic(expected = "n must be non-zero")]
    fn zero_is_rejected() {
        half_of_nonzero(0);
    }

    #[test]
    #[ignore = "needs the 4 GB fixture; run with `cargo test -- --ignored`"]
    fn huge_input() {}
}
```

```rust,legacy
// lint: should_panic_without_expect, ignore_without_reason, bool_assert_comparison, assertions_on_result_states, manual_assert
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
        let n = parse("7").unwrap();
        if n != 7 { panic!("wrong value {n}"); } // assert_eq!(n, 7)
    }
}
```

- `expected` is a substring match. Pick the stable part of the message. Use `should_panic` only when the panic is the documented contract; return `Result` for errors. A `should_panic` test can't also return `Result`.
- `assert!(r.is_ok())` throws away the error. Use `r.unwrap()`, `?`, or `assert_eq!(r, Ok(..))`. For `is_err()`, use `assert_eq!(r, Err(..))` or `r.unwrap_err()`.
- A `?` failure shows the error but no line number. In a test with many fallible steps, use `.expect("what was being done")` so the panic carries a location.
- Never `#[ignore]` a flaky test to get green. Fix it (`test-isolation`) or delete it. Run ignored tests with `cargo test -- --ignored`, or everything with `-- --include-ignored`.

## Add context where the value alone is unclear

`assert_eq!(got, want, "input {input:?}")` takes a format string. Use it in loops and tables so the failing row is named (`meaningful-tests`).

For pattern checks on Rust 1.96+, `assert_matches!` (not in the prelude) prints the actual value and accepts guards and a message:

```rust
use std::assert_matches;

#[test]
fn non_digit_names_the_character() {
    let got: Result<u8, char> = Err('x');
    assert_matches!(got, Err(c) if c.is_alphabetic());
}
```

## Compare floats with a tolerance, from a helper

Exact `==` on computed floats fails on rounding. `f64::EPSILON` is the gap near 1.0, so it is the wrong tolerance for large values. Pick a tolerance that matches the computation. `#[track_caller]` makes a failure point at the calling test, not at the helper. A NaN never passes, because `NaN <= tol` is false.

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

If you enable `unwrap_used`, `expect_used`, `panic`, or `indexing_slicing`, exempt test code in `clippy.toml` instead of sprinkling `#[allow]`:

```toml
allow-unwrap-in-tests = true
allow-expect-in-tests = true
allow-panic-in-tests = true
allow-indexing-slicing-in-tests = true
```

`allow-panic-in-tests` needs clippy 1.80 and `allow-indexing-slicing-in-tests` 1.85. Helper functions that are not `#[test]` functions or inside `#[cfg(test)]` code, such as helpers in `tests/`, are still linted. Return `Result` or use `expect` there.

## Avoid

- `#[should_panic]` with no `expected`, and `#[ignore]` with no reason: both are bare attributes that hide intent.
- `assert!(a == b)` on values that are `Debug`: `assert_eq!` prints both sides.
- Many unrelated checks in one test. Several assertions are fine when they establish one behavior.
- `println!` as a check. Output is captured and the test still passes.

## Verify

- `cargo clippy --all-targets -- -W clippy::should_panic_without_expect -W clippy::ignore_without_reason -W clippy::bool_assert_comparison -W clippy::assertions_on_result_states -W clippy::manual_assert`. `ignore_without_reason` needs clippy 1.88. Lints see test code only with `--all-targets` or `--tests`.
- Break the code under test on purpose once and read the failure output: it should name the input and both values.
