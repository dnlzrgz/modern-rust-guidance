---
name: test-seams
description: Design code so tests can control I/O, time, and failures without a mocking library, by accepting Read/Write, injected clocks and closures, and small traits at the boundary.
rust-version: "1.70"
clippy-lints: []
sources:
  - "Microsoft Pragmatic Rust Guidelines: M-IMPL-IO, M-MOCKABLE-SYSCALLS, M-TEST-UTIL (MIT)"
  - "std docs: std::io::Read, Write, BufRead, Cursor (MIT/Apache-2.0)"
  - "Comprehensive Rust, Testing chapters (CC-BY-4.0 prose, Apache-2.0 code)"
---

# Test Seams

Code that opens a file, reads the clock, or reads the environment inside the function that makes decisions can only be tested through the real thing. Move the effect to the edge: take `impl Read`/`impl Write`, take the current time or a clock, or take a closure. Tests then pass a byte slice, a `Vec<u8>`, a fake clock, or a writer that fails on purpose. No mocking crate is needed for any of this.

## Accept `Read` and `Write`, not `File` or `Stdout`

`&[u8]` is a `Read` and a `BufRead`, `Vec<u8>` is a `Write`, and `&mut W` is a `Write` whenever `W` is, so callers keep their value. Real code passes `File`, `TcpStream`, or `stdin().lock()`.

```rust
use std::io::{self, BufRead, Write};

/// Copies the non-blank lines of `input` to `output` and returns how many.
///
/// # Errors
///
/// Returns any I/O error from `input` or `output`.
pub fn keep_nonblank(input: impl BufRead, mut output: impl Write) -> io::Result<usize> {
    let mut kept = 0;
    for line in input.lines() {
        let line = line?;
        if !line.trim().is_empty() {
            writeln!(output, "{line}")?;
            kept += 1;
        }
    }
    Ok(kept)
}

#[cfg(test)]
mod tests {
    use super::*;

    // A writer that accepts `budget` bytes and then fails: the cheapest way to
    // exercise error paths that a real file almost never hits.
    struct FailAfter {
        budget: usize,
    }

    impl Write for FailAfter {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            if self.budget == 0 {
                return Err(io::Error::from(io::ErrorKind::BrokenPipe));
            }
            let n = buf.len().min(self.budget);
            self.budget -= n;
            Ok(n)
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn drops_blank_lines() {
        let mut out = Vec::new();
        let kept = keep_nonblank(&b"a\n\n  \nb\n"[..], &mut out).unwrap();
        assert_eq!(kept, 2);
        assert_eq!(out, b"a\nb\n");
    }

    #[test]
    fn write_failure_is_returned_not_swallowed() {
        let err = keep_nonblank(&b"a\nb\n"[..], FailAfter { budget: 1 }).unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::BrokenPipe);
    }
}
```

## Pass the time in

The simplest clock is a parameter, and the simplest seam is a pure function of it. `Instant` has no constructor, but `Instant::now() + Duration` makes any later instant, so a test controls time by arithmetic.

```rust
use std::time::{Duration, Instant};

/// True when at least `gap` has passed since the last action. The caller owns `last` and supplies `now`.
pub fn due(last: Option<Instant>, now: Instant, gap: Duration) -> bool {
    match last {
        None => true,
        Some(t) => now.duration_since(t) >= gap,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test] // no sleeping: instant, cannot flake
    fn due_again_after_the_gap() {
        let t0 = Instant::now();
        let gap = Duration::from_secs(10);
        assert!(due(None, t0, gap));
        assert!(!due(Some(t0), t0 + Duration::from_secs(9), gap));
        assert!(due(Some(t0), t0 + gap, gap));
    }
}
```

Production code calls `due(self.last, Instant::now(), gap)`. If time is needed deep in a call chain, store a `Box<dyn Fn() -> Instant>` or a one-method `Clock` trait with `SystemClock` and a test fake instead.

## Pass closures for small effects

Environment lookups, random numbers, and id generation fit a closure parameter. Real code hands in `|key| std::env::var(key).ok()`; a test hands in a literal, and never has to call `set_var` (see `test-isolation`):

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

Do the same for arguments: parse from `impl Iterator<Item = String>`, and call it with `std::env::args().skip(1)` in `main`. A trait is worth it only when the boundary has several operations, such as a store with `get` and `put`. Keep the trait to the operations the code uses, and write the fake as a plain struct over a `HashMap` or `Vec`.

## Split decisions from I/O (sans-IO)

For protocols and parsers, write a state machine that takes bytes and returns events (`fn feed(&mut self, chunk: &[u8]) -> Vec<Line>`), and a thin loop that reads and calls it. Tests can then feed the same input split at every byte position, which exposes boundary bugs that a real socket almost never produces.

## Avoid

- `File::open`, `SystemTime::now()`, or `env::var` inside logic that makes decisions, and `Default::default()` constructors that touch the file system or network.
- `#[cfg(test)]` branches in production code that swap in fakes: the tested code is then not the shipped code. Inject the dependency instead.
- A trait with one implementation that exists only to be mocked, around code that is already pure. Test pure functions directly.
- Fakes that live in the library without a `test-util` feature gate (see `test-organization`). Mocking crates such as `mockall` are out of scope; the seams above need none.

## Verify

- Every function that does I/O takes its source or sink as a parameter, and the real one is created only in `main` or a thin constructor.
- Each error path has a test that makes the fake fail on purpose (for example `FailAfter` above).
