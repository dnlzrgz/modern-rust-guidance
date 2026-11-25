---
name: unsafe-and-concurrency-testing
description: Test unsafe code and shared-state concurrency with Miri, drop-counting tests, and scoped-thread stress tests that shrink their workload under Miri.
rust-version: "1.70"
clippy-lints: [undocumented_unsafe_blocks]
sources:
  - "Miri README: usage, flags, limitations (MIT/Apache-2.0)"
  - "std docs: std::thread::scope, std::sync::Barrier (MIT/Apache-2.0)"
  - "Microsoft Pragmatic Rust Guidelines: M-STATIC-VERIFICATION (MIT)"
  - "Clippy lint docs: undocumented_unsafe_blocks (MIT/Apache-2.0)"
  - "massimodeluisa rust-best-practices, quality.md: test layers (MIT)"
---

# Unsafe and Concurrency Testing

Ordinary tests pass on `unsafe` code that has undefined behavior, because the UB often has no visible effect. Run the test suite under Miri, an interpreter that reports UB, and write tests that make the invariants observable: drop counts, boundary sizes, and many threads released at once. Miri is a nightly rustup component. If it is not installed, ask the user to run `rustup +nightly component add miri`; do not install it yourself.

## Run the existing tests under Miri

```sh
cargo +nightly miri test                 # unit, integration, and doc tests, interpreted
cargo +nightly miri test take_first      # name filter, as with cargo test
MIRIFLAGS="-Zmiri-many-seeds=0..16" cargo +nightly miri test  # 16 seeds: other addresses and interleavings
MIRIFLAGS="-Zmiri-tree-borrows" cargo +nightly miri test      # Tree Borrows instead of Stacked Borrows
```

The first run builds Miri's own standard library. Miri reports out-of-bounds access, use-after-free, invalid values (a `bool` that is not 0 or 1), misaligned access, violated preconditions (`copy_nonoverlapping` with overlapping ranges), data races, and leaks. Aliasing checks are experimental; the default is Stacked Borrows, and a program valid under one model can be rejected by the other. `MIRIFLAGS` applies to doctests too.
Limits: it is slow, so shrink workloads (below). Passing means no UB on the inputs tried, not soundness. It cannot run arbitrary FFI or most platform APIs (networking): skip those tests with `#[cfg_attr(miri, ignore = "needs real sockets")]`. Isolation is on by default (fake clock, environment, randomness); `-Zmiri-disable-isolation` turns it off.

## Make unsafe invariants observable

Test the safe wrapper at its edges (empty, one element, full) and count drops: a double drop or a leak changes an `Rc`'s strong count, even without Miri. Each `unsafe` block gets a `// SAFETY:` comment stating why its preconditions hold.

```rust
use std::ptr;

/// Removes and returns the first element.
pub fn take_first<T>(v: &mut Vec<T>) -> Option<T> {
    let len = v.len();
    if len == 0 {
        return None;
    }
    // SAFETY: `len >= 1`, so slot 0 is initialized. It is read out once, the tail
    // is moved down over it, and `set_len` shrinks the vector: no double drop.
    unsafe {
        let first = ptr::read(v.as_ptr());
        ptr::copy(v.as_ptr().add(1), v.as_mut_ptr(), len - 1);
        v.set_len(len - 1);
        Some(first)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::rc::Rc;

    #[test]
    fn every_element_is_dropped_exactly_once() {
        let probe = Rc::new(());
        let mut v: Vec<Rc<()>> = (0..4).map(|_| Rc::clone(&probe)).collect();
        drop(take_first(&mut v)); // a leaked element would leave 5 here
        assert_eq!(Rc::strong_count(&probe), 4);
        drop(v);
        assert_eq!(Rc::strong_count(&probe), 1);
    }

    #[test]
    fn empty_and_single_element() {
        assert_eq!(take_first(&mut Vec::<u8>::new()), None);
        assert_eq!(take_first(&mut vec![7]), Some(7));
    }
}
```

## Stress shared state with scoped threads

`thread::scope` (1.63) lets threads borrow locals, joins them all before returning, and re-panics if one panicked. With a bare `thread::spawn`, a panic in a thread nobody joins does **not** fail the test: join it with `join().unwrap()`, or use `scope`. A `Barrier` releases every thread at once, which raises contention. Assert the final state, never timing. Test a type that has its own `unsafe impl Sync`; a test of `Mutex` itself only tests std.

```rust
use std::cell::UnsafeCell;
use std::sync::atomic::{AtomicBool, Ordering};

pub struct SpinLock<T> {
    locked: AtomicBool,
    value: UnsafeCell<T>,
}

// SAFETY: `locked` serializes every access to `value`.
unsafe impl<T: Send> Sync for SpinLock<T> {}

impl<T> SpinLock<T> {
    pub const fn new(value: T) -> Self {
        Self { locked: AtomicBool::new(false), value: UnsafeCell::new(value) }
    }

    pub fn with<R>(&self, f: impl FnOnce(&mut T) -> R) -> R {
        while self.locked.compare_exchange_weak(false, true, Ordering::Acquire, Ordering::Relaxed).is_err() {
            std::hint::spin_loop();
        }
        // SAFETY: the successful compare_exchange above gives this thread exclusive access
        // until the `store` below. (Not panic-safe: a panic in `f` leaves it locked.)
        let result = f(unsafe { &mut *self.value.get() });
        self.locked.store(false, Ordering::Release);
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Barrier;
    use std::thread;

    #[test]
    fn concurrent_increments_are_not_lost() {
        const THREADS: usize = 4;
        // Miri is slow: shrink the workload, keep the thread count.
        const PER_THREAD: u64 = if cfg!(miri) { 20 } else { 20_000 };
        let lock = SpinLock::new(0u64);
        let start = Barrier::new(THREADS);
        thread::scope(|s| {
            for _ in 0..THREADS {
                s.spawn(|| {
                    start.wait();
                    for _ in 0..PER_THREAD {
                        lock.with(|n| *n += 1); // a plain, non-atomic increment
                    }
                });
            }
        });
        assert_eq!(lock.with(|n| *n), THREADS as u64 * PER_THREAD);
    }
}
```

With a wrong ordering (`Relaxed` on the unlock store), a normal run can still pass, especially on x86. Miri reports a data race for that mistake in this test, but it explores only some schedules and weak-memory outcomes, so use `-Zmiri-many-seeds` and don't read a pass as proof. `#[cfg(miri)]` and `cfg!(miri)` select Miri-only code or smaller constants.

> [!NOTE]
> Model checkers such as `loom` or `shuttle` (out of scope here) check complicated atomic code exhaustively.

## Avoid

Judging unsafe code by a green `cargo test` alone; a `// SAFETY:` comment that restates the code instead of giving the reason; `thread::sleep` to "let the other thread run" (use `Barrier`, channels, `join`; see `test-isolation`); production iteration counts under Miri.

## Verify

- `cargo clippy --workspace --all-targets -- -W clippy::undocumented_unsafe_blocks` is clean (restriction group, opt-in).
- `cargo +nightly miri test` passes on every crate with `unsafe`, in CI too.
- Each `unsafe` function has empty, one-element, and full cases, plus a drop-count assertion if it moves values.
