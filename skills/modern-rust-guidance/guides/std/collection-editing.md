---
name: collection-editing
description: Filter, remove, deduplicate, sort, search, and update Vec, HashMap, BTreeMap, and VecDeque in place instead of rebuilding or cloning them.
rust-version: "1.87"
clippy-lints: [manual_retain, manual_pop_if, map_entry, manual_contains, stable_sort_primitive]
sources:
  - "std docs: Vec, slice, HashMap, BTreeMap, VecDeque (MIT/Apache-2.0)"
  - "Rust release notes 1.61, 1.66, 1.86, 1.87, 1.88, 1.91, 1.93 (MIT/Apache-2.0)"
  - "Clippy lint docs: manual_retain, manual_pop_if, map_entry, manual_contains, stable_sort_primitive (MIT/Apache-2.0)"
---

# Collection Editing

Edit collections where they live. Models still rebuild a `Vec` with `into_iter().filter().collect()`, do `contains_key` then `insert`, and write index loops with `remove(i)`. Std has an in-place method for each, and several arrived after 1.80. For avoiding copies when you pass collections around, see `needless-copies`.

## Filter and remove in place

`retain` (and `retain_mut` since 1.61, when the closure mutates) keeps matches. When you need the removed items, `extract_if` moves them out without cloning. The closure may mutate the element (`&mut T`; maps get `(&K, &mut V)`).

| Type | `extract_if` | Since |
| --- | --- | --- |
| `Vec` | `extract_if(range, pred)` | 1.87 |
| `HashMap`, `HashSet` | `extract_if(pred)` | 1.88 |
| `BTreeMap`, `BTreeSet` | `extract_if(range, pred)` | 1.91 |

If the returned iterator is dropped early, the remaining elements stay in the collection. Always consume it (`collect()` or `for_each(drop)`).

```rust
pub struct Job {
    pub failed: bool,
    pub id: u32,
}

pub fn drop_odd(v: &mut Vec<u32>) {
    v.retain(|x| x % 2 == 0);
}

pub fn take_failed(jobs: &mut Vec<Job>) -> Vec<Job> {
    jobs.extract_if(.., |j| j.failed).collect() // `..` = whole vector
}

pub fn pop_big(stack: &mut Vec<u32>) -> Option<u32> {
    stack.pop_if(|top| *top > 100) // Vec::pop_if (1.86)
}
```

`VecDeque::pop_front_if` and `pop_back_if` are 1.93. `BinaryHeap::pop_if` is nightly-only.

## Borrow two elements mutably with `get_disjoint_mut` (1.86)

It replaces `split_at_mut` arithmetic. Slices return `Err` for out-of-bounds or equal indices. `HashMap::get_disjoint_mut` takes an array of keys, returns `[Option<&mut V>; N]`, and panics on duplicate keys.

```rust
pub fn move_text(slots: &mut [String], from: usize, to: usize) -> Option<()> {
    let [src, dst] = slots.get_disjoint_mut([from, to]).ok()?;
    dst.push_str(src);
    src.clear();
    Some(())
}
```

## Use the entry API for insert-or-update

```rust
use std::collections::HashMap;

pub fn count<'a>(words: impl IntoIterator<Item = &'a str>) -> HashMap<&'a str, usize> {
    let mut counts = HashMap::new();
    for w in words {
        *counts.entry(w).or_default() += 1; // one hash lookup
    }
    counts
}

// With String keys, `entry(name.to_owned())` allocates even on a hit.
// Look up by &str first and allocate only on a miss.
pub fn bump(counts: &mut HashMap<String, usize>, name: &str) {
    if let Some(n) = counts.get_mut(name) {
        *n += 1;
    } else {
        counts.insert(name.to_owned(), 1);
    }
}
```

`or_insert_with(f)` runs `f` only on a miss. `and_modify(f).or_insert(v)` handles both cases.

## Sort, deduplicate, and search

`dedup` removes only *consecutive* duplicates, so sort first. Use `sort_unstable*` when equal elements are indistinguishable, as with primitives or when the key is the whole element. `binary_search` and `partition_point` (1.52) require sorted input. `is_sorted` is 1.82.

```rust
pub fn unique_sorted(v: &mut Vec<u32>) {
    v.sort_unstable();
    v.dedup();
}

pub fn insertion_point(sorted: &[u32], x: u32) -> usize {
    sorted.partition_point(|&y| y < x) // first index where y >= x
}

pub fn has(v: &[u32], x: u32) -> bool {
    v.contains(&x)
}
```

`drain(range)` removes and yields a range with one shift. Use `VecDeque` when you pop from the front (`pop_front`, not `remove(0)`), and `BTreeMap::range(lo..hi)` / `pop_first()` (1.66) for ordered access. Use `swap_remove` when order does not matter. For appending, `extend`, `extend_from_slice`, and `append` beat a `push` loop.

## Avoid

```rust,legacy
// lint: manual_retain, map_entry, manual_contains, stable_sort_primitive, manual_pop_if
use std::collections::HashMap;

pub fn a(v: Vec<u32>) -> Vec<u32> {
    let mut v = v;
    v = v.into_iter().filter(|x| x % 2 == 0).collect(); // retain
    v
}
pub fn b(m: &mut HashMap<String, u32>, k: String) {
    if !m.contains_key(&k) {
        m.insert(k, 1); // two lookups; use entry
    }
}
pub fn c(v: &[u32], x: u32) -> bool {
    v.iter().any(|y| *y == x)
}
pub fn d(v: &mut [u32]) {
    v.sort(); // sort_unstable
}
pub fn e(v: &mut Vec<u32>) {
    if v.last().is_some_and(|x| *x > 3) {
        v.pop().unwrap();
    }
}
```

Also avoid `while i < v.len() { … v.remove(i) … }` (quadratic: use `retain` or `extract_if`) and cloning a collection to mutate it while iterating.

## Older MSRV

- **Below 1.93 / 1.91 / 1.88 / 1.87:** `VecDeque::pop_*_if`, `BTreeMap::extract_if`, `HashMap::extract_if`, `Vec::extract_if` are missing. Use `retain`, or `std::mem::take(v).into_iter().partition(pred)` when you need the removed items.
- **Below 1.86:** no `pop_if` (`if v.last().is_some_and(..) { v.pop(); }`) and no `get_disjoint_mut` (use `split_at_mut`, or `swap`).
- **Below 1.61:** no `retain_mut`.
- `manual_pop_if` only fires when the MSRV is 1.86 or newer.

## Verify

- `cargo clippy -- -W clippy::manual_retain -W clippy::manual_pop_if -W clippy::map_entry -W clippy::manual_contains -W clippy::stable_sort_primitive`. `stable_sort_primitive` is pedantic.
- Test the edges: empty collection, all match, none match, equal indices for `get_disjoint_mut`. Sort `HashMap` output before asserting on it (iteration order is unspecified).
