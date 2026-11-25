---
name: string-building
description: Build strings efficiently by writing into one String with write!, join, and collect instead of format! in loops.
rust-version: "1.70"
clippy-lints: [format_collect, format_push_string, format_in_format_args, to_string_in_format_args, useless_format]
sources:
  - "Rust Performance Book: Heap Allocations (String, format!) (MIT/Apache-2.0)"
  - "Microsoft Pragmatic Rust Guidelines: M-HOTPATH, M-LOG-OVERHEAD (MIT)"
  - "std docs: fmt::Write, format_args!, slice::join (MIT/Apache-2.0)"
---

# String Building

`format!` always allocates a new `String`. One call is fine. In a loop, or nested inside another `format!` or `push_str`, it allocates and frees a temporary per iteration. Write straight into one `String` with `write!`, use `join` or `collect` for sequences, and pass `format_args!` when a callee only needs to display something. For buffers reused across iterations see `allocation-reuse`. For printing, see `buffered-io`.

## Append with `write!`, not `push_str(&format!(..))`

Import `std::fmt::Write` (as `_` to avoid clashing with `io::Write`). It formats directly into the destination:

```rust
use std::fmt::Write as _;

pub fn describe(items: &[(&str, u32)]) -> String {
    let mut out = String::with_capacity(items.len() * 16);
    for (name, qty) in items {
        // The Result is Err only if a Display impl fails, where format! would panic too
        write!(out, "{name}={qty};").expect("Display impl failed");
    }
    out
}
```

## Build from sequences with `join`, `collect`, and one buffer

`fields.join(",")` and `iter.collect::<String>()` (`String` implements `FromIterator` for `char`, `&str`, and `String`) need no per-element temporary. Mapping each element through `format!` first creates one throwaway `String` per element. Write into one buffer instead:

```rust
use std::fmt::Write as _;

pub fn hex(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        write!(s, "{b:02x}").expect("Display impl failed"); // not map(format!).collect::<String>()
    }
    s
}
```

## Write directly in `Display` impls and take `format_args!`

Inside `fmt`, write to the formatter. Do not build a `String` and then write it. When a function only needs to show a message, take `fmt::Arguments` and let the caller pass `format_args!`: nothing is allocated.

```rust
use std::fmt;

pub struct Point {
    pub x: i32,
    pub y: i32,
}

impl fmt::Display for Point {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "({}, {})", self.x, self.y) // not f.write_str(&format!(..))
    }
}

pub fn log(args: fmt::Arguments<'_>) {
    eprintln!("{args}");
}

pub fn start(id: u32, at: &Point) {
    log(format_args!("job {id} started at {at}")); // no String is built
}
```

> [!TIP]
> To return a lazily formatted value without a named type, `std::fmt::from_fn` (needs 1.93) wraps a closure as `Display`. On older toolchains, write a small struct with a `Display` impl.

## Avoid

```rust,legacy
// lint: format_push_string, format_collect, useless_format, format_in_format_args, to_string_in_format_args
pub fn append(s: &mut String, n: u32) {
    s.push_str(&format!("{n};")); // temporary String; use write!(s, "{n};")
}

pub fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect() // one String per byte
}

pub fn greeting() -> String {
    format!("hello") // "hello".to_string() or String::from("hello")
}

pub fn nested(x: u32) -> String {
    format!("{}", format!("a{x}")) // format once
}

pub fn show(x: u32) {
    println!("{}", x.to_string()); // println!("{x}")
}
```

- `format!("{}", s)` with `s: &str` or `String`: use `s.to_string()`, or avoid the copy altogether.
- A `format!` just to pass `&str` to a function that only displays or writes it: use `format_args!` or `write!`.

## Verify

- `cargo clippy -- -W clippy::format_push_string -W clippy::format_collect` (both pedantic). `useless_format`, `format_in_format_args`, and `to_string_in_format_args` are on by default.
- A `format!` outside a hot path is fine. Only change loops and hot functions shown by a profile (see `measure-before-optimizing`).
