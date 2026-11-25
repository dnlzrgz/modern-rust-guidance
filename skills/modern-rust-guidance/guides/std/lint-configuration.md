---
name: lint-configuration
description: Configure compiler and Clippy lints for a package or workspace in Cargo.toml, and suppress individual warnings with a documented reason.
rust-version: "1.81"
clippy-lints: [allow_attributes, allow_attributes_without_reason, lint_groups_priority]
sources:
  - "Cargo Book: The [lints] section, The workspace.lints table (MIT/Apache-2.0)"
  - "Rust release notes 1.74 ([lints]) and 1.81 (#[expect], lint reasons) (MIT/Apache-2.0)"
  - "Rustc Book: Lint levels, unfulfilled_lint_expectations, check-cfg (MIT/Apache-2.0)"
  - "Rust release notes 1.80 (check-cfg lint config), 1.100 ([lints.cargo]) (MIT/Apache-2.0)"
  - "Microsoft Pragmatic Rust Guidelines: M-LINT-OVERRIDE-EXPECT (MIT)"
  - "Clippy docs: configuration, allow_attributes, lint_groups_priority (MIT/Apache-2.0)"
---

# Lint Configuration

Put lint levels in `Cargo.toml`, not in `#![warn(...)]` blocks at the top of `lib.rs`. Suppress a single warning with `#[expect(lint, reason = "...")]` (1.81), which tells you when the suppression is no longer needed. Keep CI strict with a command-line flag, not with `#![deny(warnings)]` in source.

## Declare lints in `Cargo.toml` (1.74)

`[lints.rust]` and `[lints.clippy]` apply to the current package only (not to dependencies). The table name is the part of the lint name before `::`. In a workspace, define the tables once and inherit them.

```toml
# Root Cargo.toml
[workspace]
members = ["crates/*"]
resolver = "2"

[workspace.package]
edition = "2021"        # "2024" needs 1.85
rust-version = "1.81"   # Clippy reads this as its MSRV; no clippy.toml entry needed

[workspace.lints.rust]
unsafe_code = "deny" # not "forbid": `forbid` cannot be overridden, so an FFI member crate could not `#[allow]` it
# Custom cfgs (fuzzing, loom, ...) must be declared here, or `unexpected_cfgs` warns on them (1.80).
unexpected_cfgs = { level = "warn", check-cfg = ['cfg(fuzzing)', 'cfg(loom)'] }

[workspace.lints.clippy]
# A lint GROUP needs a lower priority so the single lints below can override it.
pedantic = { level = "warn", priority = -1 }
module_name_repetitions = "allow"
allow_attributes = "warn"
allow_attributes_without_reason = "warn"
```

```toml
# crates/foo/Cargo.toml
[package]
name = "foo"
edition.workspace = true
rust-version.workspace = true

[lints]
workspace = true   # `[lints]` cannot mix `workspace = true` with its own entries
```

Priority is a signed integer; higher wins, default 0. Without `priority = -1` on the group, Cargo ignores the order in the file, and `clippy::lint_groups_priority` (deny by default) rejects the table. `pedantic` is opinionated: it is shown to demonstrate priority, so enable it only if the team wants it. Cargo's own lints (needs 1.100) use a third table, `[lints.cargo]`. Keep `warn` in `Cargo.toml` and make CI fail with `cargo clippy --all-targets --all-features -- -D warnings`.

## Suppress with `#[expect]` and a reason

`#[expect(lint)]` behaves like `#[allow(lint)]`, but if the lint does not fire, the compiler emits `unfulfilled_lint_expectations`. So stale suppressions are found and removed. Always add `reason = "…"` (also valid on `allow`, `warn`, `deny`, since 1.81). Attach it to the smallest item.

```rust
#[expect(clippy::cast_possible_truncation, reason = "the hash is deliberately folded to 32 bits")]
pub fn fold(hash: u64) -> u32 {
    hash as u32
}
```

List only lints that actually fire in that scope. Each unmet name in an `#[expect]` list triggers its own `unfulfilled_lint_expectations`, so do not convert `#![allow(a, b, c)]` to `#![expect(a, b, c)]` mechanically: run clippy, keep the lints that fire, and drop or split off the rest.

Use `#[allow(lint, reason = "…")]` only when the lint fires in some configurations and not others (`cfg`, features, platform-specific code), because `#[expect]` would warn where it stays quiet. The `allow_attributes` lint (restriction group, 1.81+) flags every outer `#[allow]`, including inside `cfg_attr`, but ignores `#![allow]` and macro output. `allow_attributes_without_reason` (restriction) also covers `#[expect]`, so it is not redundant with `allow_attributes`: enable both, and exempt the rare legitimate `allow` explicitly.

## Configure Clippy itself

Clippy takes its MSRV from `rust-version` in `Cargo.toml` (`CARGO_PKG_RUST_VERSION`). Set `msrv` in `clippy.toml` only when you want a different value. Other settings go in `clippy.toml` at the package or workspace root.

```toml
# clippy.toml
msrv = "1.81"
allow-unwrap-in-tests = true
allow-expect-in-tests = true
```

`allow-*-in-tests` covers only `#[cfg(test)]` code and `#[test]` functions. Benches and examples are separate crates that are not built with `cfg(test)`, so `cargo clippy --all-targets` still lints their `unwrap`, `expect`, and `panic`. Put a crate-level attribute at the top of the bench or example file (`allow_attributes` ignores inner attributes):

```rust
#![allow(clippy::unwrap_used, clippy::expect_used, reason = "bench setup may panic")]

pub fn main() {
    let n = std::env::args().next().expect("argv[0]");
    println!("{n}");
}
```

Use `allow` here when you can't tell which of the lints fire. If you use `#![expect(..)]`, name only the lints that fire in that file.

## Avoid

- `#![deny(warnings)]` in source: a newer compiler adds warnings and a dependency upgrade breaks the build. Use `-D warnings` in CI.
- Bare `#[allow(…)]` with no reason, crate-wide `#![allow(clippy::all)]` or `#![allow(dead_code)]`, and suppressions copied from a tutorial. Fix the code or scope an `#[expect]` to one item.
- A lint group in `[lints]` without a negative priority (see above).
- Duplicating the lint table in every member of a workspace: use `[workspace.lints]` plus `workspace = true`.

```rust,legacy
// lint: allow_attributes, allow_attributes_without_reason
#[allow(clippy::needless_range_loop)]
pub fn sum(v: &[u32]) -> u32 {
    let mut total = 0;
    for i in 0..v.len() {
        total += v[i];
    }
    total
}
```

## Older MSRV

- **Below 1.81:** `#[expect]` and `reason = "…"` are not stable. Use `#[allow(lint)]` with a comment above it, and review suppressions by hand.
- **Below 1.74:** no `[lints]` table. Put `#![warn(...)]` attributes in the crate root, or pass `-W` flags via `RUSTFLAGS`/`.cargo/config.toml`.
- Clippy reads `rust-version` only for the package being checked. Pin the toolchain in CI so a newer Clippy does not add lints you did not choose.

## Verify

- `cargo clippy --all-targets --all-features -- -D warnings` passes, and `rg "allow\("` in `src/` shows only entries with a `reason`.
- `unfulfilled_lint_expectations` is a warning. A new one means a suppression, or one name in an `expect` list, can be deleted. Run with `--all-targets`: benches and examples are not covered by the `in-tests` options.
- After moving lints into `[workspace.lints]`, run `cargo clippy` in one member to confirm it picks them up.
- `allow_attributes` and `allow_attributes_without_reason` are restriction lints: they are off unless you enable them in `[lints.clippy]` as above.
