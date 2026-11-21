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
cargo +nightly miri test                    # unit, integration, and doc tests, interpreted
cargo +nightly miri test take_first         # name filter, as with cargo test
MIRIFLAGS="-Zmiri-many-seeds=0..16" cargo +nightly miri test   # 16 seeds: other addresses and thread interleavings
MIRIFLAGS="-Zmiri-disable-isolation" cargo +nightly miri test  # real clock, environment, and file system
```

The first run builds Miri's own standard library and asks before it installs anything. Miri reports: out-of-bounds access and use-after-free, uninitialized or invalid values (a `bool` that is not 0 or 1), misaligned access, violated preconditions (`copy_nonoverlapping` with overlapping ranges), data races, and leaks of memory that is not reachable from a `static`. Aliasing-rule checks (Stacked Borrows, Tree Borrows) are experimental.

Limits:

- It is slow, so shrink workloads (below).
- It runs one execution per seed. Passing means no UB was found on the inputs and interleavings tried, not that the code is sound.
- It does not run arbitrary FFI or most platform APIs (networking, for example). Skip such tests with `#[cfg_attr(miri, ignore = "needs real sockets")]`.
- Isolation is on by default: the clock, environment, and randomness are fake.

## Make unsafe invariants observable

Test the safe wrapper at its edges (empty, one element, full) and count drops: a double drop or a leak changes an `Rc`'s strong count, even without Miri. Write a `// SAFETY:` comment on each `unsafe` block that states why its preconditions hold.

```rust
use std::ptr;

/// Removes and returns the first element.
pub fn take_first<T>(v: &mut Vec<T>) -> Option<T> {
    let len = v.len();
    if len == 0 {
        return None;
    }
    // SAFETY: `len >= 1`, so slot 0 is initialized. It is read out exactly once,
    // the tail is moved down over it, and `set_len` shrinks the vector, so no
    // element is dropped twice or skipped.
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
        assert_eq!(Rc::strong_count(&probe), 5);
        drop(take_first(&mut v)); // a leaked element would leave 5 here
        assert_eq!(Rc::strong_count(&probe), 4);
        drop(v);
        assert_eq!(Rc::strong_count(&probe), 1);
    }

    #[test]
    fn empty_and_single_element() {
        assert_eq!(take_first(&mut Vec::<u8>::new()), None);
        let mut one = vec![7];
        assert_eq!(take_first(&mut one), Some(7));
        assert!(one.is_empty());
    }
}
```

## Stress shared state with scoped threads

`thread::scope` (1.63) lets threads borrow locals, joins them all before returning, and re-panics if one panicked. A `Barrier` releases every thread at once, which raises contention. Assert the final state, never timing. Test a type that has its own `unsafe impl Sync`; a test of `Mutex` itself only tests std.

```rust
use std::cell::UnsafeCell;
use std::sync::atomic::{AtomicBool, Ordering};

pub struct SpinLock<T> {
    locked: AtomicBool,
    value: UnsafeCell<T>,
}

// SAFETY: `locked` serializes every access to `value`.
unsafe impl<T: Send> Sync for SpinLock<T> {}

// Compile-time check that the type really is Send + Sync.
const fn assert_send_sync<T: Send + Sync>() {}
const _: () = assert_send_sync::<SpinLock<u64>>();

impl<T> SpinLock<T> {
    pub const fn new(value: T) -> Self {
        Self { locked: AtomicBool::new(false), value: UnsafeCell::new(value) }
    }

    pub fn with<R>(&self, f: impl FnOnce(&mut T) -> R) -> R {
        while self.locked.compare_exchange_weak(false, true, Ordering::Acquire, Ordering::Relaxed).is_err() {
            std::hint::spin_loop();
        }
        // Release the lock even if `f` panics.
        struct Unlock<'a>(&'a AtomicBool);
        impl Drop for Unlock<'_> {
            fn drop(&mut self) {
                self.0.store(false, Ordering::Release);
            }
        }
        let _unlock = Unlock(&self.locked);
        // SAFETY: the successful compare_exchange above gives this thread exclusive access
        // until `_unlock` is dropped.
        f(unsafe { &mut *self.value.get() })
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

With a wrong ordering (`Relaxed` on the unlock store), a normal run can still pass, especially on x86. Under Miri the same test reports a data race, which is why it must also run there. `#[cfg(miri)]` and `cfg!(miri)` select Miri-only code or smaller constants.

> [!NOTE]
> Miri explores weak-memory behavior only partly. To check complicated atomic code exhaustively, use a model checker such as `loom` or `shuttle`. Both are out of scope here.

## Avoid

- Judging unsafe code by a green `cargo test` alone.
- `unsafe` blocks with no `// SAFETY:` comment, or a comment that restates the code instead of the reason.
- `thread::sleep` to "let the other thread run", and assertions on elapsed time. Use `Barrier`, channels, and `join`.
- Copying the production iteration count into tests that Miri runs.
- Raw pointers in the public API when a safe wrapper can own the invariant.

## Verify

- `cargo clippy --all-targets -- -W clippy::undocumented_unsafe_blocks` is clean (restriction group, opt-in).
- `cargo +nightly miri test` passes on every crate that contains `unsafe`, and CI runs it.
- Each `unsafe` function or block has a test at the empty, one-element, and full cases, and, if it moves values, a drop-count assertion.
