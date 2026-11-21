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
  - "leonardomso rust-skills, test-fixture-raii (MIT): the RAII guard idea; its env-var guard is deliberately not adopted (see the set_var Safety docs)"
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dirs_are_unique_and_removed_on_drop() -> std::io::Result<()> {
        let (a, b) = (TempDir::new("demo")?, TempDir::new("demo")?);
        assert_ne!(a.path(), b.path());
        let kept = a.path().to_owned();
        fs::write(kept.join("out.txt"), "x")?;
        drop(a);
        assert!(!kept.exists());
        Ok(())
    }
}
```

Cargo runs each test with the package root as the working directory, so read fixtures with `env!("CARGO_MANIFEST_DIR")` or a relative path. Never call `set_current_dir`; it changes the directory for every thread.

## Never change the environment from a test

`std::env::set_var` and `remove_var` are `unsafe` in edition 2024 (Rust 1.85). Other threads, including libc and std internals such as DNS lookups, read the environment without going through Rust, so on most platforms the only sound option in a multi-threaded process is not to call them. A test binary is one. Wrapping the call in `unsafe` only asserts that this holds. Nothing in a test run guarantees it.

```rust,compile_fail
// error[E0133] in edition 2024: call to unsafe function `set_var`
pub fn enable_debug_for_test() {
    std::env::set_var("APP_DEBUG", "1");
}
```

Pass configuration in instead (`test-seams`). Real code hands in `|key| std::env::var(key).ok()`:

```rust
pub fn port(get: impl Fn(&str) -> Option<String>) -> u16 {
    get("PORT").and_then(|v| v.parse().ok()).unwrap_or(8080)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn port_comes_from_the_environment_or_defaults() {
        assert_eq!(port(|k| (k == "PORT").then(|| "9000".to_owned())), 9000);
        assert_eq!(port(|_| None), 8080);
    }
}
```

To test a binary's reading of the environment, set it on the child: `Command::new(env!("CARGO_BIN_EXE_my_tool")).env("APP_DEBUG", "1")` changes only that process.

## Let the OS pick ports

Bind to port `0`, then read the port back. The listener exists before any client connects, so no retry loop or sleep is needed. Never "find a free port, close it, and reuse the number": another test can take it in between.

```rust
#[cfg(test)]
mod tests {
    use std::io::{Read, Write};
    use std::net::{TcpListener, TcpStream};

    #[test]
    fn echo_round_trip() -> std::io::Result<()> {
        let listener = TcpListener::bind("127.0.0.1:0")?;
        let addr = listener.local_addr()?;
        let server = std::thread::spawn(move || -> std::io::Result<()> {
            let (mut conn, _) = listener.accept()?;
            let mut buf = [0u8; 4];
            conn.read_exact(&mut buf)?;
            conn.write_all(&buf)
        });
        let mut client = TcpStream::connect(addr)?;
        client.write_all(b"ping")?;
        let mut reply = [0u8; 4];
        client.read_exact(&mut reply)?;
        assert_eq!(&reply, b"ping");
        server.join().unwrap()
    }
}
```

## Wait for events, not for time

`sleep(100ms)` is too short on a loaded CI machine and wasteful everywhere else. Block on the thing you wait for: `join`, a channel, a `Barrier`, a `Condvar`. A timeout is only an upper bound that turns a hang into a failure.

```rust
#[cfg(test)]
mod tests {
    use std::sync::mpsc;
    use std::time::Duration;

    #[test]
    fn worker_reports_completion() {
        let (tx, rx) = mpsc::channel();
        let worker = std::thread::spawn(move || tx.send(42).unwrap());
        // Returns the moment the message arrives; 10 s is the failure bound.
        assert_eq!(rx.recv_timeout(Duration::from_secs(10)), Ok(42));
        worker.join().unwrap();
    }
}
```

Do not assert that something took less than N milliseconds. Inject the clock instead (`test-seams`).

## Keep global state out of the code under test

A `static` counter, cache, or `OnceLock` in the code under test is shared by every test in the binary, in any order. Prefer an instance each test creates. If a true global must be exercised, give it its own `tests/<name>.rs`: a separate binary is a separate process. Seed any randomness with a fixed value and print it on failure (`meaningful-tests`).

## Avoid

- `#[serial]`-style locks or `-- --test-threads=1` to hide shared state. Use them only as a stopgap while you remove the sharing.
- Fixed names such as `/tmp/test.db`, fixed ports, and cleanup code at the end of the test body.
- `set_var`, `remove_var`, and `set_current_dir` in tests, and tests that rely on running in a particular order.
- `thread::sleep` as synchronization.

## Verify

- Run `cargo test -- --test-threads=1` and `cargo test -- --test-threads=16`. Both must pass, repeatedly.
- Optional guardrail in `clippy.toml`, if production code doesn't use them either: `disallowed-methods = [{ path = "std::env::set_var", reason = "racy" }, { path = "std::env::remove_var", reason = "racy" }, { path = "std::env::set_current_dir", reason = "process-wide" }]`.
- After a failing run, the system temp directory holds no leftover `<label>-<pid>-<n>` directories.
