---
name: error-values
description: Define, convert, and report errors with std only: custom error types, From for the ? operator, io::Error::other, Box<dyn Error> in applications, and when to panic.
rust-version: "1.74"
clippy-lints: [io_other_error, map_err_ignore]
sources:
  - "std docs: std::error::Error, std::io::Error, std::process::Termination (MIT/Apache-2.0)"
  - "Rust API Guidelines, C-GOOD-ERR (MIT/Apache-2.0)"
  - "Microsoft Pragmatic Rust Guidelines: M-ERRORS-CANONICAL-STRUCTS, M-PANIC-ON-BUG, M-APP-ERROR (MIT)"
  - "Clippy lint docs: io_other_error, map_err_ignore, unwrap_used (MIT/Apache-2.0)"
---

# Error Values

A library returns one concrete error type that implements `Display` and `Error`, keeps the underlying cause reachable through `source()`, and converts with `From` so `?` works. An application can return `Box<dyn Error + Send + Sync>` from `main`. A panic is for bugs, never for bad input. For `?` itself and early exits, see `early-return-patterns`.

> [!NOTE]
> `thiserror` (libraries) and `anyhow` (applications) automate the boilerplate below. They are crates and out of scope here; the std approach is complete and has no dependencies.

## Write a concrete error type for libraries

```rust
use std::{error::Error, fmt, io, num::ParseIntError};

#[derive(Debug)]
#[non_exhaustive] // adding a variant later is not a breaking change
pub enum ConfigError {
    Io(io::Error),
    Parse { key: &'static str, source: ParseIntError },
    Missing(&'static str),
}

impl fmt::Display for ConfigError {
    // Lowercase, no trailing period. Do not print the source here (see below).
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(_) => f.write_str("could not read config"),
            Self::Parse { key, .. } => write!(f, "invalid value for {key}"),
            Self::Missing(key) => write!(f, "missing key {key}"),
        }
    }
}

impl Error for ConfigError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Io(e) => Some(e),
            Self::Parse { source, .. } => Some(source),
            Self::Missing(_) => None,
        }
    }
}

impl From<io::Error> for ConfigError {
    fn from(e: io::Error) -> Self {
        Self::Io(e)
    }
}

pub fn load_port(path: &str) -> Result<u16, ConfigError> {
    let text = std::fs::read_to_string(path)?; // io::Error -> ConfigError via From
    let raw = text.lines().next().ok_or(ConfigError::Missing("port"))?;
    raw.trim().parse().map_err(|source| ConfigError::Parse { key: "port", source }) // keep the cause, never `|_|`
}
```

Rules that models skip:

- An error is either reported through `source()` **or** included in its own `Display`, not both. Otherwise a reporter that walks the chain prints every cause twice.
- Derive `Debug`, implement `Display` and `Error`, and keep the type `Send + Sync + 'static` so it fits `Box<dyn Error + Send + Sync>`.
- Do not override the deprecated `Error::description` or `cause`, and do not expose `Box<dyn Error>` from a library's public API: callers cannot match on it.
- `core::error::Error` (1.81) is the same trait as `std::error::Error`; it matters for `no_std` crates.

## Report errors in applications

`main` can return `Result`. On `Err` the process prints `Error: {err:?}` (the `Debug` form) to stderr and exits with code 1. For a friendlier message, walk the chain yourself and return `ExitCode`.

```rust
use std::error::Error;
use std::process::ExitCode;

type BoxError = Box<dyn Error + Send + Sync>;

fn run() -> Result<(), BoxError> {
    let port: u16 = "80".parse()?; // any error converts via `?`
    if port == 0 {
        return Err("port must be non-zero".into()); // &str converts too
    }
    Ok(())
}

pub fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            // Print each cause on its own line.
            for cause in std::iter::successors(e.source(), |&c| c.source()) {
                eprintln!("  caused by: {cause}");
            }
            ExitCode::FAILURE
        }
    }
}
```

## Wrap foreign errors with `io::Error::other` (1.74)

For an `io::Error` that does not come from the OS, `io::Error::other(e)` replaces `io::Error::new(io::ErrorKind::Other, e)`. Use a specific `ErrorKind` (`InvalidData`, `NotFound`, …) when one fits.

```rust
use std::io;

pub fn check(len: usize) -> io::Result<()> {
    if len > 1024 {
        return Err(io::Error::other(format!("frame of {len} bytes is too large")));
    }
    Ok(())
}
```

## Panic only for bugs

`unwrap()` and `expect()` on input, I/O, or parse results turn a recoverable condition into a crash: return `Result`. Reserve panics for violated invariants, with `expect("<why this cannot fail>")`. Panics are fine in tests.

## Avoid

```rust,legacy
// lint: io_other_error, map_err_ignore
use std::io;

pub fn a() -> io::Error {
    io::Error::new(io::ErrorKind::Other, "boom")
}

pub fn b(s: &str) -> Result<u8, String> {
    s.parse::<u8>().map_err(|_| String::from("bad number")) // drops the ParseIntError
}
```

Also avoid `Result<T, String>` and `Result<T, ()>` in public APIs (no `source()`, nothing to match on), and `.ok()` or `let _ =` on a `Result` you should handle.

## Older MSRV

- **Below 1.74:** use `io::Error::new(io::ErrorKind::Other, e)`. `io_other_error` only fires when the MSRV is 1.74 or newer.
- **Below 1.81:** `Error` exists only as `std::error::Error`, so `no_std` crates cannot implement it.

## Verify

- `cargo clippy -- -W clippy::io_other_error -W clippy::map_err_ignore`. `map_err_ignore` is in the `restriction` group.
- Opt in per crate with `#![warn(clippy::unwrap_used, clippy::expect_used)]` (also `restriction`) to find panics on fallible paths. Set `allow-unwrap-in-tests = true` and `allow-expect-in-tests = true` in `clippy.toml`.
- Add a test that walks `source()` from a top-level error and asserts the chain, and one that checks `Display` text has no trailing punctuation.
