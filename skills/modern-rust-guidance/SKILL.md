---
name: modern-rust-guidance
description: Current Rust standard-library idioms, performance practice, testing practice, and async language features, checked against the project's MSRV. Use when writing, reviewing, refactoring, optimizing, or testing Rust code, and when modernizing older Rust. Covers std APIs stabilized after ~1.70 that models often miss, edition 2024 changes, allocation/copy/IO performance, release profiles, std test-harness practice, async fn in traits, async closures, and pinning and polling. Not for choosing third-party crates.
license: MIT
---

# Modern Rust Guidance

Training data is full of older Rust: `lazy_static!`, `match`-and-return instead of `let … else`, `map_or(false, …)`, manual `div_ceil`, clones to quiet the borrow checker, untested binaries. These guides hold the current, verified practice. Each one lists the Rust version it needs and the clippy lints that detect the old pattern.

The guides live in `guides/<category>/<id>.md` (categories: `std`, `performance`, `testing`, `ecosystem`), and `index.md` lists all of them. If the `mrg` binary is on `PATH` (`command -v mrg`), use it. Otherwise read `index.md` and open the guide files directly. Do not install `mrg` yourself. Tell the user they can install it with `cargo install --path <this skill dir>/cli`.

## Workflow

1. **Check the MSRV.** Run `mrg msrv`, or read `rust-version` in `Cargo.toml` (also check `[workspace.package]`, then `clippy.toml` `msrv`). Never raise the MSRV without asking the user. If none is declared, the code itself shows the floor. For example, let chains mean 1.88 or newer.
   In a workspace the root may declare no MSRV while members do: pass `--manifest-path <member>/Cargo.toml` to `mrg msrv` and to `mrg retrieve` (or use `retrieve --msrv X.Y`). Also check `edition`: the guides assume 2024, so for an older edition see `edition-2024-changes` before suggesting syntax that needs it.
   Also read the project's `[lints]` table and `clippy.toml`. Project lint settings override guide snippets: if the project sets `expect_used = "deny"`, don't copy a snippet's `.expect(..)` into non-test code. If the project already enables a guide's lint (for example `manual_let_else`), expect few hits and don't flag what the lint would have caught.
2. **Find guides.** Run `mrg search "<2–5 words about the task>"`. Use one query per distinct sub-task and leave out project-specific names. Results come with a relevance score, and weak matches are cut. Treat anything well below the top score as a long shot. Without `mrg`, scan `index.md` descriptions. For broad work (reviews, "make this faster", "add tests"), also read the category hub: `std`, `performance`, `testing`, or `ecosystem` (async: `async fn` in traits, async closures, `pin!` and hand-written polling). In reviews, use the `std` hub's "what changed" table as a checklist.
3. **Read the guides before writing code.** Run `mrg retrieve <id>[,<id>…]`. It adds an MSRV banner and drops fallback sections the project doesn't need. Without `mrg`, open the files. If the project's MSRV is below a guide's `rust-version`, follow its `## Older MSRV` section.
4. **Apply them, then verify.** Run what each guide's `## Verify` section lists, normally `cargo clippy` with the guide's lints and `cargo test`.
5. **Disclose.** End your reply with a `### Modern Rust Guidance` section. Give one bullet per guide you actually applied: `` - **`<id>`**: <what you changed or recommended because of it> ``. Add one final line, `Checked, no change: <id>, <id>, …`, for guides you retrieved and found already satisfied. Only list IDs you retrieved in this session.

## Modernizing an existing codebase

Use this only when the user asks for an audit, a modernization, or a review of a whole crate.

Run `mrg scan [path]`. It runs `cargo clippy` with every guide's lints enabled and groups the findings by guide. This **builds the project**, so build scripts and proc macros run. Say so before you run it in a repository you didn't create. Then retrieve each reported guide and confirm every finding against it. Lints have false positives, and some findings are deliberate (MSRV, readability).

- Rank findings by impact, not by count. A finding from a pedantic, nursery, or restriction lint that the project doesn't enable is a style suggestion: say so, and keep it low priority. Findings in tests and benches usually matter less than findings in library code.
- Libraries with unsafe code, custom atomics or their own sync wrappers (runtimes, lock-free structures) are tuned deliberately. Don't suggest changing orderings, locks or lint setup there without loom, Miri or a measurement.
- Before suggesting a performance change, look for deliberate tuning: the profile settings, benches, and commit messages. Don't override measured choices without a measurement of your own.
- Report the findings first, and change code only when asked. Without `mrg`, collect the `clippy-lints` from the relevant guides' frontmatter (also listed in `index.md`) and run `cargo clippy -- -W clippy::<lint> …`.

## Scope

- Async is covered only for the language and std side (`ecosystem` hub). The tokio runtime, tracing, axum, Tower, hyper and reqwest have no guides yet, and neither do API design or semver. For those, say the guides don't cover it instead of stretching the nearest guide (`buffered-io` is for blocking `std::io`).
- Crate guides apply only to a crate the project already depends on (check `Cargo.lock`), and only to the version it resolves. Never add a dependency or a cargo feature without asking the user, and never add `--cfg tokio_unstable`. When a guide names another crate as the usual tool (criterion, proptest, insta, nextest, thiserror, async-trait, …), it does so in a note.
- The guides assume the 2024 edition and current stable. Version notes say when something needs a newer toolchain.
