---
name: option-result-combinators
description: Test, convert, and default Option and Result values with the current combinators instead of map_or(false, …), match, or eager unwrap_or calls.
rust-version: "1.82"
clippy-lints: [unnecessary_map_or, manual_is_variant_and, manual_inspect, option_map_or_none, manual_ok_or, unnecessary_lazy_evaluations, or_fun_call, unwrap_or_default]
sources:
  - "std docs: std::option::Option, std::result::Result (MIT/Apache-2.0)"
  - "Rust release notes 1.70, 1.75, 1.76, 1.80, 1.82 (MIT/Apache-2.0)"
  - "Clippy lint docs: unnecessary_map_or, manual_is_variant_and, manual_inspect, or_fun_call (MIT/Apache-2.0)"
---

# Option and Result Combinators

Models still write `map_or(false, …)`, `map(|x| { log(&x); x })`, and `unwrap_or(format!(…))`. Newer std has a named method for each. Pick the combinator that states the intent, and choose between eager (`unwrap_or`) and lazy (`unwrap_or_else`) forms deliberately. For early exits with `?` and `let … else`, see `early-return-patterns`.

## Test the contents with `is_some_and`, `is_ok_and`, `is_none_or`

`is_some_and`, `is_ok_and`, `is_err_and` are stable since 1.70, and `Option::is_none_or` since 1.82. They take `self` by value, so on an owned `Option<String>` call `as_ref()` or `as_deref()` first or you move the value.

```rust
pub fn is_admin(role: Option<&str>) -> bool {
    role.is_some_and(|r| r == "admin")
}

// Absent means "no restriction": is_none_or is true for None.
pub fn within_limit(limit: Option<u32>, n: u32) -> bool {
    limit.is_none_or(|max| n <= max)
}

pub fn has_prefix(name: &Option<String>) -> bool {
    // as_deref borrows; `name.is_some_and(..)` would move the String out.
    name.as_deref().is_some_and(|s| s.starts_with("tmp_"))
}

pub fn parsed_ok_and_positive(s: &str) -> bool {
    s.parse::<i32>().is_ok_and(|n| n > 0)
}
```

## Observe without consuming: `inspect`, `inspect_err` (1.76)

They run a closure on `&T` / `&E` and pass the value through. Use them for logging and metrics instead of a `map` that returns its argument.

```rust
pub fn load(path: &str) -> std::io::Result<String> {
    std::fs::read_to_string(path)
        .inspect(|text| eprintln!("read {} bytes", text.len()))
        .inspect_err(|e| eprintln!("read {path} failed: {e}"))
}
```

## Pick eager or lazy deliberately

The `_or` forms (`unwrap_or`, `ok_or`, `or`) evaluate their argument **always**. If it calls a function, allocates, or formats, use the `_else` form. If the fallback is a plain value, a variable, or a `const`, stay eager; a closure there only adds noise. For `Default::default()` use `unwrap_or_default()`.

```rust
pub fn label(name: Option<&str>, id: u32) -> String {
    // format! allocates only in the None case.
    name.map(str::to_owned).unwrap_or_else(|| format!("item-{id}"))
}

pub fn require(v: Option<u32>, key: &str) -> Result<u32, String> {
    v.ok_or_else(|| format!("missing key {key}"))
}

pub fn words(text: Option<String>) -> String {
    text.unwrap_or_default() // not unwrap_or(String::new())
}
```

## Reshape instead of matching

```rust
use std::num::ParseIntError;

// Option<Result<u16, E>> -> Result<Option<u16>, E>, so `?` can then propagate the error.
pub fn port(raw: Option<&str>) -> Result<Option<u16>, ParseIntError> {
    raw.map(str::parse::<u16>).transpose()
}

// Option::take_if (1.80): take the value out only when the predicate holds.
// The predicate receives `&mut T`.
pub fn expire(slot: &mut Option<u64>, now: u64) -> Option<u64> {
    slot.take_if(|created| now - *created > 60)
}

// Option::as_slice (1.75): zero-or-one element slice, no allocation.
pub fn run(args: &[&str]) -> usize {
    args.len()
}
pub fn run_optional(extra: Option<&str>) -> usize {
    run(extra.as_slice())
}
```

`Option::get_or_insert_default` is 1.83, `Result::flatten` is 1.89. Both are fine when the MSRV allows.

## Avoid

```rust,legacy
// lint: unnecessary_map_or, manual_is_variant_and
pub fn a(o: Option<u32>) -> bool {
    o.map_or(false, |n| n > 3) // is_some_and
}
pub fn b(o: Option<u32>) -> bool {
    o.map_or(true, |n| n > 3) // is_none_or (1.82)
}
pub fn c(o: Option<u32>) -> bool {
    o.map(|n| n > 3).unwrap_or_default() // is_some_and
}
```

```rust,legacy
// lint: manual_inspect, manual_ok_or, or_fun_call, unnecessary_lazy_evaluations
pub fn d(o: Option<u32>) -> Option<u32> {
    o.map(|n| {
        println!("{n}");
        n
    })
}
pub fn e(o: Option<u32>) -> Result<u32, &'static str> {
    o.map_or(Err("missing"), Ok) // ok_or
}
pub fn f(o: Option<u32>, id: u32) -> Result<u32, String> {
    o.ok_or(format!("missing {id}")) // allocates even when Some
}
pub fn g(o: Option<u32>) -> u32 {
    o.unwrap_or_else(|| 0) // plain value: unwrap_or(0)
}
```

Also avoid `.ok()` to silence an error you should report, `.unwrap_or_default()` on a `Result` whose failure matters, and `if x.is_some() { x.unwrap() … }`.

## Older MSRV

- **Below 1.82:** write `map_or(true, f)` for `is_none_or`. Clippy's `unnecessary_map_or` respects the MSRV and will not suggest it.
- **Below 1.80:** replace `take_if` with `if slot.as_mut().is_some_and(pred) { slot.take() } else { None }`.
- **Below 1.76:** no `inspect`/`inspect_err`; use `map`/`map_err` with a closure that returns its argument.
- **Below 1.75:** no `Option::as_slice`; use `std::slice::from_ref` inside a `match`.
- `is_some_and` and friends need 1.70, which is the floor for this skill.

## Verify

- `cargo clippy -- -W clippy::unnecessary_map_or -W clippy::manual_is_variant_and -W clippy::manual_inspect -W clippy::option_map_or_none -W clippy::manual_ok_or -W clippy::unnecessary_lazy_evaluations -W clippy::or_fun_call -W clippy::unwrap_or_default`.
- `or_fun_call` is in the `nursery` group and can be noisy. Treat each hit as a question: does the argument allocate or do real work?
- After switching to `is_some_and` on an owned value, check for "use of moved value" errors and add `as_ref()` / `as_deref()`.
