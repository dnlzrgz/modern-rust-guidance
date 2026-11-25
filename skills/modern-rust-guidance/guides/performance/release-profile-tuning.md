---
name: release-profile-tuning
description: Tune Cargo's release profile for runtime speed or binary size with lto, codegen-units, panic, strip, and target-cpu, and know the trade-offs.
rust-version: "1.70"
clippy-lints: []
sources:
  - "Cargo Book: Profiles (MIT/Apache-2.0)"
  - "Rust Performance Book: Build Configuration (MIT/Apache-2.0)"
  - "rustc book: Profile-guided Optimization, codegen options (MIT/Apache-2.0)"
---

# Release Profile Tuning

Build settings change speed, binary size, build time, and debuggability without touching code. Change them one at a time and measure each (see `measure-before-optimizing`). Cargo reads profiles only from the workspace-root `Cargo.toml`. Profile settings in dependencies are ignored, so this matters for binaries, not libraries.

## Know the defaults

`[profile.release]` defaults: `opt-level = 3`, `debug = false`, `strip = "none"`, `debug-assertions = false`, `overflow-checks = false`, `lto = false`, `panic = "unwind"`, `incremental = false`, `codegen-units = 16`. Writing `opt-level = 3` changes nothing. Release has no overflow checks or `debug_assert!`, so it behaves differently from `dev`.

## Start here for speed

```toml
[profile.release]
lto = "fat"          # optimize across all crates in the build; try "thin" too
codegen-units = 1    # one unit per crate: slower build, more optimization
```

Both slow the build, so measure the gain on a real workload. If `cargo install` users build your crate, these must be in `release`. If you ship CI-built binaries, keep `release` fast for everyday use and put the heavy settings in a custom profile:

```toml
[profile.dist]
inherits = "release"   # a custom profile must name its parent
lto = "fat"
codegen-units = 1
```

Build with `cargo build --profile dist`. The output goes to `target/dist/`.

To A/B a setting without editing `Cargo.toml`, override it from the environment: `CARGO_PROFILE_RELEASE_LTO=fat CARGO_PROFILE_RELEASE_CODEGEN_UNITS=1 cargo build --release` (the pattern is `CARGO_PROFILE_<name>_<KEY>`). It rebuilds into `target/release`, so copy the old binary aside first.

## Get `lto` values right

| Value | Meaning |
| --- | --- |
| `false` (the default) | "Thin local" LTO across this crate's own codegen units only. No LTO at all if `codegen-units = 1` or `opt-level = 0`. |
| `"off"` | LTO disabled. |
| `"thin"` | Thin LTO across the dependency graph: much faster than fat, often similar gains. |
| `true` or `"fat"` | Fat LTO across the dependency graph. Slowest build. |

`lto = false` is not `"off"`: it still runs thin-local LTO, but never across crates. With `codegen-units = 1`, `lto = false` means no LTO at all. Do not write it expecting whole-program optimization, and write `"off"` when you mean off.

## Decide on `panic = "abort"`

`panic = "abort"` can shrink the binary and speed it up slightly, but it changes behavior:

- A panic terminates the process. There is no unwinding, so destructors do not run: a `BufWriter` is not flushed and temp-file guards are not cleaned up.
- `catch_unwind` catches nothing, and a panic in any thread kills the whole program, so `JoinHandle::join` never returns `Err` for a panic.
- Tests, benchmarks, build scripts, and proc macros ignore the setting, so tests still unwind.

Use it for applications that treat a panic as fatal. Never set it for a library, and ask before enabling it in a project that uses `catch_unwind` or a panic-isolating server.

## Shrinking binaries

```toml
[profile.release]
opt-level = "z"   # "s" is less aggressive; "z" also turns off loop vectorization
strip = true      # same as "symbols"; "debuginfo" keeps symbol names
```

`opt-level = 3` is sometimes slower than `2`, and `"s"`/`"z"` are not always smaller, so measure. Stripped binaries lose names in backtraces and profiles, so never strip the build you profile.

## Tune dependencies separately

A package override changes settings for chosen packages only. `"*"` matches every dependency but not workspace members, and overrides cannot set `panic`, `lto`, or `rpath`:

```toml
[profile.release.package."*"]
opt-level = "s"   # dependencies only: your own crates keep the release settings
```

## Do not use `target-cpu=native` for shipped binaries

`-C target-cpu=native` compiles for the CPU of the build machine. rustc states the binary must only run on CPUs with all of that CPU's features, so it can crash with an illegal-instruction error elsewhere. That includes CI builders that differ from production, container images, and binaries downloaded by users. Use it only when the build host is the run host. Do not commit it to a shared `.cargo/config.toml`, where it affects every contributor and CI job.

For a distributed x86-64 binary, either pick a named baseline you are willing to require, such as `-C target-cpu=x86-64-v3` (list them with `rustc --print target-cpus`), or keep the default and select fast paths at run time with `is_x86_feature_detected!`.

## Profile-guided optimization (PGO)

PGO feeds real run data back into the compiler. It needs `llvm-profdata` (ask before installing), identical flags for both builds, and a representative workload. `--target` keeps the flags off build scripts.

```sh
RUSTFLAGS="-Cprofile-generate=/tmp/pgo" cargo build --release --target <triple>
./target/<triple>/release/app <typical workload>
llvm-profdata merge -o /tmp/pgo/merged.profdata /tmp/pgo
RUSTFLAGS="-Cprofile-use=/tmp/pgo/merged.profdata" cargo build --release --target <triple>
```

> [!NOTE]
> Alternative allocators (`mimalloc`, `tikv-jemallocator`) can also change speed and memory use, but they are crates and out of scope here. Measure on the target platform.

## Verify

- `cargo build --release` (or `--profile dist`) succeeds, and you benchmarked that artifact against the previous settings.
- Build time and binary size before and after are reported with the speed change, since this trades one for the other.
- `cargo test` passes, and any `panic = "abort"` decision is confirmed with the user.
