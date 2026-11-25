---
name: test-isolation
description: Keep tests independent when they run in parallel, with unique temp directories, no environment mutation, OS-assigned ports, and no sleep-based waiting.
rust-version: "1.70"
clippy-lints: []
sources:
  - "The Rust Book, ch. 11 Running Tests: tests run in parallel (MIT/Apache-2.0)"
  - "std docs: std::env::set_var Safety section, std::net::TcpListener, std::sync::mpsc (MIT/Apache-2.0)"
  - "Edition Guide, Rust 2024: Unsafe functions (MIT/Apache-2.0)"
  - "Cargo Book: cargo test (working directory), Profiles (panic) (MIT/Apache-2.0)"
  - "Microsoft Pragmatic Rust Guidelines: M-MOCKABLE-SYSCALLS (MIT)"
  - "leonardomso rust-skills, test-fixture-raii (MIT): the RAII guard idea only; its env-var guard is not adopted"
---

# Test Isolation

Within one test binary, `cargo test` runs tests on parallel threads of a single process. The environment, working directory, fixed file paths, fixed ports, and `static`s are shared by all of them. A test that passes alone and fails in the suite is almost always sharing one of these. Make every test own what it touches. Cargo runs separate test binaries one after another, so only tests in the same binary can collide.

## Give each test its own directory

A fixed path like `/tmp/out.txt` collides, and a `remove_file` at the end is skipped when an assertion panics. Create a unique directory and delete it in `Drop`. Tests are built with unwinding (Cargo ignores `panic = "abort"` for tests), so the guard runs when a test fails.

```rust
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};

/// A fresh, empty directory, removed when dropped.
pub struct TempDir(PathBuf);

impl TempDir {
    pub fn new(label: &str) -> std::io::Result<Self> {
        static NEXT: AtomicU32 = AtomicU32::new(0);
        let n = NEXT.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!("{label}-{}-{n}", std::process::id()));
        // `create_dir` fails if the path exists, so a collision is an error, not sharing.
        fs::create_dir(&path)?;
        Ok(Self(path))
    }

    pub fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
// In a test: `let dir = TempDir::new("parser")?;` and write under `dir.path()`.
```

> [!NOTE]
> The `tempfile` crate is the usual answer (out of scope here); the guard above is the std-only version. Don't add it without asking the user.

Cargo runs each test with the package root as the working directory, so read fixtures with `env!("CARGO_MANIFEST_DIR")` or a relative path. Never call `set_current_dir`; it changes the directory for every thread.

## Never change the environment from a test

`std::env::set_var` and `remove_var` are `unsafe` in edition 2024. Other threads, including libc and std internals such as DNS lookups, read the environment without going through Rust, so on most platforms the only sound option in a multi-threaded process is not to call them. A test binary is one. Wrapping the call in `unsafe` only asserts that this holds. Nothing in a test run guarantees it.

```rust,compile_fail
// error[E0133] in edition 2024: call to unsafe function `set_var`
pub fn enable_debug_for_test() {
    std::env::set_var("APP_DEBUG", "1");
}
```

Pass configuration in instead: take a `get: impl Fn(&str) -> Option<String>` closure, which real code feeds `|key| std::env::var(key).ok()` (worked example in `test-seams`).

To test a binary's reading of the environment, set it on the child: `Command::new(env!("CARGO_BIN_EXE_my_tool")).env("APP_DEBUG", "1")` changes only that process.

## Let the OS pick ports

Bind to port `0`, then read the port back. The listener exists before any client connects, so no retry loop or sleep is needed. Never "find a free port, close it, and reuse the number": another test can take it in between.

```rust,ignore
let listener = TcpListener::bind("127.0.0.1:0")?; // port 0: the OS picks a free one
let addr = listener.local_addr()?;                // clients connect to `addr`
let server = std::thread::spawn(move || listener.accept()); // already listening: no sleep
```

## Wait for events, not for time

`sleep(100ms)` is too short on a loaded CI machine and wasteful everywhere else. Block on the thing you wait for: `join`, a channel, a `Barrier`, a `Condvar`. A timeout is only an upper bound that turns a hang into a failure.

For example, `rx.recv_timeout(Duration::from_secs(10))` returns the moment the message arrives, and 10 s is only the failure bound. A worker thread whose result matters is `join`ed, and `join().unwrap()` surfaces its panic (see `unsafe-and-concurrency-testing`).

Do not assert that something took less than N milliseconds. Inject the clock instead (see `test-seams`).

## Keep global state out of the code under test

A `static` counter, cache, or `OnceLock` in the code under test is shared by every test in the binary, in any order. Prefer an instance each test creates. If a true global must be exercised, give it its own `tests/<name>.rs`: a separate binary is a separate process (see `test-organization`). Seed any randomness with a fixed value and print it on failure (see `meaningful-tests`).

If the global cannot be instanced (a process-wide metrics counter, say) and you only need to see that the code touched it, read it before and after and assert a lower bound. Never `assert_eq!(counter, 1)` on the absolute value, or on `after - before == 1`: another test running in parallel on a different thread may bump the same counter between your two reads, so `==` passes or fails by scheduling. A lower bound holds however the other tests interleave.

```rust
use std::sync::atomic::{AtomicU64, Ordering};

static REQUESTS: AtomicU64 = AtomicU64::new(0);

pub fn handle() {
    REQUESTS.fetch_add(1, Ordering::Relaxed);
}

pub fn requests() -> u64 {
    REQUESTS.load(Ordering::Relaxed)
}

#[test]
fn handle_counts_a_request() {
    let before = requests();
    handle();
    assert!(requests() > before, "counter did not move"); // not `== before + 1`: other tests share it
}
```

This proves the counter moved, not that it moved exactly once. For an exact count, use an instance or a separate `tests/<name>.rs` binary where this is the only test.

## Avoid

- Exact `==` assertions on a shared global counter (use a before/after lower bound or an instance).
- `#[serial]`-style locks, and `-- --test-threads=1` as a fix: it is a diagnostic that confirms shared state, not a remedy.
- Fixed names such as `/tmp/test.db`, fixed ports, and cleanup code at the end of the test body.
- `set_var`, `remove_var`, and `set_current_dir` in tests, tests that rely on a particular order, and `thread::sleep` as synchronization.

## Verify

- Run `cargo test -- --test-threads=1` and `cargo test -- --test-threads=16`. Both must pass, repeatedly. A test that passes only with 1 thread shares state.
- Optional guardrail in `clippy.toml` (the `disallowed_methods` lint is on by default but needs this config), if production code doesn't use them either: `disallowed-methods = [{ path = "std::env::set_var", reason = "racy" }, { path = "std::env::remove_var", reason = "racy" }, { path = "std::env::set_current_dir", reason = "process-wide" }]`.
- After a failing run, the system temp directory holds no leftover `<label>-<pid>-<n>` directories.
