---
name: allocation-reuse
description: Cut heap allocations in hot code by reserving capacity, reusing buffers, and avoiding repeated clones of Vec and String.
rust-version: "1.70"
clippy-lints: [vec_init_then_push, slow_vector_initialization, assigning_clones]
sources:
  - "Rust Performance Book: Heap Allocations, Standard Library Types (MIT/Apache-2.0)"
  - "Microsoft Pragmatic Rust Guidelines: M-INITIAL-CAPACITY, M-MEM-REUSE, M-SHRINK-TO-FIT, M-BOX-DST (MIT)"
  - "massimodeluisa rust-best-practices: allocation.md (MIT)"
  - "std docs: Vec, String, HashMap, mem::take (MIT/Apache-2.0)"
---

# Allocation Reuse

Each allocation and free has a cost, and growing a `Vec` or `String` one element at a time reallocates and copies. Reserve capacity when you know the size, keep one buffer alive across loop iterations, and move data out instead of cloning it. Do this where a profiler (DHAT, heaptrack, or `malloc` in a CPU profile) shows allocation is hot, not everywhere. See `measure-before-optimizing`.

## Reserve capacity when the size is known

A `Vec` grows with an unspecified strategy (roughly doubling), so pushing N items can reallocate several times. `with_capacity`, `reserve`, and `reserve_exact` replace that with one allocation. `collect()` and `extend` already use the iterator's `size_hint`, so prefer them to a `push` loop. `String`, `HashMap`, and `HashSet` have `with_capacity` and `reserve` too.

```rust
pub fn join_lines(lines: &[String]) -> String {
    // Exact size is cheap to compute, so allocate once
    let mut out = String::with_capacity(lines.iter().map(|l| l.len() + 1).sum());
    for line in lines {
        out.push_str(line);
        out.push('\n');
    }
    out
}
```

For zero-filled buffers write `vec![0; n]`. It can ask the allocator for zeroed memory, which `with_capacity` + `resize` cannot.

## Reuse one buffer across iterations

`clear()` empties a `Vec`, `String`, `HashMap`, or `HashSet` but keeps its capacity. Hoist the buffer out of the loop:

```rust
pub fn count_tokens(lines: &[&str]) -> usize {
    let mut tokens: Vec<&str> = Vec::new(); // allocated once, grows to the longest line
    let mut total = 0;
    for line in lines {
        tokens.clear(); // keeps capacity
        tokens.extend(line.split_whitespace());
        total += tokens.len();
    }
    total
}
```

For hot functions that return a fresh `Vec`, offer a variant that writes into the caller's buffer, and keep the returning version as a thin wrapper:

```rust
pub fn encode_into(values: &[u32], out: &mut Vec<u8>) {
    out.reserve(values.len() * 4);
    for v in values {
        out.extend_from_slice(&v.to_le_bytes()); // appends: the caller clears
    }
}

pub fn encode(values: &[u32]) -> Vec<u8> {
    let mut out = Vec::new();
    encode_into(values, &mut out);
    out
}
```

Reading lines into a reused `String` is the same pattern; see `buffered-io`. A reused buffer keeps its peak capacity. If one huge input inflates it, call `shrink_to(n)` afterwards.

## Overwrite with `clone_from`, move out with `mem::take`

`a.clone_from(&b)` is `a = b.clone()` that may reuse `a`'s existing allocation (`Vec` and `String` do). To hand over a collection and leave an empty one behind, use `std::mem::take` instead of clone-then-clear:

```rust
pub struct Batcher {
    pending: Vec<String>,
    last: String,
}

impl Batcher {
    pub fn flush(&mut self) -> Vec<String> {
        std::mem::take(&mut self.pending) // moves the Vec out: no copy, no clone
    }

    pub fn copy_last_from(&mut self, other: &Batcher) {
        self.last.clone_from(&other.last); // reuses self.last's buffer when it is big enough
    }
}
```

## Keep long-lived data compact

For a collection that is built once and kept for a long time, `shrink_to_fit()` after construction frees spare capacity. It may reallocate and copy, so call it once, never in a loop, and not on short-lived or still-growing data. For immutable data, `Vec<T>::into_boxed_slice()` and `String::into_boxed_str()` drop the capacity field too (`type-size-and-layout`).

## Avoid

```rust,legacy
// lint: slow_vector_initialization, assigning_clones
pub fn zeroed(n: usize) -> Vec<u8> {
    let mut v = Vec::with_capacity(n);
    v.resize(n, 0); // use vec![0; n]
    v
}

pub struct Config {
    pub name: String,
}

pub fn copy_name(dst: &mut Config, src: &Config) {
    dst.name = src.name.clone(); // allocates a new String, then frees the old one
}
```

```rust,legacy
// lint: vec_init_then_push
pub fn pair() -> Vec<i32> {
    let mut v = Vec::new(); // grows as it goes; write vec![1, 2]
    v.push(1);
    v.push(2);
    v
}
```

- `.clone()` on a `Vec` or `String` inside a loop only to read it: borrow it. See `needless-copies`.
- `Vec::new()` plus a `push` loop over an iterator of known length: use `collect()` or `with_capacity`.
- `shrink_to_fit()` in a hot loop.
- Pre-allocating giant capacities "just in case". Reserve what you will use.

> [!NOTE]
> `SmallVec`, `ArrayVec`, and arena crates avoid heap allocation for short or short-lived data. They are out of scope for this guide.

## Verify

- `cargo clippy -- -W clippy::vec_init_then_push -W clippy::slow_vector_initialization -W clippy::assigning_clones` (`assigning_clones` is pedantic).
- Confirm with a profiler or allocation counter that the allocation count dropped, and keep the before/after in the change description.
- In review, check that every reused buffer is cleared at the top of each iteration. Appending APIs such as `read_line` and `extend` do not clear for you.
