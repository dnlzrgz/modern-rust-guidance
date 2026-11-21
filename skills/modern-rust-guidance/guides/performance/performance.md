---
name: performance
description: Make Rust code faster in the right order, measuring first, and find the guide for allocations, copies, strings, I/O, loops, type sizes, and release builds.
rust-version: "1.70"
clippy-lints: []
sources:
  - "Rust Performance Book: General Tips, Benchmarking, Build Configuration, Linting (MIT/Apache-2.0)"
  - "Microsoft Pragmatic Rust Guidelines: M-HOTPATH, M-THROUGHPUT (MIT)"
  - "massimodeluisa rust-best-practices: performance.md (MIT)"
---

# Performance

Rust is fast by default once you build with `--release`. Most remaining wins come from measuring, then fixing the few places a profile points at. Models guess instead, rewrite cold code, and ship "optimizations" nobody measured. Work in the order below, and change one thing at a time.

## Order of attack

1. **Use a release build.** `cargo run` and `cargo build` produce unoptimized `dev` builds, often many times slower. Measure `--release`.
2. **Measure and profile** a representative workload. Fix what the profile shows, not what looks slow.
3. **Try the no-code wins**: `lto`, `codegen-units`, and `panic` in the release profile. Cheap to try, but each has costs, so measure it.
4. **Fix algorithms and data structures first.** Complexity, repeated work, lazy evaluation, and cache-friendly layout usually beat micro-optimizations. No guide here covers this, so reason about it and profile.
5. **Cut allocations and copies**, then **buffer I/O**, then tighten **loops** and **type sizes**.
6. **Re-measure.** Keep a change only if it shows up in the numbers. A clear but slower-looking version beats a clever version with no measured gain.

## Guides

| ID | Use when |
| --- | --- |
| `measure-before-optimizing` | You are about to make anything faster: release builds, profiling profile, profilers, std-only micro-benchmarks. |
| `release-profile-tuning` | Tuning `[profile.release]`: `lto`, `codegen-units`, `panic`, `strip`, `opt-level`, `target-cpu`, PGO. |
| `allocation-reuse` | A profile shows `malloc`/`free`, or a loop builds `Vec`s and `String`s: `with_capacity`, buffer reuse, `clone_from`, `mem::take`. |
| `needless-copies` | Code has `clone()`, `to_owned()`, `to_string()`, or `collect()` that may be avoidable: borrowing, iterators, `Cow`. |
| `string-building` | Building strings with `format!` in loops, `push_str(&format!(..))`, or `map(format!).collect()`. |
| `buffered-io` | Reading or writing files, sockets, stdin, or stdout in many small operations, or reading line by line. |
| `bounds-checks-and-iteration` | A hot loop indexes slices or `Vec`s: iterators, `zip`, chunks, up-front length checks. |
| `type-size-and-layout` | Many values of one type, large enum variants, large `Result` errors, large arrays, big futures, or by-value moves of big structs. |

## Habits to unlearn

- Optimizing before measuring, or measuring a `dev` build.
- `clone()` to quiet the borrow checker, and `collect()` into a `Vec` that is only iterated once.
- `format!` inside loops, and unbuffered `File` writes or `println!` per line.
- `lto = false` as "LTO off" (it is thin-local LTO; `"off"` disables it) and `target-cpu=native` on a binary that ships to other machines.
- `unsafe` `get_unchecked` before trying iterators or an up-front length check.
- Quoting speedup numbers nobody measured. Report the command, input, and before/after numbers.

## Clippy

The `perf` group is warn-by-default, so plain `cargo clippy` already reports those lints. Several useful ones are opt-in (pedantic, nursery, or restriction), such as `needless_pass_by_value`, `format_push_string`, and `redundant_clone`. Each guide lists its lints under `## Verify`. Fix clippy findings in hot code first. In cold code, prefer clarity.

> [!NOTE]
> Out of scope in this phase, and a reason to measure rather than assume: faster hashers than std's DoS-resistant default (`foldhash`, `rustc-hash`), alternative allocators (`mimalloc`, `tikv-jemallocator`), small-vector crates (`smallvec`, `arrayvec`), data parallelism (`rayon`), and benchmark harnesses (`criterion`, `divan`). Inline attributes (`#[inline]`, `#[cold]`) belong after a profile shows a hot call boundary, and with a benchmark. Do not add dependencies without asking the user.
