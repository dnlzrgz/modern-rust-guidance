---
name: bounds-checks-and-iteration
description: Loop over slices and Vecs with iterators, zip, chunks, and up-front length checks so the compiler can drop per-element bounds checks.
rust-version: "1.88"
clippy-lints: [needless_range_loop, manual_memcpy, explicit_counter_loop, manual_slice_fill, chunks_exact_to_as_chunks]
sources:
  - "Rust Performance Book: Bounds Checks, Iterators (MIT/Apache-2.0)"
  - "std docs: slice::as_chunks, slice::chunks_exact, slice::copy_from_slice, slice::fill (MIT/Apache-2.0)"
---

# Bounds Checks and Iteration

Every `v[i]` is checked. Usually that costs little, and iterators are the cleanest way to remove it: `for x in &v` needs no check at all. Do not rewrite indexing code for speed unless a profile points at the loop. When it does, iterate, or prove the length to the compiler once before the loop. Use `unsafe` last.

## Iterate instead of indexing

```rust
pub fn dot(a: &[f32], b: &[f32]) -> f32 {
    // zip stops at the shorter slice. If a length mismatch is a bug, say so.
    assert_eq!(a.len(), b.len());
    a.iter().zip(b).map(|(x, y)| x * y).sum()
}

pub fn gather(table: &[u32], idx: &[u8], out: &mut [u32]) {
    let out = &mut out[..idx.len()]; // one length check; panics if `out` is too short
    for (o, &k) in out.iter_mut().zip(idx) {
        *o = table[usize::from(k)]; // data-dependent index: this check stays
    }
}
```

Need the position? Use `for (i, x) in v.iter().enumerate()`, not a hand-kept counter. Indexing and iterating are not equivalent: `zip` silently truncates where indexing would panic, so keep an `assert_eq!` or `debug_assert_eq!` on the lengths when they must match.

## Prove the length once

When you must index, tell the compiler the bounds first. A re-slice or an `assert!` before the loop can replace many checks with one:

```rust
pub fn add_shifted(dst: &mut [u32], src: &[u32]) {
    // dst[i + 1] reads one past i; prove the bounds once, not per element
    assert!(dst.len() > src.len());
    for i in 0..src.len() {
        dst[i + 1] += src[i];
    }
}
```

On Rust 1.100 (x86-64, `-O`), the `assert!` version of this loop compiles without a bounds-check call and the plain version keeps one. Confirm for your code and target by looking at the assembly (`cargo rustc --release -- --emit asm`), not by assuming.

## Process fixed-size chunks

`as_chunks` (1.88) splits a slice into `&[[T; N]]` plus a remainder, with no length checks inside the chunks. It panics if `N` is 0. `chunks_exact(n)` is the older form (see Older MSRV). Prefer both to `chunks(n)` when you do not need a short final chunk.

```rust
pub fn sum_le_u32(bytes: &[u8]) -> u32 {
    let (chunks, rest) = bytes.as_chunks::<4>(); // &[[u8; 4]] and the 0..=3 left-over bytes
    let mut sum = 0u32;
    for chunk in chunks {
        sum = sum.wrapping_add(u32::from_le_bytes(*chunk));
    }
    for &b in rest {
        sum = sum.wrapping_add(u32::from(b));
    }
    sum
}
```

For neighbors use `windows(n)` instead of `v[i - 1]` and `v[i + 1]`. To copy or set a range, use `dst[..n].copy_from_slice(&src[..n])` (panics if the lengths differ) and `slice.fill(value)` (1.50).

## Avoid

```rust,legacy
// lint: needless_range_loop, manual_memcpy, explicit_counter_loop, manual_slice_fill
pub fn sum(v: &[u32]) -> u32 {
    let mut s = 0;
    for i in 0..v.len() {
        s += v[i]; // for x in v / v.iter().sum()
    }
    s
}

pub fn copy(dst: &mut [u32], src: &[u32]) {
    for i in 0..src.len() {
        dst[i] = src[i]; // dst[..src.len()].copy_from_slice(src)
    }
}

pub fn weighted(a: &[u32]) -> u32 {
    let mut i = 0;
    let mut s = 0;
    for x in a {
        s += x * i;
        i += 1; // for (i, x) in a.iter().enumerate()
    }
    s
}

pub fn zeroed() -> [u32; 8] {
    let mut a = [1; 8];
    for i in 0..a.len() {
        a[i] = 0; // a.fill(0)
    }
    a
}
```

- `get_unchecked` and `get_unchecked_mut` to skip checks. They are `unsafe` and undefined behavior if the index is out of range. Reach for them only after iteration, re-slicing, and an up-front `assert!` failed and a benchmark proves the gain. Add a `// SAFETY:` comment stating the invariant, and keep a checked version behind a `debug_assert!` or test.
- Assuming an iterator chain beats a simple loop. Both usually optimize well, so benchmark when it matters.

## Older MSRV

Before 1.88 there is no `as_chunks`. Use `chunks_exact`, which exists since 1.31, and read the tail from `remainder()`. Clippy's `chunks_exact_to_as_chunks` suggests `as_chunks` only when the project's MSRV is 1.88 or newer (it reads `rust-version`, or `msrv` in `clippy.toml`):

```rust,legacy
// lint: chunks_exact_to_as_chunks
pub fn sum_le_u32(bytes: &[u8]) -> u32 {
    let chunks = bytes.chunks_exact(4);
    let rest = chunks.remainder(); // take it before the loop consumes `chunks`
    let mut sum = 0u32;
    for chunk in chunks {
        let arr: [u8; 4] = chunk.try_into().expect("chunk is 4 bytes");
        sum = sum.wrapping_add(u32::from_le_bytes(arr));
    }
    for &b in rest {
        sum = sum.wrapping_add(u32::from(b));
    }
    sum
}
```

## Verify

- `cargo clippy` (these lints are on by default): `needless_range_loop`, `manual_memcpy`, `explicit_counter_loop`, `manual_slice_fill`, and `chunks_exact_to_as_chunks` (it respects the project's MSRV).
- A test with mismatched lengths and with lengths that are not multiples of the chunk size, since `zip` and chunking change edge-case behavior.
- A before/after measurement on a release build (`measure-before-optimizing`). Without one, keep the simpler code.
