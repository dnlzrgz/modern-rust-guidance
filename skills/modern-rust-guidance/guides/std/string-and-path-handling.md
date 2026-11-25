---
name: string-and-path-handling
description: Parse, trim, split, slice, and build strings and file paths with the right std methods instead of manual indexing, string concatenation, and lossy conversions.
rust-version: "1.91"
clippy-lints: [manual_split_once, manual_strip, manual_pattern_char_comparison, manual_is_ascii_check]
sources:
  - "std docs: str, String, char, std::path::Path, std::ffi::OsStr (MIT/Apache-2.0)"
  - "Microsoft Pragmatic Rust Guidelines: M-IMPL-ASREF (MIT)"
  - "Clippy lint docs: manual_split_once, manual_strip, manual_pattern_char_comparison, manual_is_ascii_check, uninlined_format_args (MIT/Apache-2.0)"
---

# String and Path Handling

Use the parsing method that returns an `Option` instead of computing byte offsets, and use `Path` methods instead of building paths with `format!`. Newer APIs worth knowing: `trim_ascii` (1.80), `split_at_checked` (1.80), `std::path::absolute` (1.79), `fs::exists` (1.81), `OsStr::display` (1.87), `floor_char_boundary` and `with_added_extension` (1.91). Older toolchains: see the Older MSRV section.

## Parse with `Option`-returning methods

`split_once` and `rsplit_once` (1.52), `strip_prefix` and `strip_suffix` (1.45) replace `splitn(2, …)`, `find` plus slicing, and `starts_with` plus `[n..]`. Patterns can be a `char`, `&str`, a `[char; N]` array, or a closure.

```rust
pub fn parse_kv(line: &str) -> Option<(&str, &str)> {
    let (key, value) = line.split_once('=')?;
    Some((key.trim(), value.trim()))
}

pub fn version(tag: &str) -> &str {
    tag.strip_prefix('v').unwrap_or(tag) // no `&tag[1..]` index math
}

pub fn split_separators(s: &str) -> Vec<&str> {
    s.split(['/', '\\']).collect() // char array, not |c| c == '/' || c == '\\'
}

pub fn is_hex(c: char) -> bool {
    c.is_ascii_hexdigit() // not matches!(c, '0'..='9' | 'a'..='f' | 'A'..='F')
}
```

`str::trim_prefix` and `trim_suffix` (needs 1.100) strip only if present and return the `&str` directly; `strip_prefix(p).unwrap_or(s)` does the same everywhere.

## Slice by character boundary

`&s[..n]` panics when `n` falls inside a multi-byte character. Use `s.floor_char_boundary(n)` (1.91) to truncate safely, `split_at_checked` (1.80) when you need both halves, or `char_indices()` when iterating positions. `len()` counts bytes, and `chars().count()` counts scalar values.

```rust
pub fn truncate(s: &str, max_bytes: usize) -> &str {
    &s[..s.floor_char_boundary(max_bytes)] // 1.91
}
```

## Return `Cow<str>` when you only sometimes allocate

For a function that rewrites a string only on some inputs (escaping, normalizing), return `Cow::Borrowed(s)` on the common path and `Cow::Owned(..)` after the change, so most callers allocate nothing. See `needless-copies` for more on `Cow`.

## Choose parameter types by what you do with them

Take `&str` (not `&String`) and `&Path` (not `&PathBuf`). Plain `&str` parameters are fine as they are. Use `impl AsRef<Path>` only for public functions that open or inspect files, so callers can pass `&str`, `String`, `PathBuf`, or `&Path`. Do not put `AsRef` bounds on struct fields: store `String` or `PathBuf`.

## Build and inspect paths with `Path`

`join` adds a component with the right separator. `with_extension` **replaces** the extension, so `a.tar` becomes `a.gz` and `v1.2` becomes `v1.toml`. Use `with_added_extension` (1.91) to append. `Path::starts_with` compares whole components, not characters. `display()` prints a path.

```rust
use std::path::{Path, PathBuf};

pub fn config_path(root: &Path, name: &str) -> PathBuf {
    // `name` may contain a dot ("v1.2"): `with_extension` would turn it into "v1.toml".
    root.join("config").join(name).with_added_extension("toml") // "v1.2" -> "v1.2.toml"
}

pub fn gzip_name(p: &Path) -> PathBuf {
    p.with_added_extension("gz") // "a.tar" -> "a.tar.gz" (1.91)
}

pub fn is_rust_source(p: &Path) -> bool {
    p.extension().is_some_and(|e| e.eq_ignore_ascii_case("rs")) // also matches "RS"
}

pub fn describe(p: &Path) -> String {
    format!("opened {}", p.display())
}
```

## Resolve and test paths

- `std::path::absolute(p)` (1.79) makes a path absolute without touching the filesystem: no symlink resolution, and `..` may remain on Unix. `fs::canonicalize` resolves symlinks and `..`, but needs the path to exist and returns `\\?\` verbatim paths on Windows, which many tools reject. Prefer `absolute` unless you need symlinks resolved.
- `fs::exists(p)?` (1.81) and `Path::try_exists` return `io::Result<bool>`. `Path::exists` and `is_file` fold every error (permission denied, I/O failure) into `false`, so use them only when "not accessible" and "absent" mean the same thing.

> [!WARNING]
> `base.join(user_input)` **discards `base`** if `user_input` is absolute, and `..` components still escape it. Validate untrusted names (single `Component::Normal`) before joining.

## Avoid

```rust,legacy
// lint: manual_split_once, manual_strip, manual_pattern_char_comparison, manual_is_ascii_check
pub fn a(s: &str) -> Option<(&str, &str)> {
    let mut it = s.splitn(2, '=');
    let k = it.next()?;
    let v = it.next()?;
    Some((k, v))
}
pub fn b(s: &str) -> &str {
    if s.starts_with("v=") { &s[2..] } else { s }
}
pub fn c(s: &str) -> Vec<&str> {
    s.split(|c| c == '/' || c == '\\').collect()
}
pub fn d(c: char) -> bool {
    matches!(c, '0'..='9')
}
pub fn e(base: &str, name: &str) -> String {
    format!("{}/{}", base, name) // use Path::join for paths
}
```

Also avoid `path.to_str().unwrap()` (non-UTF-8 paths panic; keep `&Path` and print with `display()`), and `String::from_utf8_lossy` on data you must preserve (it replaces invalid bytes with U+FFFD; use it only for display). Use `String::from_utf8(vec)` to reuse the buffer and `str::from_utf8(bytes)` (inherent since 1.87, or `std::str::from_utf8` before).

## Older MSRV

- **Below 1.91:** no `floor_char_boundary` (use `(0..=max).rev().find(|&i| s.is_char_boundary(i))`), no `with_added_extension` (`let mut s = p.as_os_str().to_owned(); s.push(".gz"); PathBuf::from(s)`).
- **Below 1.87:** `str::from_utf8` is only `std::str::from_utf8`, and `OsStr::display` is missing (use `to_string_lossy()`).
- **Below 1.81 / 1.80 / 1.79:** no `fs::exists` (use `Path::try_exists`), no `trim_ascii` or `split_at_checked`, no `path::absolute` (join onto `env::current_dir()?`).
- `split_once` and `strip_prefix` are old enough to need no fallback.

## Verify

- `cargo clippy` flags all of these lints by default. For a hand-run style pass, `-W clippy::uninlined_format_args` (pedantic) lists `format!("{}", x)` calls that could be `{x}`; it is noisy on existing code.
- Test non-ASCII input (`"é"`, `"日本"`), an empty string, a missing delimiter, and a path with no extension.
