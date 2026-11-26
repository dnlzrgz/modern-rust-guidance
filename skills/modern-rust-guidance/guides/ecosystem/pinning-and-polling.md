---
name: pinning-and-polling
description: Poll a future by hand without Box::pin, pin_mut, or Pin::new_unchecked, and replace futures-crate `ready!`, `poll_fn`, and no-op waker helpers with std equivalents.
rust-version: "1.70"
clippy-lints: [manual_noop_waker, waker_clone_wake]
sources:
  - "std docs: std::pin::pin!, std::future::{ready, pending, poll_fn, IntoFuture}, std::task::{ready!, Wake, Waker::noop} (MIT/Apache-2.0)"
  - "rust-src stability attributes for the items above (MIT/Apache-2.0)"
  - "Rust release notes 1.68, 1.77, 1.85 (MIT/Apache-2.0)"
  - "Clippy lint docs: manual_noop_waker, waker_clone_wake (MIT/Apache-2.0)"
---

# Pinning and Polling

To poll a future you own, pin it on the stack with `std::pin::pin!` (1.68). It needs no allocation and no `unsafe`, and it replaces `Box::pin`, `Pin::new_unchecked`, `futures::pin_mut!`, `pin_utils::pin_mut!` and `tokio::pin!` when all you want is a `Pin<&mut F>`. The small helpers people import from `futures` for hand-written futures (`ready`, `pending`, `poll_fn`, `ready!`, a no-op waker) now live in `std`. For async trait methods, see `async-fn-in-traits`.

