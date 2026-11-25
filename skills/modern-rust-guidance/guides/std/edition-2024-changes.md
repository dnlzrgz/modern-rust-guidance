---
name: edition-2024-changes
description: Write and migrate Rust code for edition 2024: temporary scopes, impl Trait lifetime capture, unsafe extern and attributes, static mut, env::set_var, and reserved gen.
rust-version: "1.85"
clippy-lints: []
sources:
  - "Rust Edition Guide, Rust 2024 chapters (MIT/Apache-2.0)"
  - "Rust release notes 1.82, 1.85, 1.88, 1.99, 1.100 (MIT/Apache-2.0)"
  - "std docs: std::env::set_var (MIT/Apache-2.0)"
---

# Edition 2024 Changes

Edition 2024 (stable since 1.85) changes how some code written in older style behaves or compiles. New projects should use `edition = "2024"`. For existing ones, run `cargo fix --edition`, review its edits, then change the edition and set `rust-version = "1.85"` or higher. Run `cargo fmt` as a **separate commit** afterwards: edition 2024 also switches rustfmt to style edition 2024, which reformats code. Rust 1.82 added several 2024 features to all editions (`use<..>`, `unsafe extern`, `unsafe(..)` attributes, `&raw`), so you can adopt them before switching.

## Temporaries drop sooner

In `if let`, scrutinee temporaries (lock guards, `RefCell` borrows) are dropped **before the `else` branch**. In 2021 the guard lived through `else`, so the code below deadlocked. In a block's tail expression, temporaries are dropped **before** the block's locals.

```rust
use std::cell::RefCell;
use std::sync::RwLock;

pub fn init(slot: &RwLock<Option<u32>>) {
    if let Some(v) = *slot.read().unwrap() {
        println!("have {v}");
    } else {
        *slot.write().unwrap() = Some(1); // 2021: deadlock (read guard still alive); 2024: fine
    }
}

pub fn len(c: &RefCell<String>) -> usize {
    let local = RefCell::new(String::new());
    local.borrow().len() + c.borrow().len() // `local.borrow()` no longer outlives `local`
}
```

The reverse also happens: a tail-expression temporary that lived to the end of the statement may now die at the end of the block.

```rust,compile_fail
pub fn n() -> usize {
    let x = { &String::from("1234") }.len(); // 2024: temporary dropped at end of block
    x
}
```

Fix it by binding the block's value first: `let s = { &String::from("1234") }; s.len()`. To keep the old `if let` semantics, rewrite it as `match`. The lints `if_let_rescope` and `tail_expr_drop_order` flag affected code.

## `impl Trait` in return position captures every lifetime

In 2024, return-position `impl Trait` captures all in-scope lifetimes and type parameters. Drop the `+ '_` and `Captures` workarounds. Opt out with `use<..>` (1.82) when the result must not borrow the arguments. `use<..>` must still list every type parameter in scope (they are always captured); only lifetimes are optional.

```rust
pub fn chars(s: &str) -> impl Iterator<Item = char> {
    s.chars() // 2021 needed `+ '_`; 2024 captures the borrow of `s`
}

pub fn indices(s: &str) -> impl Iterator<Item = u32> + use<> {
    0..(s.len() as u32) // captures nothing, so the caller may drop `s` while iterating
}
```

## Unsafe things say so

```rust
// `unsafe extern` is required. `safe fn` items need no `unsafe` at the call site.
unsafe extern "C" {
    pub safe fn abs(x: i32) -> i32;
    pub unsafe fn strlen(p: *const std::ffi::c_char) -> usize;
}

// no_mangle, export_name, and link_section must be wrapped in unsafe(..).
#[unsafe(no_mangle)]
pub extern "C" fn plugin_version() -> u32 {
    1
}

// Operations inside an `unsafe fn` body need their own `unsafe` block (warn by default).

static mut BUF: [u8; 4] = [0; 4];

