---
name: type-size-and-layout
description: Shrink hot types and avoid large copies by boxing rare enum variants and large errors, using small indices, and guarding size_of at compile time.
rust-version: "1.80"
clippy-lints: [large_enum_variant, result_large_err, large_types_passed_by_value, large_futures, large_stack_arrays]
sources:
  - "Rust Performance Book: Type Sizes, Heap Allocations (Box) (MIT/Apache-2.0)"
  - "Microsoft Pragmatic Rust Guidelines: M-BOX-DST, M-ASYNC-STACK-SIZE, M-AVOID-INDIRECTION (MIT)"
  - "massimodeluisa rust-best-practices: performance.md (MIT)"
  - "std docs: mem::size_of, num::NonZero*, boxed slices (MIT/Apache-2.0)"
---

# Type Size and Layout

A value is as large as its largest part, and every move or return copies all of it. Shrinking a type that is created in bulk or moved often saves memory and copying. This only matters for hot or numerous types, so find them first (see `measure-before-optimizing`). To make immutable data compact, see `allocation-reuse`.

## Measure sizes, then guard them

`size_of::<T>()` (in the prelude since 1.80) gives the size. On nightly, `RUSTFLAGS=-Zprint-type-sizes cargo +nightly build --release` prints every type's layout, padding, and enum variant sizes. The compiler already reorders struct fields to minimize padding unless you use `#[repr(C)]`, so do not hand-sort fields.

Pin the size of a hot type so growth fails the build. Guard sizes with `<=`, since layout is not guaranteed across compiler versions, and restrict the check to one pointer width because `usize` and pointers vary:

```rust
use std::num::NonZeroU32;

pub struct Hot {
    pub id: u32,
    pub flags: u8,
    pub pos: [f32; 2],
    pub parent: Option<NonZeroU32>, // same size as u32: 0 is the None niche
    pub name: Box<str>,
}

#[cfg(target_pointer_width = "64")]
const _: () = assert!(size_of::<Hot>() <= 40, "Hot grew: check the layout before raising this");
```

## Box rare, large enum variants

An enum is as big as its biggest variant. Boxing the outlier makes every other value small, at the cost of one allocation when the big variant is built. That is a win when it is rare. Clippy's `large_enum_variant` fires when variants differ by 200 bytes or more (`enum-variant-size-threshold` in `clippy.toml`).

```rust
pub struct Header {
    pub fields: [u64; 40],
}

pub enum Event {
    Tick,
    Packet(Box<Header>), // Event stays small; only Packet pays for an allocation
}
```

## Keep `Result` error types small

`Result<T, E>` is at least as large as `E`, even on the `Ok` path, and it is copied on every return. `result_large_err` fires at 128 bytes (`large-error-threshold`). Box the error, or box its large part:

```rust
pub struct ParseError(Box<ParseErrorDetails>); // one word

pub struct ParseErrorDetails {
    pub context: [u8; 200],
    pub offset: usize,
}

pub fn parse(input: &str) -> Result<u32, ParseError> {
    input.parse().map_err(|_| {
        ParseError(Box::new(ParseErrorDetails { context: [0; 200], offset: 0 }))
    })
}
```

## Use smaller types where the range allows

Store indices and IDs as `u32` (or smaller) and widen with `usize::try_from` or `usize::from` at use sites. Check the range once when creating the ID (overflow must be an error, not a wrap), and use `NonZeroU32` so `Option<Id>` costs no extra space. For immutable owned sequences, `Box<[T]>` and `Box<str>` are two words instead of three. `Vec::into_boxed_slice` may reallocate to drop spare capacity.

## Keep big values off the stack

Pass large `Copy` types by reference, and keep large arrays out of locals. `large_types_passed_by_value` flags by-value arguments of 256 bytes or more (`pass-by-value-size-limit`) and skips exported functions. `large_stack_arrays` flags arrays over 16384 bytes by default (`array-size-threshold`). `[0u8; 1 << 20]` and `Box::new([0u8; N])` may build the array on the stack before moving it. Allocate directly instead:

```rust
pub fn zeroed_buffer() -> Box<[u8]> {
    vec![0u8; 1 << 20].into_boxed_slice() // heap from the start
}
```

For `async fn`, locals and arguments held across an `.await` become part of the future, so a large one makes every future of that function large. Check with `size_of_val(&fut)` in a test, move big buffers to the heap, or `Box::pin` the future. `large_futures` fires at 16 KiB.

## Avoid

```rust,legacy
// lint: large_enum_variant, result_large_err, large_types_passed_by_value, large_stack_arrays, large_futures
pub struct Header {
    pub fields: [u64; 40],
}

pub enum Event {
    Tick,
    Packet(Header), // every Event is 320+ bytes
}

pub struct BigErr {
    pub buf: [u8; 200],
}

pub fn check(x: u32) -> Result<u32, BigErr> {
    Ok(x) // Result is 200+ bytes on every return
}

#[derive(Clone, Copy)]
pub struct Matrix {
    pub m: [[f64; 8]; 8],
}

fn trace(m: Matrix) -> f64 {
    m.m[0][0] // take &Matrix
}

pub fn buffer() -> Vec<u8> {
    let buf = [0u8; 1 << 20]; // 1 MiB of stack; use vec![0u8; 1 << 20]
    buf.to_vec()
}

async fn read_big() -> u8 {
    let buf = [0u8; 20_000]; // lives across the await, so the future is 20 KB
    std::future::ready(()).await;
    buf[0]
}

pub async fn run() -> u8 {
    read_big().await
}
```

## Older MSRV

`size_of` joined the prelude in 1.80. Before that, write `use std::mem::{size_of, size_of_val};`.

## Verify

- `cargo clippy -- -W clippy::large_enum_variant -W clippy::result_large_err -W clippy::large_types_passed_by_value -W clippy::large_futures -W clippy::large_stack_arrays`. The first two are on by default and the other three are pedantic.
- Run the size guard on the architectures you ship, and confirm a boxed variant is rare in practice.
