---
name: modern-rust-guidance
description: Current Rust standard-library idioms, performance practice, and testing practice, checked against the project's MSRV. Use when writing, reviewing, refactoring, optimizing, or testing Rust code, and when modernizing older Rust. Covers std APIs stabilized after ~1.70 that models often miss, edition 2024 changes, allocation/copy/IO performance, release profiles, and std test-harness practice. Not for choosing third-party crates.
---

# Modern Rust Guidance

Training data is full of older Rust: `lazy_static!`, `match`-and-return instead of `let … else`, `map_or(false, …)`, manual `div_ceil`, clones to quiet the borrow checker, untested binaries. These guides hold the current, verified practice. Each one lists the Rust version it needs and the clippy lints that detect the old pattern.

The guides live in `guides/<category>/<id>.md` (categories: `std`, `performance`, `testing`), and `index.md` lists all of them. If the `mrg` binary is on `PATH` (`command -v mrg`), use it. Otherwise read `index.md` and open the guide files directly. Do not install `mrg` yourself. Tell the user they can install it with `cargo install --path <this skill dir>/cli`.

## Workflow

1. **Check the MSRV.** Run `mrg msrv`, or read `rust-version` in `Cargo.toml` (also check `[workspace.package]`, then `clippy.toml` `msrv`). Never raise the MSRV without asking the user.
2. **Find guides.** Run `mrg search "<2–5 words about the task>"`. Use one query per distinct sub-task and leave out project-specific names. Without `mrg`, scan `index.md` descriptions. For broad work (reviews, "make this faster", "add tests"), also read the category hub: `std`, `performance`, or `testing`.
3. **Read the guides before writing code.** Run `mrg retrieve <id>[,<id>…]`. It adds an MSRV banner and drops fallback sections the project doesn't need. Without `mrg`, open the files. If the project's MSRV is below a guide's `rust-version`, follow its `## Older MSRV` section.
4. **Apply them, then verify.** Run what each guide's `## Verify` section lists, normally `cargo clippy` with the guide's lints and `cargo test`.
5. **Disclose.** End your reply with a `### Modern Rust Guidance` section. Give one bullet per guide you actually applied: `` - **`<id>`**: <what you changed because of it> ``. Only list IDs you retrieved in this session.

## Modernizing an existing codebase

Use this only when the user asks for an audit, a modernization, or a review of a whole crate.

Run `mrg scan [path]`. It runs `cargo clippy` with every guide's lints enabled and groups the findings by guide. This **builds the project**, so build scripts and proc macros run. Say so before you run it in a repository you didn't create. Then retrieve each reported guide and confirm every finding against it. Lints have false positives, and some findings are deliberate (MSRV, readability). Report the findings first, and change code only when asked. Without `mrg`, collect the `clippy-lints` from the relevant guides' frontmatter (also listed in `index.md`) and run `cargo clippy -- -W clippy::<lint> …`.

## Scope

- Phase 1 covers std only. When a guide names a crate as the usual tool (criterion, proptest, insta, nextest, thiserror, …), it does so in a note. Don't add dependencies without asking the user.
- The guides assume the 2024 edition and current stable. Version notes say when something needs a newer toolchain.