// `&BUF`, `&mut BUF`, and `BUF.len()` are denied (`static_mut_refs`). A raw borrow (1.82) is fine.
pub fn set_first(b: u8) {
    let p = &raw mut BUF;
    // SAFETY: only called from one thread.
    unsafe { p.cast::<u8>().write(b) }
}
```

Prefer an atomic, a `Mutex`, or `LazyLock`/`OnceLock` to `static mut` (see `lazy-initialization`).

`std::env::set_var` and `remove_var` are `unsafe` because they race with other threads reading the environment. Prefer not to call them: pass values with `Command::env`, or parse once at startup. If you must, call them before any thread starts, with a `// SAFETY:` comment.

## Avoid

2021-style code that no longer compiles in edition 2024:

```rust,compile_fail
extern "C" {
    fn abs(x: i32) -> i32; // error: extern blocks must be unsafe
}
```

```rust,compile_fail
#[no_mangle] // error: unsafe attribute used without unsafe
pub extern "C" fn plugin_version() -> u32 {
    1
}
```

```rust,compile_fail
pub fn set() {
    std::env::set_var("MODE", "fast"); // error: call to unsafe function
}
```

```rust,compile_fail
pub fn g() -> u32 {
    let gen = 1; // error: `gen` is a reserved keyword
    gen
}
```

Write `r#gen` instead, for variables and methods alike (`rng.gen()` becomes `rng.r#gen()`). `cargo fix --edition` does this.

## Smaller changes

- `Box<[T]>` implements `IntoIterator` by value (1.80). In 2024, `boxed.into_iter()` yields `T`, not `&T`.
- A pattern matched through a reference cannot add `mut`, `ref`, or `&`: `let (mut a, b) = &pair;` is an error. Write the binding mode explicitly or dereference first.
- The `expr` macro fragment also matches `const { .. }` and `_`. `expr_2021` keeps the old behavior.
- `Future` and `IntoFuture` are in the prelude, which can make a user trait's `.poll()` ambiguous.
- Let chains (`if let … && …`, needs 1.88) work in edition 2024 only. See `early-return-patterns`.
- Never type (1.100+): `!` is stable, and never-to-any coercions fall back to `!` instead of `()` in **every** edition. Code that relied on a `()` fallback can stop inferring or change behavior: add an explicit type. The `never_type_fallback_flowing_into_unsafe` (deny) and `method_call_on_diverging_infer_var` lints flag the risky spots.
- Cargo: `edition = "2024"` implies `resolver = "3"` (honors `rust-version`). Underscore keys such as `dev_dependencies` and `default_features`, and `[project]`, are errors. In a 2024 package, `default-features = false` on a `workspace = true` dependency overrides the workspace setting (needs 1.99; earlier versions reject it). `cargo fix --edition` removes the key, so re-add it if you want the override.

## Older MSRV

- **Below 1.85:** edition 2024 is not available. Stay on 2021, and enable `-W rust-2024-compatibility` to see what would change.
- **Below 1.82:** no `use<..>`, `unsafe extern`, `#[unsafe(..)]`, or `&raw`. Use `+ '_` and `static mut` with `addr_of_mut!`.
- **Below 1.88:** no let chains.
- **Below 1.100:** `!` cannot be named as a type on stable, and editions before 2024 still fall back to `()`.
- **Below 1.99:** `default-features` on an inherited dependency is rejected in a 2024 package.

## Verify

- `cargo fix --edition` then `cargo build` and `cargo test`. Review each `match` it introduced in place of `if let`, then commit `cargo fmt` separately.
- `RUSTFLAGS="-W rust-2024-compatibility" cargo check` lists remaining incompatibilities (`if_let_rescope`, `tail_expr_drop_order`, `impl_trait_overcaptures`, …) before you switch.
- Run tests that hold locks or `RefCell` borrows across `if let` / tail expressions, since drop order changed silently.
