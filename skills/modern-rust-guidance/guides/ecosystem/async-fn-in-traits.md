---
name: async-fn-in-traits
description: Define async methods on traits for tokio and spawned tasks without the async_trait macro, keep the futures Send for spawn, and know when dyn Trait still needs a boxed future.
rust-version: "1.75"
clippy-lints: [manual_async_fn]
sources:
  - "Rust blog: Announcing async fn and return-position impl Trait in traits (MIT/Apache-2.0)"
  - "Rust release notes 1.75 (MIT/Apache-2.0)"
  - "rustc lint docs: async_fn_in_trait; error index E0038, E0658 (MIT/Apache-2.0)"
  - "Clippy lint docs: manual_async_fn (MIT/Apache-2.0)"
---

# Async Fn in Traits

Traits can declare `async fn` natively (1.75), so `#[async_trait]` is no longer the default. The catch is `Send`: a bare `async fn` in a trait gives callers no `Send` guarantee on its future, so generic code that hands it to `tokio::spawn` fails to compile. For a trait that is public or will be spawned, declare the method as `fn f(&self) -> impl Future<Output = T> + Send;` and let impls keep writing `async fn`. For closures that return futures, see `async-closures`.

## Declare `+ Send` when the future will be spawned

`spawn` below stands in for `tokio::spawn`: both require `Send + 'static`.

```rust,compile_fail
pub trait Store {
    async fn get(&self, k: u32) -> u32;
}

pub fn spawn<F: std::future::Future + Send + 'static>(_f: F) {}

// error: future cannot be sent between threads safely
// (`impl Future<Output = u32>` from `S::get` is not `Send`)
pub fn run<S: Store + Send + Sync + 'static>(s: S) {
    spawn(async move {
        s.get(1).await;
    });
}
```

```rust
use std::future::Future;

pub trait Store {
    // The trait promises a `Send` future. A trait method's future captures `&self`
    // and every argument lifetime automatically, so there is no `+ '_`.
    fn get(&self, k: u32) -> impl Future<Output = u32> + Send;
}

pub struct Memory;

impl Store for Memory {
    // Impls may still write `async fn`; the `Send` promise is checked here.
    async fn get(&self, k: u32) -> u32 { k + 1 }
}

pub fn spawn<F: Future + Send + 'static>(_f: F) {}

pub fn run<S: Store + Send + Sync + 'static>(s: S) {
    spawn(async move { s.get(1).await; });
}
```

A bare `async fn` is fine when the trait is private to your crate, or when callers only ever use concrete types (the compiler sees through to the real future and its auto traits). The `async_fn_in_trait` lint warns only about publicly reachable traits. Silence it with `#[allow(async_fn_in_trait)]` on a public trait only if you deliberately do not promise `Send`, for example for a single-threaded runtime. The `+ Send` promise is a breaking change to remove, and it rejects impls that hold an `Rc` across an `.await`.

> [!NOTE]
> The `trait-variant` crate generates a `Send` twin of a trait from one definition. It is out of scope here: the desugared form above needs no dependency. Ask before adding a crate.

## `dyn Trait` still does not work with `async fn`

`dyn Trait` cannot have a method that is `async` or that returns `impl Trait` (E0038). This holds even with `+ Send`. Return type notation (`S: Store<get(..): Send>`) would let callers ask for `Send` without changing the trait, but it is still unstable (E0658), so do not use it.

```rust,compile_fail
pub trait Store {
    async fn get(&self, k: u32) -> u32;
}

// error[E0038]: the trait `Store` is not dyn compatible (method `get` is `async`)
pub fn use_dyn(_s: &dyn Store) {}
```

When you really need `dyn`, make the future explicit. This is the std-only form of what `#[async_trait]` expands to, and it costs one allocation per call:

```rust
use std::future::Future;
use std::pin::Pin;

pub trait Store {
    fn get<'a>(&'a self, k: &'a str) -> Pin<Box<dyn Future<Output = u32> + Send + 'a>>;
}

pub struct Memory;

impl Store for Memory {
    fn get<'a>(&'a self, k: &'a str) -> Pin<Box<dyn Future<Output = u32> + Send + 'a>> {
        Box::pin(async move { k.len() as u32 })
    }
}

pub fn spawn<F: Future + Send + 'static>(_f: F) {}

pub fn run(s: Box<dyn Store + Send + Sync>) {
    spawn(async move { s.get("key").await; });
}
```

> [!NOTE]
> The `async-trait` crate hides that boxing behind `#[async_trait]`, and is the usual answer when many methods need `dyn`. Use it only if the project already depends on it, otherwise ask first.

## Symptom, cause, fix

| Symptom | Cause | Fix |
| --- | --- | --- |
| `future cannot be sent between threads safely` ... `impl Future<Output = T>` | bare `async fn` in a generic trait, then spawned | declare `-> impl Future<Output = T> + Send` |
| same error, "required by a bound in `Trait::method::{anon_assoc#0}`" (the name varies by rustc version) | the impl holds a non-`Send` value (`Rc`, `MutexGuard`) across `.await` | fix the impl body; do not drop `+ Send` to silence it |
| `E0038` "method `x` is `async`" | `dyn Trait` over an async method | boxed future as above |
| `E0658` "return type notation is experimental" | `Trait<method(..): Send>` | desugar to `impl Future + Send` |

## Avoid

`#[async_trait]` on traits used only with generics or concrete types: it boxes every call for nothing.

Hand-desugaring in an impl whose trait already declares the method: write `async fn` there.

```rust,legacy
// lint: manual_async_fn
use std::future::Future;

pub trait Store {
    fn get(&self, k: u32) -> impl Future<Output = u32> + Send;
}

pub struct Memory;

impl Store for Memory {
    fn get(&self, k: u32) -> impl Future<Output = u32> + Send {
        async move { k + 1 }
    }
}
```

## Older MSRV

- **Before 1.75:** `async fn` and `-> impl Trait` in traits do not compile. Use the boxed `Pin<Box<dyn Future<Output = T> + Send + '_>>` form shown above, or ask the user about `#[async_trait]`.
- **Edition 2021 and older:** a trait method's `-> impl Future` still captures every lifetime, so the trait declaration needs no change. A free or inherent `fn` returning `impl Future` that borrows its arguments needs `+ '_` (E0700 otherwise); edition 2024 drops that requirement.

## Verify

- `async_fn_in_trait` is a rustc lint (warn by default), not clippy. `cargo check` shows it for every public trait that still has a bare `async fn`.
- `cargo clippy` runs `manual_async_fn` by default; it flags `fn f() -> impl Future { async { .. } }` that should be `async fn`. On a free function it suggests dropping `+ Send`: do not follow that where the future must be `Send`. On a trait impl it has no suggestion, and the fix is `async fn` (see `## Avoid`).
- For each spawnable trait, keep one generic function that calls `spawn`, as in the second example. It fails the build if a future stops being `Send`.
- In review, look for `#[async_trait]` on traits that never become `dyn`.
