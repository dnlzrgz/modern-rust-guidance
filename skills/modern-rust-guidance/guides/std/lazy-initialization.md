---
name: lazy-initialization
description: Initialize global statics, lazily computed values, and thread-local state without lazy_static, once_cell, or static mut.
rust-version: "1.80"
clippy-lints: [non_std_lazy_statics, missing_const_for_thread_local]
sources:
  - "std docs: std::sync::LazyLock, std::sync::OnceLock, std::cell::LazyCell, std::thread_local (MIT/Apache-2.0)"
  - "Rust Edition Guide, 2024: Disallow references to static mut (MIT/Apache-2.0)"
  - "Rust release notes 1.80, 1.94 (MIT/Apache-2.0)"
  - "Clippy lint docs: non_std_lazy_statics, missing_const_for_thread_local (MIT/Apache-2.0)"
---

# Lazy Initialization

Global or lazily computed state needs no macro, no crate, and no `unsafe`. Pick by when the value is known: a `const` constructor means a plain `static`; a known but non-`const` initializer means `LazyLock` (1.80); a value that only exists at runtime means `OnceLock` (1.70). Per-thread state is `thread_local!`. For why `static mut` stopped working in edition 2024, see `edition-2024-changes`.

## Use a plain `static` when the constructor is `const`

`Mutex::new`, `RwLock::new`, `Condvar::new` (all `const` since 1.63), atomics, `Vec::new`, `String::new`, `BTreeMap::new` (1.66), and `VecDeque::new` (1.68) need no lazy wrapper. `HashMap::new` and `HashSet::new` are **not** `const` (`with_hasher` is, but needs 1.85), so a `Mutex<HashMap<..>>` static needs `LazyLock`.

```rust
use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};

// `Mutex::new` and `Vec::new` are const: no lazy wrapper.
pub static EVENTS: Mutex<Vec<String>> = Mutex::new(Vec::new());

// `HashMap::new` is not const: build it on first access (1.80).
pub static REGISTRY: LazyLock<Mutex<HashMap<String, u32>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

pub fn register(name: &str, id: u32) {
    // LazyLock derefs to the inner value; the closure runs once, on first deref.
    REGISTRY.lock().unwrap().insert(name.to_owned(), id);
}
```

`LazyLock::force(&X)` runs the initializer eagerly, e.g. at startup, so a failure surfaces early. `LazyLock::get(&X)` (needs 1.94) returns `Option<&T>` without running the initializer. If the closure panics, the `LazyLock` is poisoned for good: every later access panics. Keep initializers infallible, or use `OnceLock` when you must handle errors.

## Use `OnceLock` when the input arrives at runtime

```rust
use std::sync::OnceLock;

static CONFIG_PATH: OnceLock<String> = OnceLock::new();

// Called once from main after argument parsing; Err(value) if already set.
pub fn init(path: String) -> Result<(), String> {
    CONFIG_PATH.set(path)
}

// Or initialize on first use from whatever is available.
pub fn path() -> &'static str {
    CONFIG_PATH.get_or_init(|| std::env::var("APP_CONFIG").unwrap_or_default())
}
```

`get_or_init` runs one closure even under races; if it panics the cell stays empty. Initializing the same cell from inside its own closure is an error (currently a deadlock). `OnceLock::get_or_try_init` is nightly-only, so do not use it: call `get()`, compute, then `set()`, and accept that a racing thread may compute twice.

`LazyCell` and `OnceCell` (the non-`Sync` twins, 1.80 and 1.70) are for struct fields and `thread_local!` values, not `static`s.

## Give `thread_local!` a `const` initializer

With `const { .. }` the value is built at compile time and skips the lazy-initialization check on every access. It requires a `const` expression. `LocalKey` gets direct accessors (1.73): `with_borrow`, `with_borrow_mut`, `set`, `get`, `take`, `replace`.

```rust
use std::cell::{Cell, RefCell};

thread_local! {
    static DEPTH: Cell<u32> = const { Cell::new(0) };
    static SCRATCH: RefCell<Vec<u8>> = const { RefCell::new(Vec::new()) };
}

pub fn nest() -> u32 {
    DEPTH.replace(DEPTH.get() + 1) // no `.with(|d| ...)` closure needed (1.73)
}

pub fn fill(n: usize) -> usize {
    SCRATCH.with_borrow_mut(|buf| {
        buf.clear();
        buf.resize(n, 0);
        buf.len()
    })
}
```

## Avoid

`lazy_static!`, `once_cell::sync::Lazy`, and `once_cell::sync::OnceCell`: use `LazyLock` / `OnceLock`.

> [!NOTE]
> `lazy_static` and `once_cell` are crates, which are out of scope here. The std types above replace them for the common cases. `once_cell::race` and `OnceBox` have no std equivalent.

```rust,ignore
lazy_static! { static ref TABLE: HashMap<u8, &'static str> = build(); }
static NAME: once_cell::sync::Lazy<String> = once_cell::sync::Lazy::new(make_name);
```

`static mut` with `unsafe` reads and writes. Edition 2024 denies references to it (`static_mut_refs`). Use `Mutex<Vec<u32>>` (const-constructible), an atomic, or `LazyLock`/`OnceLock` instead.

```rust,compile_fail
static mut LOG: Vec<u32> = Vec::new();

pub fn add(x: u32) {
    unsafe { LOG.push(x) } // error: creates a mutable reference to a mutable static
}
```

`thread_local!` without `const` when the initializer is `const`-capable:

```rust,legacy
// lint: missing_const_for_thread_local
use std::cell::RefCell;

thread_local! {
    static SCRATCH: RefCell<Vec<u8>> = RefCell::new(Vec::new());
}
```

A `Mutex<Option<T>>` plus `.unwrap()` as a hand-rolled lazy global.

## Older MSRV

- **1.70 to 1.79:** no `LazyLock`/`LazyCell`. Use `OnceLock` behind an accessor: `fn registry() -> &'static Mutex<HashMap<..>> { static R: OnceLock<Mutex<HashMap<..>>> = OnceLock::new(); R.get_or_init(Default::default) }`.
- **Before 1.73:** use `KEY.with(|c| ..)` instead of `with_borrow_mut`, `get`, `set`, `replace`.
- `non_std_lazy_statics` only fires when MSRV is 1.80 or newer.

## Verify

- `cargo clippy` flags `missing_const_for_thread_local` by default. `non_std_lazy_statics` is pedantic and only matters while `lazy_static`/`once_cell` are still dependencies: run it with `-W clippy::non_std_lazy_statics`.
- `rg "static mut|lazy_static"` should come back empty after the change. On MSRV 1.80 or newer, `rg once_cell` should too, unless you use `once_cell::race` or `OnceBox`.
- In review, check that no `LazyLock` initializer can panic or call back into its own static.
