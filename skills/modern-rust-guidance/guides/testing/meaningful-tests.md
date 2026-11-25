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
pub const SIDES: usize = 4;

pub fn fee(cents: u64) -> u64 { cents * 3 / 100 }

#[cfg(test)]
mod tests {
    use super::*;

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

These stay green when the code is wrong in the same way, and only add noise to later changes. Assert hand-computed values instead, including the rounding edge:

```rust
pub fn fee(cents: u64) -> u64 { cents * 3 / 100 }

#[cfg(test)]
mod tests {
    use super::*;

    // Worked by hand: 3% of $100.00, then the rounding edge around one cent.
    #[test]
    fn fee_examples() {
        assert_eq!((fee(10_000), fee(33), fee(34)), (300, 0, 1));
    }
}
```

Mental mutation check for any test: flip a `<` to `<=`, drop a `+ 1`, return a default. If no test fails, that line is uncovered. Don't write tests only to satisfy a coverage or mutation tool, and don't test constants, derives, trivial getters, or std itself.

## Cover boundaries in a table, and properties in a loop

Edge cases: empty, one element, exactly full, one over, zero, `MAX`, overflow, duplicates, unsorted input, NaN, and non-ASCII text (byte indexing panics inside a multi-byte character). Put cases in a table and name the row in the failure message. Where the answer is not a fixed value, check an invariant over every input of a small domain: all `u8`s, or all strings up to three characters over a four-character alphabet.

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

    #[test]
    fn truncate_returns_the_longest_fitting_prefix() {
        let alphabet = ['a', 'é', '日', '👍'];
        // Every string of 0 to 3 characters (85 strings), against every limit 0..=14.
        let mut layer = vec![String::new()];
        let mut all = layer.clone();
        for _ in 0..3 {
            layer = layer.iter().flat_map(|s| alphabet.map(|c| format!("{s}{c}"))).collect();
            all.extend(layer.iter().cloned());
        }
        for s in &all {
            for max in 0..=14 {
                let out = truncate(s, max);
                let ctx = format!("truncate({s:?}, {max}) = {out:?}");
                assert!(s.starts_with(out) && out.len() <= max, "{ctx}");
                // Longest: the next character would not have fit.
                if let Some(next) = s[out.len()..].chars().next() {
                    assert!(out.len() + next.len_utf8() > max, "{ctx}");
                }
            }
        }
    }
}
```

Other useful properties: round trip (`decode(encode(x)) == x`), idempotence (`f(f(x)) == f(x)`), and agreement with a slow, obviously correct reference. If the domain is too large to enumerate, use a fixed-seed generator and print the seed in the failure message so the run reproduces.

> [!NOTE]
> A property-testing crate (`proptest`) adds random generation, shrinking, and saved failures, and `cargo-mutants` automates the mutation check. Both are out of scope here; the std loop covers most needs.

## Name regression tests for the bug

After a fix, add the smallest input that triggered it, named for the symptom or the issue (`regression_truncate_inside_multibyte_char_panicked`). Confirm it fails on the old code before trusting it.

## Avoid

- Expected values computed by the formula under test, or pasted from the code's own output without checking them.
- Tests with no real assertion. `let _ = f(input);` only proves it does not panic; name it that way (`never_panics_on_empty_input`) if that is the intent. `assert!(x.is_some())` has the same weakness: it checks existence, not the value.
- Asserting on `Debug` output or private fields when a public result exists.
- Failure messages without the input.

## Verify

- `cargo clippy --workspace --all-targets` shows no `assertions_on_constants` (on by default). It catches `assert!(CONST == 4)`, not every tautology.
- Every failure message names the input (a table row, or a `ctx` string as above).
