---
name: collection-editing
description: Filter, remove, deduplicate, sort, search, and update Vec, HashMap, BTreeMap, and VecDeque in place instead of rebuilding or cloning them.
rust-version: "1.87"
clippy-lints: [manual_retain, manual_pop_if, map_entry, manual_contains, stable_sort_primitive]
sources:
  - "std docs: Vec, slice, HashMap, BTreeMap, VecDeque (MIT/Apache-2.0)"
  - "Rust release notes 1.77, 1.82, 1.86, 1.87, 1.88, 1.91, 1.93 (MIT/Apache-2.0)"
  - "Clippy lint docs: manual_retain, manual_pop_if, map_entry, manual_contains, stable_sort_primitive (MIT/Apache-2.0)"
---

# Collection Editing

Edit collections where they live. Models still rebuild a `Vec` with `into_iter().filter().collect()`, do `contains_key` then `insert`, and write index loops with `remove(i)`. Std has an in-place method for each, and several arrived after 1.80. For avoiding copies when you pass collections around, see `needless-copies`.

## Filter and remove in place

Reach for `retain` (`retain_mut` when the closure mutates) first. Use `extract_if` only when you need the removed items: it moves them out without cloning. The closure may mutate the element (`&mut T`; maps get `(&K, &mut V)`; sets get `&T`).

| Type | `extract_if` | Since |
| --- | --- | --- |
| `Vec` | `extract_if(range, pred)` | 1.87 |
| `HashMap`, `HashSet` | `extract_if(pred)` | needs 1.88 |
| `BTreeMap`, `BTreeSet` | `extract_if(range, pred)` | needs 1.91 |

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
```

`Vec::pop_if(pred)` (1.86) pops the last element only if the predicate holds. `VecDeque::pop_front_if` and `pop_back_if` (needs 1.93) are the deque equivalents. `BinaryHeap::pop_if` is nightly-only.

To move a field or element out of a `&mut` borrow, use `std::mem::take(&mut self.buf)` (or `mem::replace`), not `self.buf.clone()`.

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
```

With `String` keys, `entry(name.to_owned())` allocates even on a hit: call `get_mut(name)` first and insert only on a miss. `or_insert_with(f)` runs `f` only on a miss. `and_modify(f).or_insert(v)` handles both cases.

## Sort and group

`dedup` removes only *consecutive* duplicates, so sort first. `sort_unstable*` is fine whenever equal elements are indistinguishable. Floats are not `Ord`: sort with `total_cmp`, not `partial_cmp().unwrap()`. For a key that allocates, `sort_by_cached_key` computes it once per element instead of once per comparison. `chunk_by` (1.77) groups equal neighbors of a sorted slice without a manual loop.

```rust
use std::cmp::Reverse;

pub fn unique_sorted(v: &mut Vec<u32>) {
    v.sort_unstable();
    v.dedup();
}

pub fn sort_scores(v: &mut [f64]) {
    v.sort_unstable_by(f64::total_cmp); // NaN-safe, no unwrap
}

pub fn newest_first(v: &mut [(u32, String)]) {
    v.sort_by_key(|(ts, _)| Reverse(*ts)); // descending without a custom closure
}

pub fn by_lowercase(v: &mut [String]) {
    v.sort_by_cached_key(|s| s.to_lowercase()); // one allocation per element
}

pub fn run_lengths(sorted: &[u32]) -> Vec<(u32, usize)> {
    sorted.chunk_by(|a, b| a == b).map(|g| (g[0], g.len())).collect()
}
```

`drain(range)` removes and yields a range with one shift. Pop from the front of a `VecDeque` (`pop_front`), not `remove(0)`. Use `swap_remove` when order does not matter, and `BTreeMap::pop_first()` for ordered access. For appending, `extend`, `extend_from_slice`, and `append` beat a `push` loop.

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

- **Below 1.93 / 1.91 / 1.88 / 1.87:** `VecDeque::pop_*_if`, `BTreeMap::extract_if`, `HashMap::extract_if`, `Vec::extract_if` are missing. Use `retain`, or when you need the removed items: `let (taken, kept): (Vec<_>, Vec<_>) = std::mem::take(v).into_iter().partition(pred); *v = kept;`.
- **Below 1.86:** no `pop_if` (`if v.last().is_some_and(..) { v.pop(); }`) and no `get_disjoint_mut` (use `split_at_mut`, or `swap`).
- `manual_pop_if` only fires when the MSRV is 1.86 or newer.

## Verify

- `cargo clippy` flags `manual_retain`, `manual_pop_if`, `map_entry`, and `manual_contains` by default. `stable_sort_primitive` is pedantic: run it with `-W clippy::stable_sort_primitive`.
- Test the edges: empty collection, all match, none match, equal indices for `get_disjoint_mut`. Sort `HashMap` output before asserting on it (iteration order is unspecified).
