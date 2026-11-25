---
name: early-return-patterns
description: Handle missing values, parse failures, and nested conditions with early returns instead of deeply nested match and if let.
rust-version: "1.88"
clippy-lints: [manual_let_else, question_mark, collapsible_if, collapsible_match]
sources:
  - "Rust release notes 1.65 (let-else) and 1.88 (let chains) (MIT/Apache-2.0)"
  - "Rust Edition Guide, 2024: let chains in if and while; if let temporary scope (MIT/Apache-2.0)"
  - "Clippy lint docs: manual_let_else, question_mark, collapsible_if, collapsible_match (MIT/Apache-2.0)"
---

# Early Return Patterns

Keep the happy path at the left margin: bail out of the failure case first, then continue. Use `?` when the failure can be passed up unchanged, `let … else` when you must pick the diverging action yourself, and let chains (1.88, edition 2024 only) when a condition mixes patterns and booleans. Error type design is in `error-values`; combinators like `ok_or_else` are in `option-result-combinators`.

## Use `?` when the caller can take the failure as is

`?` works on `Option` and on `Result` (with a `From` conversion of the error). Never hand-write `match … { Err(e) => return Err(e) }` or `if x.is_none() { return None; }`.

```rust
pub fn host_and_port(url: &str) -> Option<(&str, u16)> {
    let (_scheme, rest) = url.split_once("://")?; // None propagates
    let (host, port) = rest.split_once(':')?;
    Some((host, port.parse().ok()?))
}
```

## Use `let … else` (1.65) to extract or diverge

The `else` block must not fall through: `return`, `continue`, `break`, `panic!`, or another `!` expression. The binding stays in scope after the statement, so the rest of the function is not nested.

```rust
pub fn total(lines: &[&str]) -> Result<u32, String> {
    let mut sum = 0;
    for line in lines {
        // `continue` skips blank lines without nesting the rest of the body.
        let Some((_, qty)) = line.split_once('=') else {
            continue;
        };
        // Need the parse error text, so `let Ok(..) else` would discard it: use map_err + `?`.
        let qty: u32 = qty.trim().parse().map_err(|e| format!("{line}: {e}"))?;
        sum += qty;
    }
    Ok(sum)
}
```

Notes: the initializer cannot end in `}` (an `if`/`match` directly before `else` is rejected; wrap it in parentheses or bind it first), and the `else` block cannot see the pattern's bindings. `let Ok(x) = res else { return … }` throws the `Err` away, so use it only when the error is irrelevant.

## Chain conditions with let chains (1.88, edition 2024 only)

In edition 2024, `if` and `while` accept `&&` chains that mix `let` patterns and booleans. This replaces nested `if let { if cond { … } }`. It does not compile in edition 2021 or earlier. `||` is not allowed, and `let` must be at the top level of the chain. `matches!(v, Pat if guard)` is a plain `bool`, so it also composes inside a chain.

```rust
use std::collections::HashMap;

pub fn lookup_limit(cfg: &HashMap<String, String>, key: &str) -> Option<u32> {
    if let Some(raw) = cfg.get(key)
        && let Ok(n) = raw.trim().parse::<u32>()
        && n > 0
    {
        Some(n)
    } else {
        None
    }
}
```

Edition 2024 drops `if let` scrutinee temporaries (guards, borrows) *before* the `else` branch, which removes the classic re-lock deadlock (see `edition-2024-changes`). It does **not** change `match` scrutinees or `while let`: a guard created in a `match` scrutinee still lives through every arm.

```rust
use std::sync::Mutex;

pub fn pop_or_refill(m: &Mutex<Vec<u32>>) -> u32 {
    // `match m.lock().unwrap().pop() { None => m.lock()… }` would deadlock: the guard lives through the arms.
    let item = m.lock().unwrap().pop(); // guard dropped at the semicolon
    match item {
        Some(x) => x,
        None => {
            m.lock().unwrap().push(1);
            1
        }
    }
}
```

## Avoid

Hand-rolled `match` that returns early, nested `if let`, and `is_none()` followed by `return None`:

```rust,legacy
// lint: manual_let_else, question_mark, collapsible_if
pub fn port(url: Option<&str>) -> u16 {
    let url = match url {
        Some(u) => u,
        None => return 80,
    };
    url.len() as u16
}

pub fn first_doubled(v: &[u32]) -> Option<u32> {
    let x = v.first().copied();
    if x.is_none() {
        return None;
    }
    Some(x? * 2)
}

pub fn parse_positive(a: Option<&str>) -> u32 {
    if let Some(s) = a {
        if let Ok(n) = s.parse::<u32>() {
            return n;
        }
    }
    0
}
```

Also avoid a `match` whose only arm re-matches the inner value (`collapsible_match`: put the inner pattern into the outer arm), and `.unwrap()` after an `is_some()` check (use `if let` or a let chain).

## Older MSRV

- **Let chains need 1.88 and edition 2024.** On older toolchains or edition 2021, nest the `if let`, or use `let … else` with `continue`/`return` on each failed step.
- `collapsible_if` only suggests let chains when the MSRV is 1.88 or newer and the crate is edition 2024.

## Verify

- `cargo clippy` flags `question_mark`, `collapsible_if`, and `collapsible_match` by default. `manual_let_else` is pedantic: run it with `-W clippy::manual_let_else`.
- Every `else` of a `let … else` should `return`, `continue`, or `break` with intent. A bare `else { panic!() }` on external input deserves a second look.
- When you convert `if let … else` to a chain or `match`, re-check any lock guard or `RefCell` borrow in the scrutinee (edition 2024 temporary scope).
