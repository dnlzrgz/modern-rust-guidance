---
name: data-structure-choice
description: Pick the right collection for lookups, queues, deduplication, sorting, and grids instead of scanning Vec, removing from the front, or nesting Vec<Vec<T>>.
rust-version: "1.70"
clippy-lints: [map_entry, set_contains_or_insert, stable_sort_primitive, linkedlist]
sources:
  - "Rust Performance Book: Standard Library Types, Hashing, Linting (MIT/Apache-2.0)"
  - "std docs: collections (HashMap, HashSet, VecDeque, LinkedList, BTreeMap), slice::sort_unstable (MIT/Apache-2.0)"
---

# Data Structure Choice

A wrong data structure costs far more than any micro-tuning: a linear scan inside a loop turns O(n) into O(n²), and no allocator or build flag fixes that. Check the complexity of what a hot loop does before tuning it, then confirm with a profile (see `measure-before-optimizing`). For editing collections in place, see `collection-editing`.

## Replace repeated scans with a set or map

`Vec::contains`, `iter().position(..)`, and `iter().find(..)` are O(n) per call. Called once per item they are quadratic. Build a `HashSet` or `HashMap` once, then look up. Use a `BTreeMap` or `BTreeSet` when you need sorted order or ranges. For a few dozen elements a slice scan is fine and often simpler, so switch when the input can grow.

```rust
use std::collections::HashSet;

pub fn count_known(items: &[u32], known: &[u32]) -> usize {
    let known: HashSet<u32> = known.iter().copied().collect(); // built once
    items.iter().filter(|x| known.contains(x)).count()
}

pub fn first_seen(seen: &mut HashSet<u32>, x: u32) -> bool {
    seen.insert(x) // returns true if x was new: one hash, not contains + insert
}
```

For read-only data, a sorted `Vec` with `binary_search` also works and keeps the data contiguous.

## Use `VecDeque` for queues, `swap_remove` for unordered removal

`Vec::remove(0)` and `insert(0, ..)` shift every element. A queue needs `VecDeque` (`push_back`, `pop_front`). If order does not matter, `swap_remove(i)` is O(1). To drop many items, one `retain` pass beats a remove-in-a-loop, which is quadratic and shifts the indices you are still using.

```rust
use std::collections::VecDeque;

pub fn drain_sum(queue: &mut VecDeque<u32>) -> u64 {
    let mut sum = 0;
    while let Some(x) = queue.pop_front() {
        sum += u64::from(x);
    }
    sum
}

pub fn take_first_even(v: &mut Vec<u32>) -> Option<u32> {
    let i = v.iter().position(|x| x % 2 == 0)?;
    Some(v.swap_remove(i)) // moves the last element into the hole: order changes
}
```

Avoid `LinkedList`. Each node is a separate allocation, traversal chases pointers, and std's own docs say `Vec` or `VecDeque` is generally faster and more cache-friendly.

## Hash maps: one lookup, sized up front

- Use the entry API (`*map.entry(k).or_insert(0) += 1`), not `contains_key` followed by `insert` or `get_mut`: that hashes twice. If building the key allocates, see `needless-copies`.
- `HashMap::with_capacity(n)` and `reserve(n)` avoid rehashing while it grows.
- Iteration order is random per map. Use `BTreeMap` when output must be deterministic.
- The default hasher (SipHash 1-3) is slower than simple hashers on purpose: it resists HashDoS from attacker-chosen keys. Keep it for anything that hashes untrusted input.

> [!NOTE]
> Faster hashers (`foldhash`, `rustc-hash`) are crates and out of scope for this guide. They are only appropriate for trusted keys, and only after a profile shows hashing is hot. Ask before adding a dependency.

## Sort and select cheaply

`sort_unstable` (and `sort_unstable_by_key`) is the default when equal elements are indistinguishable, such as primitives, or you do not need their original order. It does not allocate. Use stable `sort` only when ties must keep their input order. For the smallest or largest `k`, use `select_nth_unstable` or a `BinaryHeap` instead of sorting everything, and `min_by_key` / `max_by_key` for one.

## Do not index strings by character position

`s.chars().nth(i)` walks from the start every time, so a loop over `i` is quadratic. Iterate once, zip two `chars()` iterators, or use `char_indices()`. For ASCII data, `s.as_bytes()` indexes in O(1). If you need random access to characters, collect into a `Vec<char>` once.

## Store grids in one flat `Vec`

`Vec<Vec<T>>` costs one allocation per row and scatters rows in memory. For rectangular data, use one `Vec<T>` and compute `y * width + x`. Check `x` against the width yourself, or an out-of-range `x` silently reads the next row:

```rust
pub struct Grid<T> {
    width: usize,
    cells: Vec<T>,
}

impl<T: Clone> Grid<T> {
    pub fn new(width: usize, height: usize, fill: T) -> Self {
        Grid { width, cells: vec![fill; width * height] }
    }

    pub fn get(&self, x: usize, y: usize) -> Option<&T> {
        if x >= self.width {
            return None;
        }
        self.cells.get(y * self.width + x)
    }
}
```

## Avoid

```rust,legacy
// lint: map_entry, set_contains_or_insert, stable_sort_primitive, linkedlist
use std::collections::{HashMap, HashSet, LinkedList};

pub fn tally(counts: &mut HashMap<String, u32>, key: String) {
    if !counts.contains_key(&key) {
        counts.insert(key, 1); // two lookups: use the entry API
    }
}

pub fn first_seen(seen: &mut HashSet<u32>, x: u32) -> bool {
    if !seen.contains(&x) {
        seen.insert(x); // use `seen.insert(x)` and read its bool
        return true;
    }
    false
}

pub fn sorted(mut v: Vec<u32>) -> Vec<u32> {
    v.sort(); // sort_unstable: equal u32s are indistinguishable
    v
}

fn queue() -> LinkedList<u32> {
    LinkedList::new() // VecDeque
}

pub fn overlap(a: &[u32], b: &[u32]) -> usize {
    a.iter().filter(|x| b.contains(x)).count() // O(a * b): HashSet the smaller side
}

pub fn same_chars(a: &str, b: &str) -> usize {
    (0..a.len()).filter(|&i| a.chars().nth(i) == b.chars().nth(i)).count() // O(n^2)
}
```

## Verify

- `cargo clippy` reports `map_entry` by default. `stable_sort_primitive` and `linkedlist` are pedantic, and `set_contains_or_insert` is nursery, so enable them with `-W clippy::<lint>`.
- No lint catches `contains`, `position`, `remove(0)`, or `chars().nth(i)` inside a loop. Look for them in review.
- Benchmark at the real input size and at ten times that size. Time that grows about 100x points to a quadratic step.
