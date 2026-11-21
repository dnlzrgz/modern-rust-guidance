---
name: measure-before-optimizing
description: Measure and profile Rust code before making it faster, with release builds, a profiling profile, external profilers, and a std-only micro-benchmark.
rust-version: "1.71"
clippy-lints: []
sources:
  - "Rust Performance Book: Benchmarking, Profiling, Build Configuration (MIT/Apache-2.0)"
  - "Microsoft Pragmatic Rust Guidelines: M-HOTPATH (MIT)"
  - "Cargo Book: Profiles (MIT/Apache-2.0)"
  - "std docs: std::hint::black_box (MIT/Apache-2.0)"
---

# Measure Before Optimizing

Do not guess where time goes. Build in release mode, find the hot spot with a profiler, change one thing, and measure again. An optimization with no before/after measurement is not done. For the order of attack, see `performance`.

## Always measure a release build

`cargo run` and `cargo build` use the unoptimized `dev` profile. `cargo build --release` and `cargo run --release` use `release`, and `cargo bench` uses `bench`, which inherits `release`. A timing taken from a dev build says nothing about production. `cargo test` uses the `test` profile, which inherits `dev`, so run timing tests with `cargo test --release`.

## Profile first

Profile a release-grade build that keeps line tables, using a custom profile so the real `release` profile stays unchanged:

```toml
[profile.profiling]
inherits = "release"            # same optimizations as release
debug = "line-tables-only"      # lets profilers show source lines
```

Build it with `cargo build --profile profiling`. The binary lands in `target/profiling/`. Then use an external tool on the user's machine: `perf` (Linux), `samply`, or Instruments (macOS) for where CPU time goes; Cachegrind or Callgrind for per-line instruction counts; DHAT or heaptrack for allocation sites. If stacks look broken, rebuild with `RUSTFLAGS="-C force-frame-pointers=yes"`. Do not install profilers on your own. If none is available, say so and fall back to the micro-benchmark below.

> [!NOTE]
> Prebuilt `std` has no debug info, so std frames show up without source lines.

## Micro-benchmark with std only

`#[bench]` needs nightly. On stable, time with `Instant`, pass inputs and outputs through `std::hint::black_box` (1.66), and take the median of several runs:

```rust
use std::hint::black_box;
use std::time::{Duration, Instant};

/// Median time per call of `f` over `runs` runs of `iters` calls each.
pub fn time_it<R>(runs: usize, iters: u32, mut f: impl FnMut() -> R) -> Duration {
    black_box(f()); // warm up caches and lazy statics
    let mut samples: Vec<Duration> = (0..runs)
        .map(|_| {
            let start = Instant::now();
            for _ in 0..iters {
                black_box(f()); // the result must look used
            }
            start.elapsed() / iters
        })
        .collect();
    samples.sort_unstable();
    samples[samples.len() / 2]
}

fn sum_squares(data: &[u64]) -> u64 {
    data.iter().map(|x| x * x).sum()
}

#[test]
#[ignore = "benchmark: cargo test --release -- --ignored --nocapture"]
fn bench_sum_squares() {
    let data: Vec<u64> = (0..100_000).collect();
    // black_box the input too, or the compiler may fold the whole call away
    let t = time_it(15, 100, || sum_squares(black_box(&data)));
    println!("sum_squares: {t:?} per call");
}
```

- `black_box` is best-effort, not a guarantee. Treat a suspiciously fast result as a broken benchmark.
- Use realistic input sizes and shapes. A tiny input measures the harness, not the code.
- Wall time is noisy. Run several times, and when differences are small compare instruction counts (Cachegrind) instead.
- Keep a correctness test next to every optimization.

> [!NOTE]
> `criterion` and `divan` are the usual benchmark crates (they need `harness = false` under `[[bench]]`). They are out of scope for this guide, so ask before adding a dev-dependency.

## Avoid

- Timing a dev build, or timing once.
- "Optimizing" code no profile points at. Algorithm and data-structure changes usually beat micro-tweaks.
- Changing several things at once, so nobody knows which one helped.
- Quoting a speedup you did not measure. Report the command, the input, and before/after numbers.

## Older MSRV

`debug = "line-tables-only"` needs Cargo 1.71. On Cargo 1.70 use `debug = 1` (more debug info, still gives line numbers). `black_box` needs 1.66.

## Verify

- `cargo build --release` (or `--profile profiling`) succeeds, and the measurement used that build.
- The benchmark was run several times, and before/after numbers are in the change description.
- `cargo test` still passes, including correctness tests for the optimized path.
