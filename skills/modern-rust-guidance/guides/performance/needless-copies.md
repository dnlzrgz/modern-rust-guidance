---
name: needless-copies
description: Avoid needless clone, to_owned, to_string, and collect calls by borrowing, returning iterators, and using Cow.
rust-version: "1.70"
clippy-lints: [redundant_clone, needless_collect, unnecessary_to_owned, implicit_clone, iter_overeager_cloned, cmp_owned, ptr_arg, needless_pass_by_value]
sources:
  - "Rust Performance Book: Heap Allocations (clone, to_owned, Cow), Iterators (collect and extend), Linting (MIT/Apache-2.0)"
  - "Microsoft Pragmatic Rust Guidelines: M-HOTPATH, M-AVOID-INDIRECTION (MIT)"
  - "massimodeluisa rust-best-practices: performance.md (MIT)"
  - "std docs: borrow::Cow, iter::Iterator (MIT/Apache-2.0)"
---

# Needless Copies

`clone()`, `to_owned()`, `to_string()`, and `collect()` usually allocate and copy. They are fine when you need ownership, and they are the first things models reach for to make the borrow checker quiet. Borrow instead, return iterators instead of collections, and clone only what you must keep. A clone you cannot remove without hurting clarity is not a bug: find the hot ones with a profiler first (`measure-before-optimizing`). For buffers you keep and refill, see `allocation-reuse`.

## Take borrowed parameters

Accept `&str`, `&[T]`, and `&Path` so callers need not own a `String`, `Vec`, or `PathBuf`. Take ownership only if the function stores or consumes the value.

```rust
pub fn total(xs: &[u32]) -> u32 {
    xs.iter().sum() // callers can pass a Vec, an array, or a sub-slice
}
```

## Fix ownership instead of cloning to appease the borrow checker

Borrow the field, not all of `self`. Disjoint fields can be borrowed at once:

```rust
pub struct Inventory {
    items: Vec<String>,
    total: usize,
}

impl Inventory {
    pub fn recount(&mut self) {
        // `self.items` and `self.total` are different fields: no clone needed
        for item in &self.items {
            self.total += item.len();
        }
    }
}
```

When a loop must call `&mut self` methods, `std::mem::take(&mut self.items)` and put it back afterwards, collect indices, or split the type into two. `let items = self.items.clone();` just to loop is the habit to avoid.

## Do not collect to iterate again

Chain adapters to the final consumer. When a function's caller may only iterate, return `impl Iterator` and let them collect if they need to. In edition 2024, `impl Trait` return types capture the elided lifetime automatically. In edition 2021 add `+ '_`.

```rust
pub fn long_words(text: &str, min: usize) -> impl Iterator<Item = &str> {
    text.split_whitespace().filter(move |w| w.len() >= min)
}

pub fn append_lens(dst: &mut Vec<usize>, words: &[&str]) {
    dst.extend(words.iter().map(|w| w.len())); // not collect() then append()
}
```

Filter before you clone: `.iter().filter(..).cloned()` copies only what passes, while `.cloned().filter(..)` copies everything first.

## Use `Cow` for "usually borrowed, sometimes owned"

```rust
use std::borrow::Cow;

pub fn normalize(path: &str) -> Cow<'_, str> {
    if path.contains('\\') {
        Cow::Owned(path.replace('\\', "/")) // allocate only when we change something
    } else {
        Cow::Borrowed(path)
    }
}

pub fn messages() -> Vec<Cow<'static, str>> {
    vec![
        Cow::Borrowed("something went wrong"), // literals need no allocation
        Cow::Owned(format!("failed on line {}", 100)),
    ]
}
```

`Cow::to_mut()` clones only when you first mutate. `Rc::make_mut` and `Arc::make_mut` do the same for shared values.

## Know what is cheap

- `Rc::clone` and `Arc::clone` only bump a reference count, with no deep copy. An `Arc` clone is still an atomic operation, so pass `&Arc<T>` or `&T` into hot inner functions instead of cloning per call.
- Storing `&'a str` instead of `String` in a struct avoids copies but adds lifetimes. Do it only where a profile shows the copies matter.
- Current std specializes `to_string()` for `str`, `&str`, `&&str`, `String`, and `Cow<str>`, so `s.to_string()` on a string type does not go through the formatting machinery. Clippy's `inefficient_to_string` only matters when the MSRV is below 1.82.

## Avoid

```rust,legacy
// lint: ptr_arg, needless_pass_by_value, needless_collect, cmp_owned, unnecessary_to_owned
pub fn total(xs: &Vec<u32>) -> u32 {
    xs.iter().sum() // &Vec<T> forces callers to own a Vec; take &[T]
}

pub fn shout_len(s: String) -> usize {
    s.len() // owns it only to read; callers must clone
}

pub fn total_len(words: &[&str]) -> usize {
    let lens: Vec<usize> = words.iter().map(|w| w.len()).collect();
    lens.into_iter().sum() // allocates a Vec to sum it; sum() straight off the map
}

pub fn is_foo(x: &str) -> bool {
    x == "foo".to_string() // allocates to compare; write x == "foo"
}

fn takes(_: &str) {}

pub fn call(x: &str) {
    takes(&x.to_string()); // takes(x)
}
```

```rust,legacy
// lint: iter_overeager_cloned, redundant_clone, implicit_clone
pub fn long_names(names: &[String]) -> Vec<String> {
    names.iter().cloned().filter(|s| s.len() > 2).collect() // clones every name first
}

pub fn consume(s: String) -> usize {
    let t = s.clone(); // s is never used again
    t.len()
}

pub struct User {
    pub name: String,
}

impl User {
    pub fn name(&self) -> String {
        self.name.to_string() // a String already: say .clone()
    }
}
```

## Verify

- `cargo clippy -- -W clippy::needless_collect -W clippy::unnecessary_to_owned -W clippy::iter_overeager_cloned -W clippy::cmp_owned -W clippy::ptr_arg`. `redundant_clone` (nursery), `implicit_clone`, and `needless_pass_by_value` (pedantic) are opt-in and can be noisy. Check each finding by hand.
- Keep a clone that makes ownership sound or the code readable. For a claimed win, show allocation counts or timings before and after.
