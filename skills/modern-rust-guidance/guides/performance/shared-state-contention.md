---
name: shared-state-contention
description: Reduce lock contention and slow shared state across threads by shortening critical sections, accumulating per thread, using atomics, and passing ownership instead of sharing.
rust-version: "1.70"
clippy-lints: [await_holding_lock, mutex_atomic]
sources:
  - "Rust Performance Book: Parallelism (MIT/Apache-2.0)"
  - "std docs: sync::Mutex, sync::RwLock, sync::atomic, sync::mpsc, thread::scope (MIT/Apache-2.0)"
---

# Shared State Contention

A lock is slow only when threads wait for it. Threads that share one `Arc<Mutex<_>>` and lock it per item run one at a time, plus the cost of the locking. Fix it by holding locks briefly, giving each thread its own state and merging once, and passing ownership instead of sharing. Add threads only to CPU-bound work that a profile shows is worth splitting (see `performance`). For testing concurrent code, see `unsafe-and-concurrency-testing`.

## Hold a lock only for the access

Never hold a guard across slow work: I/O, sleeping, a big computation, or a call into code you don't control. Take what you need, let the guard drop, then work. In sync code, end the guard's life with the end of the scope, a `{ .. }` block, or `drop(guard)`. Never hold a `std::sync` guard across `.await`: it blocks the executor thread and can deadlock. There, only a `{ .. }` block works: after `drop(guard)` the compiler still treats the guard as live across the `.await`, so the future is not `Send` and `await_holding_lock` still fires.

```rust
use std::sync::Mutex;

pub struct Cache {
    entries: Mutex<Vec<(u32, String)>>,
}

impl Cache {
    fn lookup(&self, key: u32) -> Option<String> {
        let entries = self.entries.lock().expect("cache lock poisoned");
        entries.iter().find(|(k, _)| *k == key).map(|(_, v)| v.clone())
    } // guard dropped here

    pub fn get_or_compute(&self, key: u32, compute: impl FnOnce() -> String) -> String {
        if let Some(v) = self.lookup(key) {
            return v;
        }
        let value = compute(); // slow work with no lock held
        // Another thread may have stored it meanwhile (fine if `compute` is idempotent)
        let mut entries = self.entries.lock().expect("cache lock poisoned");
        if let Some((_, v)) = entries.iter().find(|(k, _)| *k == key) {
            return v.clone();
        }
        entries.push((key, value.clone()));
        value
    }
}
```

```rust
use std::sync::Mutex;

pub async fn refresh(cache: &Mutex<Vec<u32>>) -> usize {
    let len = {
        let mut guard = cache.lock().expect("cache lock poisoned");
        guard.clear();
        guard.len()
    }; // guard is out of scope before the await, so the future stays `Send`
    std::future::ready(()).await;
    len
}
```

## Accumulate per thread, merge once

Have each thread compute a private result and combine them after the join. `thread::scope` lets threads borrow local data (no `Arc`, no `'static`), joins them all before returning, and re-panics if one panicked.

```rust
use std::thread;

pub fn count_matches(data: &[u32], target: u32) -> usize {
    let threads = thread::available_parallelism().map_or(1, |n| n.get());
    let chunk_len = data.len() / threads + 1; // at most `threads` chunks
    thread::scope(|s| {
        let handles: Vec<_> = data
            .chunks(chunk_len)
            .map(|chunk| s.spawn(move || chunk.iter().filter(|&&x| x == target).count()))
            .collect();
        handles.into_iter().map(|h| h.join().expect("worker panicked")).sum()
    })
}
```

The same shape works for a `HashMap` per thread merged at the end. Do not spawn threads for tiny inputs: thread start-up costs more than the work.

## Use atomics for counters and flags

A `Mutex<usize>` or `Mutex<bool>` is an atomic with extra steps. Use `fetch_add` for a counter, with `Ordering::Relaxed` when it only counts and does not publish other data to other threads. If another thread reads data written before the flag, use `Release` on the store and `Acquire` on the load, or use a lock. In existing code, do not weaken an ordering (`SeqCst` to `Relaxed`) or swap a lock for an atomic without a loom or Miri test: the ordering may be what makes a flag or counter a completion signal. Many threads incrementing one atomic still contend on its cache line, so for a hot counter keep one per thread and sum them.

```rust
use std::sync::atomic::{AtomicUsize, Ordering};

pub struct Stats {
    requests: AtomicUsize,
}

impl Stats {
    pub fn record(&self) {
        self.requests.fetch_add(1, Ordering::Relaxed); // read it with load(Ordering::Relaxed)
    }
}
```

## Do not assume `RwLock` is faster

`RwLock` bookkeeping costs more than a `Mutex`, so it pays off only when reads are many and long, and writes are rare. Upgrading a read guard to a write guard in place deadlocks, and std documents the reader/writer priority as OS-dependent. Start with `Mutex` and switch only after measuring. For read-mostly data, share an `Arc` snapshot and hold the lock only to clone or swap it:

```rust
use std::sync::{Arc, Mutex};

pub struct Shared<T> {
    current: Mutex<Arc<T>>,
}

impl<T> Shared<T> {
    pub fn snapshot(&self) -> Arc<T> {
        Arc::clone(&self.current.lock().expect("lock poisoned")) // readers use it lock-free
    }

    pub fn replace(&self, value: T) {
        *self.current.lock().expect("lock poisoned") = Arc::new(value);
    }
}
```

## Prefer ownership transfer to sharing

- Return results from threads (`join`, or `thread::scope` as above) instead of pushing into a shared `Arc<Mutex<Vec<_>>>`.
- To hand work between threads, send it over `mpsc::channel`: the receiver owns each item, and no lock is held while processing it. `Receiver` is single-consumer.
- `Arc::clone` is an atomic increment on a shared cache line. Don't clone per item in a hot loop. Scoped threads can borrow `&T` instead, and a thread that needs the `Arc` can clone it once.

## Avoid

```rust,legacy
// lint: mutex_atomic, await_holding_lock
use std::sync::Mutex;
use std::thread;

pub fn count_matches(data: &[u32], target: u32) -> usize {
    let total = Mutex::new(0usize); // an AtomicUsize, or better a per-thread count
    let shared = &total;
    thread::scope(|s| {
        for chunk in data.chunks(1024) {
            s.spawn(move || {
                for &x in chunk {
                    if x == target {
                        *shared.lock().unwrap() += 1; // lock per item: threads queue up
                    }
                }
            });
        }
    });
    total.into_inner().unwrap()
}

pub async fn refresh(cache: &Mutex<Vec<u32>>) {
    let mut guard = cache.lock().unwrap();
    std::future::ready(()).await; // guard held across .await
    guard.clear();
}
```

## Verify

- `cargo clippy`: `await_holding_lock` is on by default. `mutex_atomic` is restriction, so enable it with `-W clippy::mutex_atomic`. `significant_drop_tightening` (nursery) flags guards that live longer than needed, but it is noisy, so run it by hand.
- Run with 1 thread and with N threads, and check that both give the same result. If N is not faster, profile what holds the lock (see `measure-before-optimizing`).