> [!NOTE]
> `futures`, `pin-utils` and `tokio` stay the source for `Stream`, `join!`, `select!` and `BoxFuture` (std's `join!` is unstable). `pin_mut!` and `tokio::pin!` still work. They have no `#[deprecated]` attribute, though the `futures` docs call `pin_mut!` soft-deprecated since 1.68. They are redundant for plain stack pinning. Rewrite `tokio::pin!(f);` as `let mut f = pin!(f);`: std's macro is an expression, not a rebinding statement. Do not add a crate for anything below. Ask the user first.

## Pin on the stack with `pin!`

`pin!(fut)` moves the future into a hidden local and gives back `Pin<&mut F>`. Poll it with `.as_mut().poll(cx)`, or await it by reference with `(&mut f).await` (for example in a `select!` loop, so a long-lived future is not recreated each iteration).

```rust
use std::future::{poll_fn, Future};
use std::pin::pin;
use std::task::Poll;

// Run `fut` to completion, calling `on_pending` each time it yields.
pub async fn with_progress<F: Future>(fut: F, mut on_pending: impl FnMut()) -> F::Output {
    let mut fut = pin!(fut); // stack-pinned: no allocation, no unsafe
    poll_fn(|cx| match fut.as_mut().poll(cx) {
        Poll::Pending => {
            on_pending();
            Poll::Pending
        }
        done => done,
    })
    .await
}
```

## Use std helpers instead of `futures` ones

| Instead of | Use | Since |
| --- | --- | --- |
| `futures::future::ready`, `pending` | `std::future::ready`, `pending` | 1.48 |
| `futures::future::poll_fn` | `std::future::poll_fn` | 1.64 |
| `futures::ready!` | `std::task::ready!` | 1.64 |
| `futures::task::ArcWake` | `std::task::Wake` with `Waker::from(Arc<W>)` (implement `wake(self: Arc<Self>)`; `wake_by_ref` has a default) | 1.51 |
| `futures::task::noop_waker`, an empty `impl Wake` | `Waker::noop()` | needs 1.85 |
| a builder with a `.send()` that returns a future | `impl IntoFuture`, so callers write `builder.await` | 1.64 |

```rust
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

// `ready!` returns `Poll::Pending` early, the way `?` returns errors.
pub fn poll_len<F: Future<Output = String>>(mut f: Pin<&mut F>, cx: &mut Context<'_>) -> Poll<usize> {
    let s = std::task::ready!(f.as_mut().poll(cx));
    Poll::Ready(s.len())
}
```

## Test runtime-agnostic code without a runtime

`Waker::noop()` (needs 1.85) does nothing when woken. With `pin!` it makes a tiny executor for unit-testing code that needs no timer, socket or other thread. A future waiting on an outside event spins at full speed or never completes, and tokio's timers and sockets panic outside a runtime, so use the runtime's test macros for that code.

```rust
use std::future::{poll_fn, Future};
use std::pin::pin;
use std::task::{Context, Poll, Waker};

// Needs 1.85 for `Waker::noop`. Spins on `Pending`: only for self-driving futures.
pub fn block_on<F: Future>(fut: F) -> F::Output {
    let mut fut = pin!(fut);
    let mut cx = Context::from_waker(Waker::noop());
    loop {
        if let Poll::Ready(out) = fut.as_mut().poll(&mut cx) {
            return out;
        }
    }
}

#[test]
fn yields_once_then_completes() {
    let mut first = true;
    let fut = poll_fn(move |_| match std::mem::take(&mut first) {
        true => Poll::Pending,
        false => Poll::Ready(7),
    });
    assert_eq!(block_on(fut), 7);
}
```

To assert on `Pending`, poll once with `fut.as_mut().poll(&mut cx)` instead of looping.

## Keep `Box::pin` where the future must live on the heap

`pin!` cannot outlive its scope (returning it fails with E0515). Use `Box::pin` for async recursion (needs 1.77), futures stored in a struct, `Vec` or map, type-erased `Pin<Box<dyn Future<Output = T> + Send>>`, futures that must move after being polled (`Pin<Box<F>>` is `Unpin`), and very large futures.

```rust,compile_fail
// error[E0733]: recursion in an async fn requires boxing
pub async fn depth(n: u32) -> u32 {
    if n == 0 { 0 } else { 1 + depth(n - 1).await }
}
```

```rust
use std::future::Future;
use std::pin::Pin;

// Needs 1.77. Boxing the recursive call gives the future a finite size.
pub async fn depth(n: u32) -> u32 {
    if n == 0 { 0 } else { 1 + Box::pin(depth(n - 1)).await }
}

// Mixed futures in one collection need one erased, boxed type.
pub type Job = Pin<Box<dyn Future<Output = u32> + Send>>;
```

## Avoid

`Box::pin` just to get a `Pin<&mut F>` for a local poll, and `unsafe { Pin::new_unchecked(&mut fut) }`, which is sound only if the future never moves again. `pin!` does both safely, with no allocation.

An empty `Wake` impl only to get a do-nothing waker, and `clone().wake()`:

```rust,legacy
// lint: manual_noop_waker, waker_clone_wake
use std::sync::Arc;
use std::task::{Wake, Waker};

struct Noop;
impl Wake for Noop {
    fn wake(self: Arc<Self>) {}
}

pub fn make() -> Waker {
    Waker::from(Arc::new(Noop)) // Waker::noop() (needs 1.85) replaces all of this
}

pub fn nudge(w: &Waker) {
    w.clone().wake(); // refcount bump only to consume it; wake_by_ref() borrows
}
```

Adding `futures` or `pin-utils` for `pin_mut!`, `ready`, `poll_fn` or `noop_waker` alone. Use the std items above.

## Verify

- `manual_noop_waker` (complexity) and `waker_clone_wake` (perf) are warn by default. `manual_noop_waker` suggests `Waker::noop()`, so clippy stays silent when the project's MSRV is below that (needs 1.85). `rg "impl Wake for"` finds the pattern anyway.
- `rg "pin_mut!|Pin::new_unchecked|Box::pin"`: each `Box::pin` should store, erase, recurse, or hold a future that moves. `clippy::large_futures` (pedantic) flags oversized futures that should be boxed.
- Keep `block_on` helpers in test code. A test whose future needs a real wake-up would spin or hang.
