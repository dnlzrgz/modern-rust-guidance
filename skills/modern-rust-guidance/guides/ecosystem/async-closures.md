---
name: async-closures
description: Pass async callbacks to generic functions with async closures and AsyncFn bounds instead of Fn -> Future, and keep Fn + Send bounds when the future goes to tokio spawn.
rust-version: "1.85"
clippy-lints: []
sources:
  - "Rust release notes 1.85 (MIT/Apache-2.0)"
  - "rust-src core::ops::async_function and core::prelude::v1 (MIT/Apache-2.0)"
  - "rustc lint docs: closure_returning_async_block (MIT/Apache-2.0)"
  - "Rust RFC 3668: Async closures (MIT/Apache-2.0)"
---

# Async Closures

A callback that returns a future used to take two type parameters and an `async move` block: `F: Fn(A) -> Fut, Fut: Future<Output = T>` with `|x| async move { .. }`. Since 1.85, write `async |x| { .. }` and bound it with `AsyncFn`, `AsyncFnMut` or `AsyncFnOnce` (all in the prelude; pick like `Fn`, `FnMut`, `FnOnce`; plain `async fn` items satisfy them too). The future can borrow from the closure and from its arguments, which the old form cannot express. One exception: when the future must be `Send` (anything that ends up in `tokio::spawn`), a generic `AsyncFn` bound cannot say so. Keep the old bounds there.

## Let the future borrow captured state

The old form fails when the async block borrows something the closure captured. `async move` does not rescue it: it moves the captured value out of an `FnMut` closure (E0507), which is why agents then clone everything.

```rust,compile_fail
use std::future::Future;

pub async fn each<F, Fut>(mut f: F) -> u32
where
    F: FnMut(u32) -> Fut,
    Fut: Future<Output = u32>,
{
    f(1).await + f(2).await
}

pub async fn run() -> u32 {
    let mut seen = vec![];
    // error: captured variable cannot escape `FnMut` closure body (and E0373 for `n`)
    each(|n| async { seen.push(n); seen.len() as u32 }).await
}
```

```rust
// `AsyncFnMut` lends `seen` to each call's future. No clone, no `move`.
pub async fn each<F: AsyncFnMut(u32) -> u32>(mut f: F) -> u32 { f(1).await + f(2).await }

pub async fn run() -> u32 {
    let mut seen = vec![];
    each(async |n| { seen.push(n); seen.len() as u32 }).await
}
```

## Borrow from arguments with a higher-ranked bound

`Fn(&T) -> Fut` cannot let `Fut` borrow the `&T`, because `Fut` is one type chosen before the lifetime is known. `AsyncFn(&T)` can.

```rust,compile_fail
use std::future::Future;

pub async fn with_name<F, Fut>(f: F) -> usize
where F: Fn(&String) -> Fut, Fut: Future<Output = usize> {
    f(&String::from("ada")).await
}

pub async fn run() {
    // error: lifetime may not live long enough (the future borrows the argument)
    with_name(|s| async move { s.len() }).await;
}
```

```rust
pub async fn with_name<F: AsyncFn(&String) -> usize>(f: F) -> usize {
    f(&String::from("ada")).await
}

pub async fn run() -> usize {
    with_name(async |s| s.len()).await
}
```

## Keep `Fn -> Fut + Send` when the future must be `Send`

A generic `F: AsyncFn(..) + Send` does not make the futures it returns `Send`, and there is no stable way to add that bound. Awaiting such a call inside a spawned task fails. `spawn` below stands in for `tokio::spawn`.

```rust,compile_fail
pub fn spawn<F: std::future::Future + Send + 'static>(_f: F) {}

pub fn run<F: AsyncFn(u32) -> u32 + Send + 'static>(f: F) {
    // error: future cannot be sent between threads safely
    // (`<F as AsyncFnMut<(u32,)>>::CallRefFuture<'_>` is not `Send`)
    spawn(async move { f(1).await; });
}
```

```rust
use std::future::Future;

pub fn spawn<F: Future + Send + 'static>(_f: F) {}

// The two-parameter form lets you write `Fut: Send`.
pub fn run<F, Fut>(f: F)
where
    F: Fn(u32) -> Fut + Send + 'static,
    Fut: Future<Output = u32> + Send,
{
    spawn(async move { f(1).await; });
}

pub fn caller() {
    // A non-capturing async closure satisfies the old-style bound. One that
    // captures state (`async move |n| n + k`) does not implement `Fn`: pass
    // `move |n| async move { n + k }` instead.
    run(async |n| n + 1);
}
```

`AsyncFn` bounds are fine whenever no `Send` is needed (local or single-threaded code), or when the caller passes a concrete closure and the compiler checks the real future. For a public generic API that users will spawn, assume they need `Send`. The `Fn -> Fut` form cannot express a future that borrows its argument (see the previous section). To get a `Send` future that borrows, box it: `F: for<'a> Fn(&'a T) -> Pin<Box<dyn Future<Output = R> + Send + 'a>> + Send`, at the cost of one allocation per call. Tokio's own callback APIs (such as `sync::OnceCell::get_or_init`) still take `Fn -> Fut`.

## Avoid

A closure that only wraps an async block, with a two-parameter bound to match. It compiles, but it is the shape that `AsyncFn` replaces whenever `Send` is not needed:

```rust,legacy
use std::future::Future;

pub async fn call<F, Fut>(f: F) -> u32
where
    F: Fn(u32) -> Fut,
    Fut: Future<Output = u32>,
{
    f(1).await
}

pub async fn run() -> u32 {
    call(|n| async move { n + 10 }).await
}
```

Do not store `AsyncFn` callbacks as `Box<dyn AsyncFn(..)>`: it is not dyn compatible.

```rust,compile_fail
// error[E0038]: the trait `AsyncFnMut` is not dyn compatible
pub fn store(_cb: Box<dyn AsyncFn(u32) -> u32>) {}
```

For `dyn`, box the future by hand: `Box<dyn Fn(u32) -> Pin<Box<dyn Future<Output = u32> + Send>> + Send + Sync>`.

## Older MSRV

- **Before 1.85:** there is no `async |x|` syntax and no `AsyncFn*` traits. Use `F: Fn(A) -> Fut, Fut: Future<Output = T>` with `|x| async move { .. }`, and clone or `Arc` what the future needs. The `Send` rule above then applies everywhere.

## Verify

- `closure_returning_async_block` is a rustc lint, allow-by-default, not clippy. Run `RUSTFLAGS="-W closure_returning_async_block" cargo check` once. It flags `|x| async move { .. }` and suggests `async |x| { .. }`. Skip its suggestion where the closure feeds a `Fn -> Fut + Send` bound that spawns.
- Add one assertion per public generic API that is meant to be spawned: a function with a `T: Send` bound that is given the future of a call to your API, as `run` above does with `spawn`.
- In review, look for `.clone()` calls made only to feed an `async move` block inside a closure, and for `F: AsyncFn + Send` on a function that spawns.
