# modern-rust-guidance

An [Agent Skill](https://agentskills.io) that keeps coding agents from writing outdated Rust. It holds 26 verified guides plus 3 hubs on current std APIs, performance and testing. Each guide states the Rust version it needs and the clippy lints that detect the old pattern. The agent checks your project's MSRV first, so it never suggests an API your toolchain lacks.

- **Guides** (`skills/modern-rust-guidance/guides/`): `std` (let-else, `LazyLock`, combinators, edition 2024, lints, …), `performance` (allocation, copies, buffered I/O, release profile, contention, …), `testing` (assertions, isolation, seams, doc tests, …). Every code snippet is compiled and every lint marker checked (`mrg check --compile --clippy`).
- **`mrg`** (optional CLI, no dependencies, no network): `search`, `retrieve` (MSRV-aware), `msrv`, and `scan`, which runs clippy with every guide's lints and groups findings by guide. Without it the agent falls back to reading `index.md`.
- **Scope**: std only. It doesn't cover async/tokio, third-party crate choice, or API design yet (see [roadmap](docs/roadmap.md)).

## Install

The skill is the folder `skills/modern-rust-guidance/` (it contains `SKILL.md`, `index.md`, `guides/` and the CLI source). Clone this repo, then copy or symlink that folder into your agent's skills directory. A symlink keeps it updated with `git pull`.

```sh
git clone <this-repo-url> ~/src/modern-rust-guidance
SKILL=~/src/modern-rust-guidance/skills/modern-rust-guidance
```

| Agent | User-wide (all projects) | One project |
| --- | --- | --- |
| Claude Code | `~/.claude/skills/` | `.claude/skills/` |
| Codex CLI | `~/.agents/skills/` | `.agents/skills/` |
| Gemini CLI | `~/.gemini/skills/` or `~/.agents/skills/` | `.gemini/skills/` or `.agents/skills/` |
| OpenCode | `~/.config/opencode/skills/` (also reads `~/.claude/skills`, `~/.agents/skills`) | `.opencode/skills/` (also `.claude/skills`, `.agents/skills`) |
| Cursor | `~/.cursor/skills/` or `~/.agents/skills/` | `.cursor/skills/` or `.agents/skills/` |
| GitHub Copilot | `~/.copilot/skills/` or `~/.agents/skills/` | `.github/skills/`, `.claude/skills/` or `.agents/skills/` |
| pi | `~/.pi/agent/skills/` | `.pi/skills/` |

```sh
# example: Claude Code, user-wide
mkdir -p ~/.claude/skills && ln -s "$SKILL" ~/.claude/skills/modern-rust-guidance

# example: any agent that reads .agents/skills, for one project
mkdir -p .agents/skills && ln -s "$SKILL" .agents/skills/modern-rust-guidance
```

`~/.agents/skills/` is the shared path: Codex, Gemini CLI, OpenCode, Cursor and Copilot all read it, so one symlink there covers them. Restart the agent (or start a new session) afterwards. Use a copy (`cp -r`) instead of a symlink if the agent doesn't follow symlinks.

**With GitHub CLI** (once this repo is on GitHub; needs `gh` ≥ 2.90 with `gh skill`):

```sh
gh skill install <owner>/modern-rust-guidance modern-rust-guidance --agent claude-code --scope user
# other --agent values: codex, gemini-cli, cursor, github-copilot, opencode, …
```

Paths above come from each agent's docs (checked 2026-11); if one has moved, the folder is a plain Agent Skill and any directory your agent scans works.

### Optional: the `mrg` CLI

```sh
cargo install --path skills/modern-rust-guidance/cli   # needs Rust ≥ 1.85; installs `mrg`
mrg msrv && mrg search "early return"
```

The agent never installs it by itself. Without it, search quality is lower (the agent reads `index.md`) and `scan` is unavailable.

## Use

Just ask for Rust work: "review this crate for outdated idioms", "make this faster", "add tests". The skill triggers on its description. The agent then checks the MSRV, finds and reads the relevant guides, applies them, runs each guide's `## Verify` steps, and ends with a `### Modern Rust Guidance` section listing which guides it applied. For a whole-crate audit, ask for `mrg scan` (it builds the project).

## How it performed

Trials on real projects are in [docs/reviews/](docs/reviews/): ousagi (memcached server), caskade, axum and tokio. In short: searches almost always put the right guide first; `scan` findings were true positives but low priority on already-modern code; on tuned projects (axum, tokio) the skill mostly confirms "no change" and finds weak test assertions. It does not help with async, Tower or unsafe/atomics review.

## Repository layout

| Path | Contents |
| --- | --- |
| `skills/modern-rust-guidance/` | The skill: `SKILL.md`, `index.md` (generated), `guides/`, `cli/` |
| `docs/STATUS.md` | Where things stand and next steps. Start here when contributing |
| `docs/authoring.md`, `docs/cli.md` | Guide format rules and the `mrg` spec |
| `docs/guides-plan.md`, `docs/roadmap.md`, `docs/decisions.md` | Scope, plans, decisions |
| `docs/reviews/`, `docs/research/` | Review and trial reports; phase 0 research |

## Develop

```sh
cargo test                                  # CLI tests
cargo clippy --all-targets -- -D warnings
cargo run -q -- check --compile --clippy    # validate every guide (compiles snippets with rustc)
cargo run -q -- index > skills/modern-rust-guidance/index.md   # regenerate the catalog
```

`check --compile` runs `rustc` on the guides' code, so only run it on guides you trust.

## Credits and resources

Nothing was installed from these. Other skills and repos were cloned and read only, then used for ideas; the content was written fresh and checked against primary sources. Each guide lists its own sources in its frontmatter. Full notes: [docs/research/](docs/research/).

**Design reference**
- [GoogleChrome/modern-web-guidance](https://github.com/GoogleChrome/modern-web-guidance) and its [authoring repo](https://github.com/GoogleChrome/modern-web-guidance-src): the model for a version-aware guidance skill with a CLI, guide format and evals.
- [Agent Skills](https://agentskills.io) open standard.

**Existing Rust skills surveyed** (security-audited in [02-existing-rust-skills.md](docs/research/02-existing-rust-skills.md))
- [massimodeluisa/rust-best-practices-skill](https://github.com/massimodeluisa/rust-best-practices-skill) (MIT): allocation profiles, measurement protocol and test layers inform the performance and testing guides.
- [leonardomso/rust-skills](https://github.com/leonardomso/rust-skills) (MIT): rule format and the RAII test-guard idea.
- [morganlinton/BeautifulRust](https://github.com/morganlinton/BeautifulRust) (MIT): the idea of CI-checking that each listed clippy lint fires, which became `mrg check`.
- [apollographql/rust-best-practices](https://github.com/apollographql/rust-best-practices) (Apache-2.0): test naming, one behavior per test, the doctest and nextest caveat.
- Reviewed and not used: [curtisault/rust-skills](https://github.com/curtisault/rust-skills), [actionbook/rust-skills](https://github.com/actionbook/rust-skills), [lx-industries/ms-rust-skill](https://gitlab.com/lx-industries/ms-rust-skill).

**Reference material** (facts and versions verified against these)
- [Rust std docs and rust-src](https://doc.rust-lang.org/std/) (`#[stable(since)]`), [release notes](https://doc.rust-lang.org/releases.html), [Edition Guide](https://doc.rust-lang.org/edition-guide/), [The Rust Book](https://doc.rust-lang.org/book/), [Cargo Book](https://doc.rust-lang.org/cargo/), [rustc book](https://doc.rust-lang.org/rustc/), [rustdoc book](https://doc.rust-lang.org/rustdoc/) (MIT/Apache-2.0).
- [Clippy](https://github.com/rust-lang/rust-clippy): lint docs and the MSRV table (MIT/Apache-2.0). Every lint name is checked against the installed clippy.
- [Rust Performance Book](https://nnethercote.github.io/perf-book/) (MIT/Apache-2.0).
- [Microsoft Pragmatic Rust Guidelines](https://github.com/microsoft/rust-guidelines) (MIT).
- [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/) (MIT/Apache-2.0).
- [Comprehensive Rust](https://google.github.io/comprehensive-rust/) (prose CC-BY-4.0, code Apache-2.0).
- [Miri](https://github.com/rust-lang/miri) (MIT/Apache-2.0).
- Research: [RustEvo²](https://arxiv.org/abs/2503.16922) (models do worse on newer API changes), which motivates the focus on post-cutoff features.
- Link-only, never copied: Effective Rust (CC BY-NC-ND), Rust Design Patterns (MPL-2.0).

**Tooling used while building**: the pi coding agent with Claude, and the pi extensions pi-lens (diagnostics), pi-web-access (research) and pi-sanity (command guard); trial targets were ousagi and caskade (local projects), [axum](https://github.com/tokio-rs/axum) and [tokio](https://github.com/tokio-rs/tokio).

## License

[MIT](LICENSE), Copyright (c) 2026 Daniel. The guides paraphrase the sources above under their own licenses (all permissive; CC-BY-4.0 prose from Comprehensive Rust is attributed in each guide's `sources`).
