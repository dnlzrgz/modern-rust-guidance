---
name: meaningful-tests
description: Write tests that fail when behavior breaks, not tautologies that restate the code, using boundary cases, tables, std-only property loops, and regression tests.
rust-version: "1.70"
clippy-lints: [assertions_on_constants]
sources:
  - "Microsoft Pragmatic Rust Guidelines: M-TAUTOLOGICAL-TESTS (MIT)"
  - "massimodeluisa rust-best-practices, quality.md: test layers and boundary cases (MIT)"
  - "Apollo Rust Best Practices handbook, ch. 5: naming, one behavior per test (Apache-2.0)"
  - "The Rust Book, ch. 11 (MIT/Apache-2.0)"
  - "Clippy lint docs: assertions_on_constants (MIT/Apache-2.0)"
---

# Meaningful Tests

A test is worth keeping if a plausible bug makes it fail. Generated tests often pass by construction: they copy a constant, or compute the expected value with the same formula as the code. Pick expected values independently (hand-computed examples, properties, a simple reference), cover the edges where bugs live, and name the case that failed.

## Do not restate the code

```rust,legacy
// lint: assertions_on_constants
pub const CHECKPOINTS: [u32; 4] = [0, 90, 180, 270];
pub const SIDES: usize = 4;

pub fn fee(cents: u64) -> u64 { cents * 3 / 100 }

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checkpoints_are_correct() {
        assert_eq!(CHECKPOINTS, [0, 90, 180, 270]); // the constant, again
    }

    #[test]
    fn side_count() {
        assert!(SIDES == 4); // true by construction; clippy flags it
    }

    #[test]
    fn fee_matches() {
        for c in 0..1000 {
            assert_eq!(fee(c), c * 3 / 100); // the implementation, again
        }
    }
}
```

These stay green when the code is wrong in the same way, and only add noise to later changes. Assert a property of the constants and hand-computed values for the function:

```rust
pub const CHECKPOINTS: [u32; 4] = [0, 90, 180, 270];

pub fn fee(cents: u64) -> u64 { cents * 3 / 100 }

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checkpoints_ascend_with_equal_gaps() {
        let gap = CHECKPOINTS[1] - CHECKPOINTS[0];
        assert!(CHECKPOINTS.windows(2).all(|w| w[1] - w[0] == gap));
    }

    // Worked by hand: 3% of $100.00, then the rounding edge around one cent.
    #[test]
    fn fee_examples() {
        assert_eq!((fee(10_000), fee(33), fee(34)), (300, 0, 1));
    }
}
```

Mental mutation check for any test: flip a `<` to `<=`, drop a `+ 1`, return a default. If no test fails, that line is uncovered. Don't write tests only to satisfy a coverage or mutation tool, and don't test derives, trivial getters, or std itself.

## Cover boundaries in a table, and properties in a loop

Edge cases: empty, one element, exactly full, one over, zero, `MAX`, overflow, duplicates, unsorted input, NaN, and non-ASCII text (byte indexing panics inside a multi-byte character). Put cases in a table and name the row in the failure message. Where the answer is not a fixed value, check an invariant over many inputs from a seeded generator. The seed is a constant so failures reproduce, and the message prints it. For small domains (all `u8`, all `u16`), loop over every value instead.

```rust
/// Longest prefix of `s` that fits in `max_bytes` without splitting a character.
pub fn truncate(s: &str, max_bytes: usize) -> &str {
    if s.len() <= max_bytes {
        return s;
    }
    let mut end = max_bytes;
    while !s.is_char_boundary(end) {
        end -= 1;
    }
    &s[..end]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn truncate_boundaries() {
        // (input, limit, expected)
        let cases = [
            ("", 0, ""),       // empty
            ("abc", 3, "abc"), // exactly fits
            ("abc", 2, "ab"),  // one over
            ("é", 1, ""),      // limit inside a 2-byte character
            ("日本", 4, "日"),  // 3-byte characters
            ("👍", 3, ""),     // 4-byte character
        ];
        for (input, limit, want) in cases {
            assert_eq!(truncate(input, limit), want, "truncate({input:?}, {limit})");
        }
    }

    // Deterministic xorshift generator for tests. Not for anything else.
    fn next(state: &mut u64) -> u64 {
        *state ^= *state >> 12;
        *state ^= *state << 25;
        *state ^= *state >> 27;
        state.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    #[test]
    fn truncate_returns_the_longest_fitting_prefix() {
        const SEED: u64 = 0x9E37_79B9_7F4A_7C15;
        let (mut state, alphabet) = (SEED, ['a', 'é', '日', '👍']);
        for case in 0..2_000 {
            let len = next(&mut state) % 12;
            let s: String = (0..len).map(|_| alphabet[(next(&mut state) % 4) as usize]).collect();
            let max = (next(&mut state) % 20) as usize;
            let out = truncate(&s, max);
            let ctx = format!("seed {SEED:#x} case {case}: truncate({s:?}, {max}) = {out:?}");
            assert!(s.starts_with(out) && out.len() <= max, "{ctx}");
            // Longest: the next character would not have fit.
            if let Some(next_char) = s[out.len()..].chars().next() {
                assert!(out.len() + next_char.len_utf8() > max, "{ctx}");
            }
        }
    }
}
```

Other useful properties: round trip (`decode(encode(x)) == x`), idempotence (`f(f(x)) == f(x)`), and agreement with a slow, obviously correct reference.

> [!NOTE]
> A property-testing crate (`proptest`) adds shrinking and saved failures, and `cargo-mutants` automates the mutation check. Both are out of scope here; the std loop covers most needs.

## Name regression tests for the bug

After a fix, add the smallest input that triggered it, named for the symptom or the issue (`regression_truncate_inside_multibyte_char_panicked`). Confirm it fails on the old code before trusting it.

## Avoid

- Expected values computed by the formula under test, or pasted from the code's own output without checking them.
- Tests with no assertion. `let _ = f(input);` only proves it does not panic; name it that way (`never_panics_on_empty_input`) if that is the intent.
- Asserting on `Debug` output or private fields when a public result exists.
- Random input without a fixed seed, and failure messages without the input.

## Verify

- `cargo clippy --all-targets -- -W clippy::assertions_on_constants` is clean. It catches `assert!(CONST == 4)`, not every tautology.
- For each new test, break the code under test in one plausible way and confirm the test fails.
