---
name: ecosystem
description: Write async Rust with current language features, find the guide for async traits, closures, pinning and polling, and know the rules for using async crates.
rust-version: "1.70"
clippy-lints: []
sources:
  - "Rust release notes 1.64 to 1.100, std docs and rust-src stability attributes (MIT/Apache-2.0)"
  - "tokio 1.54.1 crate docs and CHANGELOG (MIT)"
  - "Rust blog: Announcing async fn and return-position impl Trait in traits (MIT/Apache-2.0)"
---

# Async and Crate Ecosystem

Async code is where agents most often write code that compiles and then hangs, panics at startup, or leaks. This hub routes to the async-language guides, which cover the language and std only. There is no guide yet for the tokio runtime, tracing, axum, tower, hyper or reqwest: for those, say the guides don't cover it, and still follow the rules below.

## Rules for async crates

- **Crates are in scope only when the project already depends on them.** Check `Cargo.lock` for the crate and its resolved version, and apply only advice that is valid for that version. The allow-list is tokio, tokio-util, bytes, tracing (with tracing-subscriber), axum, axum-extra, tower, tower-http, hyper, hyper-util and reqwest. For any other crate, say the guides don't cover it; a guide may still name one in a `> [!NOTE]` pointer. If a crate is not in `Cargo.lock`, don't apply its guide.
- **Never add a dependency or a cargo feature without asking.** If the fix needs one (for example `tokio-util` for `CancellationToken`), say so and ask first.
- **Never recommend `tokio_unstable` or a nightly-only API.** A `--cfg tokio_unstable` in `.cargo/config.toml` (or `RUSTFLAGS`) applies to every crate in the build, and tokio says unstable APIs may break in 1.x releases, so it is the user's decision, not a fix. See the unstable list below.
- **Branch on the resolved version, not on GitHub `main`.** Examples on `main` may use an unreleased API.
- **Version floors:** axum 0.7, tower-http 0.5, reqwest 0.12, tokio-util 0.7. Below a floor, tell the user to upgrade first; there is no tailored fallback.

## Async language (std only)

These need no crate. They apply to any project on a recent toolchain (current tokio itself needs Rust 1.85).

| ID | Use when |
| --- | --- |
| `async-fn-in-traits` | A trait has async methods: native `async fn` in traits instead of `#[async_trait]`, making the futures `Send` for `tokio::spawn`, and when `dyn Trait` still needs a crate. |
| `async-closures` | A function takes a callback that returns a future: `async \|x\| ..` and `AsyncFn` bounds instead of `Fn(A) -> Fut`, and when to keep the old form. |
| `pinning-and-polling` | Code calls `Box::pin`, `pin_mut!` or `Pin::new_unchecked` just to poll a future, or hand-writes a `Waker`, `poll_fn` or `ready!`. |

## Still unstable: do not suggest

On stable Rust 1.100 these do not compile or are not usable: `std::future::join!`, `std::async_iter::AsyncIterator` and `async gen` blocks, async `Drop`, return type notation (`T::method(..): Send`), `AsyncFn*::async_call*` and `CallRefFuture`, `Waker::from_fn_ptr` and `LocalWaker`. `dyn Trait` with an `async fn` method is still rejected (E0038). Use the stable route:

- `join!` and `select!` → `tokio::join!` and `tokio::select!` (they need tokio's `macros` feature: ask before enabling it), or the `futures` crate's if the project already uses it.
- `Stream` → the `futures-core` trait, which `tokio_stream` re-exports. Use it only if the project already depends on one of them.
- Async cleanup → an explicit `async fn close(self)` or `shutdown()` that the caller awaits.
- Naming a `Send` bound on an async trait method → write `-> impl Future<Output = T> + Send` (see `async-fn-in-traits`).
