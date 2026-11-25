---
name: needless-copies
description: Avoid needless clone, to_owned, to_string, and collect calls by borrowing, returning iterators, and using Cow or Arc<str>.
rust-version: "1.70"
clippy-lints: [needless_collect, unnecessary_to_owned, iter_overeager_cloned, cmp_owned, manual_ignore_case_cmp, ptr_arg]
sources:
  - "Rust Performance Book: Heap Allocations (clone, to_owned, Cow), Iterators (collect and extend), Linting (MIT/Apache-2.0)"
  - "Microsoft Pragmatic Rust Guidelines: M-HOTPATH, M-AVOID-INDIRECTION (MIT)"
  - "massimodeluisa rust-best-practices: performance.md (MIT)"
  - "std docs: borrow::Cow, sync::Arc, str::eq_ignore_ascii_case, iter::Iterator (MIT/Apache-2.0)"
---

# Needless Copies

`clone()`, `to_owned()`, `to_string()`, and `collect()` usually allocate and copy. They are fine when you need ownership, and they are the first things models reach for to make the borrow checker quiet. Borrow instead, return iterators instead of collections, and clone only what you must keep. A clone you cannot remove without hurting clarity is not a bug: find the hot ones with a profiler first (see `measure-before-optimizing`). For buffers you keep and refill, see `allocation-reuse`.

## Fix ownership instead of cloning to appease the borrow checker

The common case: iterating `self.items` while calling a `&mut self` method fails with E0502, and the "fix" is `self.items.clone()`. Move the collection out with `mem::take` and put it back, so nothing is copied:

```rust
pub struct Inventory {
    items: Vec<String>,
    total: usize,
}

impl Inventory {
    fn add(&mut self, n: usize) {
        self.total += n;
    }

    pub fn recount(&mut self) {
        let items = std::mem::take(&mut self.items); // moves the Vec out, leaves an empty one
        for item in &items {
            self.add(item.len());
        }
        self.items = items; // put it back
    }
}
```

While `items` is taken, `self.items` is empty, so the called method must not read it. Other fixes: iterate by index and copy out the small value you need each time, or split the type so the borrowed part and the mutated part are separate fields (disjoint fields can be borrowed at once).

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

## Look up before you allocate a key

`map.entry(word.to_string())` allocates a `String` on every call, even when the key is already there. When most words repeat, look up with the borrowed `&str` first and allocate only on a miss:

```rust
use std::collections::HashMap;

pub fn count(counts: &mut HashMap<String, u32>, word: &str) {
    if let Some(n) = counts.get_mut(word) {
        *n += 1; // hit: no allocation
    } else {
        counts.insert(word.to_string(), 1); // miss: allocate once
    }
}
```

## Compare without allocating

`a.to_ascii_lowercase() == b` and `a.to_lowercase() == b` allocate a `String` just to compare. For ASCII text use `a.eq_ignore_ascii_case(b)`, which suits file extensions and header names. Keep `to_lowercase` only when you need Unicode-aware case mapping.

```rust
pub fn is_json(ext: &str) -> bool {
    ext.eq_ignore_ascii_case("json")
}
```

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
```

`Cow::to_mut()` clones only when you first mutate. `Rc::make_mut` and `Arc::make_mut` do the same for shared values.

## Know what is cheap

- `Rc::clone` and `Arc::clone` only bump a reference count, with no deep copy. An `Arc` clone is still an atomic operation, so pass `&Arc<T>` or `&T` into hot inner functions instead of cloning per call.
- For an immutable string that many owners share, `Arc<str>` (`Rc<str>` on one thread) clones in O(1) instead of copying the text. Build it once with `Arc::from(s)`. For a single owner, `Box<str>` drops the capacity field.
- Storing `&'a str` instead of `String` in a struct avoids copies but adds lifetimes. Do it only where a profile shows the copies matter.

## Avoid

```rust,legacy
// lint: ptr_arg, needless_collect, cmp_owned, unnecessary_to_owned, manual_ignore_case_cmp
pub fn total(xs: &Vec<u32>) -> u32 {
    xs.iter().sum() // &Vec<T> forces callers to own a Vec; take &[T]
}

pub fn total_len(words: &[&str]) -> usize {
    let lens: Vec<usize> = words.iter().map(|w| w.len()).collect();
    lens.into_iter().sum() // allocates a Vec to sum it; sum() straight off the map
}

pub fn is_foo(x: &str) -> bool {
    x == "foo".to_string() // allocates to compare; write x == "foo"
}

pub fn same_name(a: &str, b: &str) -> bool {
    a.to_ascii_lowercase() == b.to_ascii_lowercase() // two Strings; a.eq_ignore_ascii_case(b)
}

fn takes(_: &str) {}

pub fn call(x: &str) {
    takes(&x.to_string()); // takes(x)
}
```

```rust,legacy
// lint: iter_overeager_cloned
pub fn long_names(names: &[String]) -> Vec<String> {
    names.iter().cloned().filter(|s| s.len() > 2).collect() // clones every name first
}
```

## Verify

- `cargo clippy`: `unnecessary_to_owned`, `iter_overeager_cloned`, `cmp_owned`, `manual_ignore_case_cmp`, and `ptr_arg` are on by default. `needless_collect` is nursery, so enable it with `-W clippy::needless_collect`.
- Opt-in lints worth a hand-run, with noise: `redundant_clone` (nursery), `implicit_clone` and `needless_pass_by_value` (pedantic). Check each finding by hand.
- Keep a clone that makes ownership sound or the code readable. For a claimed win, show allocation counts or timings before and after.
