---
name: integer-arithmetic
description: Do integer math safely and readably: overflow handling, rounding up, differences, averages, multiples, logarithms, clamping, and numeric conversions.
rust-version: "1.87"
clippy-lints: [manual_div_ceil, manual_abs_diff, manual_midpoint, manual_is_multiple_of, manual_ilog2, legacy_numeric_constants, manual_rem_euclid, manual_clamp, cast_possible_truncation]
sources:
  - "std docs: integer primitive types (MIT/Apache-2.0)"
  - "Rust release notes 1.60, 1.67, 1.73, 1.84, 1.85, 1.87, 1.91 (MIT/Apache-2.0)"
  - "Clippy lint docs and clippy_utils msrvs table (MIT/Apache-2.0)"
---

# Integer Arithmetic

Plain `+`, `-`, `*` panic on overflow in debug builds and silently wrap in release builds, and `as` silently truncates. Say what you mean with a named method: `checked_*` when overflow is an error, `saturating_*` when clamping is right, `wrapping_*` when modular arithmetic is the point. Many hand-rolled formulas now have a std method; models keep writing the formulas. For `Option` plumbing around `checked_*`, see `early-return-patterns`.

## State the overflow policy

Use `checked_*` on sizes, offsets, and anything derived from input. `strict_*` (1.91, for example `strict_add`) panics on overflow in *every* build profile, which suits internal invariants.

```rust
pub fn total_size(count: u32, elem: u32, header: u32) -> Option<u32> {
    count.checked_mul(elem)?.checked_add(header)
}

pub fn dec(n: u32) -> u32 {
    n.saturating_sub(1) // stops at 0 instead of panicking or wrapping
}

pub fn advance(pos: usize, delta: isize) -> Option<usize> {
    pos.checked_add_signed(delta) // mixed signedness (1.66)
}
```

## Use the std method, not the formula

Several of these exist **only for unsigned types**. Signed `div_ceil`, `next_multiple_of`, and `is_multiple_of` are not stable, so do not suggest them for `i32`/`i64`.

| Instead of | Use | Since |
| --- | --- | --- |
| `(a + b - 1) / b` | `a.div_ceil(b)` (unsigned) | 1.73 |
| `((a + b - 1) / b) * b` | `a.next_multiple_of(b)` (unsigned) | 1.73 |
| `if a > b { a - b } else { b - a }` | `a.abs_diff(b)`, returns the unsigned type | 1.60 |
| `(a + b) / 2` (can overflow) | `a.midpoint(b)`, rounds toward zero (unsigned, floats 1.85; signed 1.87) | 1.85 / 1.87 |
| `n % d == 0` | `n.is_multiple_of(d)` (unsigned; `d == 0` does not panic) | 1.87 |
| `31 - n.leading_zeros()` | `n.ilog2()`, `ilog10()`, `ilog(base)`; `checked_ilog2` returns `None` for 0 | 1.67 |
| `(n as f64).sqrt() as u64` | `n.isqrt()`; `checked_isqrt` for signed | 1.84 |
| `x.max(lo).min(hi)` | `x.clamp(lo, hi)`; panics if `lo > hi` | 1.50 |
| `((x % m) + m) % m` | `x.rem_euclid(m)` | 1.38 |

```rust
pub fn pages(len: usize, per_page: usize) -> usize {
    len.div_ceil(per_page)
}

pub fn align_up(addr: u64, align: u64) -> u64 {
    addr.next_multiple_of(align)
}

pub fn mid(lo: usize, hi: usize) -> usize {
    lo.midpoint(hi) // no `lo + hi` overflow in a binary search
}

pub fn is_leap_year(y: u32) -> bool {
    y.is_multiple_of(4) && (!y.is_multiple_of(100) || y.is_multiple_of(400))
}

pub fn bit_len(n: u32) -> u32 {
    n.checked_ilog2().map_or(0, |b| b + 1)
}

pub fn wrap_index(i: i32, len: i32) -> i32 {
    i.rem_euclid(len) // always in 0..len, even for negative i
}

pub fn gap(a: u32, b: u32) -> u32 {
    a.abs_diff(b)
}
```

## Convert deliberately

Lossless: `u64::from(x)`. Fallible: `u16::try_from(n)` or `n.try_into()`, handle the `Err`. `as` is for bit reinterpretation or when truncation is the intent. Since 1.87, `cast_signed()` / `cast_unsigned()` express a same-width sign reinterpretation, so `as` is not needed there. A float-to-int `as` saturates and maps NaN to 0.

```rust
pub fn widen(n: u32) -> u64 {
    u64::from(n) // `as` would also compile, but hides a later type change
}

pub fn narrow(n: usize) -> Result<u16, std::num::TryFromIntError> {
    u16::try_from(n)
}

pub fn as_signed(hash: u32) -> i32 {
    hash.cast_signed() // 1.87; not `hash as i32`
}
```

## Avoid

```rust,legacy
// lint: manual_div_ceil, manual_abs_diff, manual_is_multiple_of, manual_midpoint, manual_clamp, manual_ilog2
pub fn a(len: usize, n: usize) -> usize {
    (len + n - 1) / n
}
pub fn b(a: u32, b: u32) -> u32 {
    if a > b { a - b } else { b - a }
}
pub fn c(n: u32) -> bool {
    n % 4 == 0
}
pub fn d(a: usize, b: usize) -> usize {
    (a + b) / 2
}
pub fn e(x: i32) -> i32 {
    x.max(0).min(100)
}
pub fn f(n: u32) -> u32 {
    31 - n.leading_zeros()
}
```

```rust,legacy
// lint: legacy_numeric_constants, cast_possible_truncation, manual_rem_euclid
pub fn g() -> u32 {
    std::u32::MAX // u32::MAX; likewise `i64::max_value()` -> i64::MAX
}
pub fn h(n: u64) -> u32 {
    n as u32 // silently drops the high bits
}
pub fn i(x: i32) -> i32 {
    ((x % 7) + 7) % 7
}
```

Also avoid `as f64` square roots and `as usize` on values that may be negative.

## Older MSRV

- **Below 1.85 (unsigned, floats) or 1.87 (signed):** no `midpoint`. For unsigned use `a / 2 + b / 2 + (a % 2 + b % 2) / 2`, or widen to a larger type.
- **Below 1.87:** `is_multiple_of` and `cast_signed`/`cast_unsigned` do not exist; use `% d == 0` (guard `d != 0`) and `as`.
- **Below 1.84:** no `isqrt`; avoid float round-trips for large values and use a checked integer search.
- **Below 1.73:** no `div_ceil` or `next_multiple_of`; use `a / b + usize::from(a % b != 0)`.
- **Below 1.67:** no `ilog*`. `abs_diff` needs 1.60.
- Clippy suggests each method only when the MSRV allows it.

## Verify

- `cargo clippy -- -W clippy::manual_div_ceil -W clippy::manual_abs_diff -W clippy::manual_midpoint -W clippy::manual_is_multiple_of -W clippy::manual_ilog2 -W clippy::legacy_numeric_constants -W clippy::manual_rem_euclid -W clippy::manual_clamp`.
- `manual_midpoint`, `manual_ilog2`, and `cast_possible_truncation` are pedantic: enable them explicitly. Expect `cast_possible_truncation` to flag deliberate hashing or bit-twiddling casts; annotate those with `#[expect(clippy::cast_possible_truncation, reason = "…")]`.
- Add boundary tests: `0`, `1`, `T::MAX`, and for signed code `T::MIN` and negative inputs.
