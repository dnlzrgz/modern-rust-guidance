---
name: performance
description: Make Rust code faster in the right order, measuring first, and find the guide for data structures, allocations, copies, strings, I/O, loops, threads, type sizes, and release builds.
rust-version: "1.70"
clippy-lints: []
sources:
  - "Rust Performance Book: General Tips, Benchmarking, Build Configuration, Linting, Parallelism (MIT/Apache-2.0)"
  - "Microsoft Pragmatic Rust Guidelines: M-HOTPATH, M-THROUGHPUT (MIT)"
  - "massimodeluisa rust-best-practices: performance.md (MIT)"
---

# Performance

Rust is fast by default once you build with `--release`. Most remaining wins come from measuring, then fixing the few places a profile points at. Models guess instead, rewrite cold code, and ship "optimizations" nobody measured. Work in the order below, and change one thing at a time.

## Order of attack

1. **Use a release build.** `cargo run` and `cargo build` produce unoptimized `dev` builds, often many times slower. Measure `--release`.
2. **Measure and profile** a representative workload. Fix what the profile shows, not what looks slow.
3. **Fix algorithms and data structures before micro-tuning.** A quadratic scan, a wrong collection, or repeated work outweighs every allocation and bounds-check tweak. See `data-structure-choice`.
4. **Try the build-config wins**: `lto` and `codegen-units` in the release profile. Cheap to try, but they cost build time, so measure. `panic = "abort"` is not a free win: it changes behavior (see `release-profile-tuning`).
5. **Cut allocations and copies**, then **buffer I/O**, then tighten **loops** and **type sizes**.
6. **Split CPU-bound work across threads** only when the single-threaded code is already reasonable.
7. **Re-measure.** Keep a change only if it shows up in the numbers. A clear but slower-looking version beats a clever version with no measured gain.

## Guides

| ID | Use when |
| --- | --- |
| `measure-before-optimizing` | You are about to make anything faster: release builds, profiling profile, profilers, std-only micro-benchmarks. |
| `data-structure-choice` | A loop scans a `Vec` with `contains`, removes from the front, indexes strings by character, nests `Vec<Vec<T>>`, or uses `LinkedList`: sets, `VecDeque`, entry API, sorting, flat grids. |
| `release-profile-tuning` | Tuning `[profile.release]`: `lto`, `codegen-units`, `panic`, `strip`, `opt-level`, `target-cpu`, PGO. |
| `allocation-reuse` | A profile shows `malloc`/`free`, or a loop builds `Vec`s and `String`s: `with_capacity`, buffer reuse, `clone_from`, `mem::take`. |
| `needless-copies` | Code has `clone()`, `to_owned()`, `to_string()`, or `collect()` that may be avoidable: borrowing, iterators, `Cow`, `Arc<str>`. |
| `string-building` | Building strings with `format!` in loops, `push_str(&format!(..))`, or `map(format!).collect()`. |
| `buffered-io` | Reading or writing files, sockets, stdin, or stdout in many small operations. |
| `bounds-checks-and-iteration` | A hot loop indexes slices or `Vec`s: iterators, `zip`, chunks, up-front length checks. |
| `type-size-and-layout` | Many values of one type, large enum variants, large `Result` errors, large arrays, big futures, or by-value moves of big structs. |
| `shared-state-contention` | Threads share an `Arc<Mutex<_>>` or `RwLock`, a guard is held across slow work or `.await`, or a shared counter is hot: short critical sections, per-thread accumulation, atomics. |

## Threads and async

Std-only parallelism is `thread::scope` (threads borrow local data and are joined before it returns) sized by `thread::available_parallelism()`. Use it for CPU-bound work that splits into independent chunks, and see `shared-state-contention` for the sharing rules. Do not spawn threads for tiny inputs, where start-up costs more than the work. Async is for waiting on many I/O operations, not for speeding up computation. An `async fn` with no `.await` is a smell that `unused_async` (pedantic) reports.

## Habits to unlearn

- Optimizing before measuring, or measuring a `dev` build.
- `Box<dyn Trait>` or `&dyn Trait` calls in a hot loop: the call is indirect and usually cannot be inlined. Use generics (`impl Trait`) or an enum for a closed set of types, and keep `dyn` for open sets and cold paths.
- `unsafe` `get_unchecked` before trying iterators or an up-front length check.
- Quoting speedup numbers nobody measured. Report the command, input, and before/after numbers.

## Clippy

The `perf` group is warn-by-default, so plain `cargo clippy` already reports those lints. Several useful ones are opt-in (pedantic, nursery, or restriction), such as `needless_pass_by_value`, `redundant_clone`, and `unused_async`. Each guide lists its lints under `## Verify`. Fix clippy findings in hot code first. In cold code, prefer clarity.

> [!NOTE]
> Out of scope in this phase, and a reason to measure rather than assume: faster hashers than std's DoS-resistant default (`foldhash`, `rustc-hash`), alternative allocators (`mimalloc`, `tikv-jemallocator`), small-vector crates (`smallvec`, `arrayvec`), data parallelism (`rayon`), and benchmark harnesses (`criterion`, `divan`). Inline attributes (`#[inline]`, `#[cold]`) belong after a profile shows a hot call boundary, and with a benchmark. Do not add dependencies without asking the user.
