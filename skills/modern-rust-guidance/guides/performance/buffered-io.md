---
name: buffered-io
description: Speed up blocking std::io and std::net file, stdin, stdout, and socket I/O with BufReader, BufWriter, locked stdout, and whole-file reads.
rust-version: "1.70"
clippy-lints: [unbuffered_bytes, verbose_file_reads]
sources:
  - "Rust Performance Book: I/O, Heap Allocations (Reading Lines from a File) (MIT/Apache-2.0)"
  - "std docs: io::BufReader, io::BufWriter, io::copy, fs::read, io::Stdout (MIT/Apache-2.0)"
---

# Buffered I/O

`File`, `TcpStream`, and similar types are unbuffered: each `read` or `write` call is a system call, and `write!` on a `File` can make several per call. Wrap them in `BufReader` or `BufWriter` when you do many small reads or writes. For one big read or write, use the whole-file helpers instead: `BufWriter` adds nothing to a single large `write_all`.

> [!NOTE]
> This guide covers blocking `std::io` and `std::net`. Async runtimes (tokio) have their own buffering types and cancellation rules and are out of scope for now. Do not apply `BufReader`/`BufWriter` advice to async code.

If code already batches output in its own buffer (a `Vec<u8>` filled by a formatter, then one `write_all` or vectored write), it is already buffered: do not add a `BufWriter` on top. That copies every byte twice.

## Wrap writers in `BufWriter` and flush explicitly

Take a generic `W: Write` so the function works on files, sockets, and `Vec<u8>` in tests.

```rust
use std::io::{self, BufWriter, Write};

pub fn write_lines<W: Write>(out: W, lines: &[String]) -> io::Result<()> {
    let mut out = BufWriter::new(out);
    for line in lines {
        writeln!(out, "{line}")?;
    }
    out.flush() // Drop flushes too, but silently discards the error
}

#[test]
fn writes_every_line() {
    let mut buf = Vec::new();
    write_lines(&mut buf, &["a".to_string(), "b".to_string()]).unwrap();
    assert_eq!(buf, b"a\nb\n");
}
```

Dropping a `BufWriter` flushes it but ignores errors, and it does not run at all on `process::exit` or with `panic = "abort"`. Call `flush()` or `into_inner()` (which flushes and returns the inner writer) and handle the `Result`. `into_parts` (1.56) returns the writer together with any bytes still buffered.

## Lock and buffer stdout for bulk output

`println!` locks stdout on every call, and `Stdout` is line-buffered, so each newline-terminated line is flushed to the OS. `println!` also panics if the write fails, for example when piped into `head`. For many lines, lock once and buffer. `Stdout::lock` returns a `'static` guard (since 1.61), so no named `stdout` variable is needed.

```rust
use std::io::{self, BufWriter, Write};

pub fn print_all(items: &[u32]) -> io::Result<()> {
    let mut out = BufWriter::new(io::stdout().lock());
    for item in items {
        writeln!(out, "{item}")?; // an error is returned, not a panic
    }
    out.flush()
}
```

Flush before blocking on input if the output is an interactive prompt. `stdin().lock()` and `stderr().lock()` work the same way for repeated reads and writes.

`BufWriter::new` and `BufReader::new` use an 8 KiB buffer. For large outputs, `BufWriter::with_capacity(64 * 1024, out)` means fewer system calls. Measure the size with `strace -c` or a profiler rather than guessing.

## Read lines with `lines()`, and reuse a `String` only when profiled

`BufRead::lines()` is fine by default: it strips line endings and is simple. It allocates one `String` per line, so switch only when a profile shows that cost. `Stdin` is buffered internally, so use `io::stdin().lock()` and do not wrap it in a `BufReader`. Files and sockets need `BufReader`.

```rust
use std::io::{self, BufRead, BufReader, Read};

pub fn longest_line(input: impl Read) -> io::Result<usize> {
    let mut reader = BufReader::new(input);
    let mut line = String::new();
    let mut longest = 0;
    while reader.read_line(&mut line)? != 0 {
        longest = longest.max(line.trim_end().len()); // read_line keeps the line ending
        line.clear(); // read_line appends; forgetting this grows `line` forever
    }
    Ok(longest)
}
```

This works only if the loop body can use a `&str`. If you need an owned `String` per line, `lines()` is the right tool. To skip UTF-8 validation, `read_until(b'\n', &mut buf)` fills a reused `Vec<u8>`.

## Read or copy a whole stream in one call

```rust
use std::io;

pub fn load(path: &str) -> io::Result<(String, Vec<u8>)> {
    let text = std::fs::read_to_string(path)?; // sizes the buffer from file metadata
    let bytes = std::fs::read(path)?;
    Ok((text, bytes))
}

pub fn dump(mut from: impl io::Read, mut to: impl io::Write) -> io::Result<u64> {
    io::copy(&mut from, &mut to) // no hand-written read/write loop
}
```

`io::read_to_string(reader)` (1.65) does the same for any `Read`.

## Avoid

```rust,legacy
// lint: unbuffered_bytes, verbose_file_reads
use std::fs::File;
use std::io::{self, Read};

pub fn checksum(path: &str) -> io::Result<u32> {
    let file = File::open(path)?;
    let mut sum = 0u32;
    for byte in file.bytes() {
        sum = sum.wrapping_add(u32::from(byte?)); // one read call per byte
    }
    Ok(sum)
}

pub fn slurp(path: &str) -> io::Result<Vec<u8>> {
    let mut buf = Vec::new();
    let mut file = File::open(path)?;
    file.read_to_end(&mut buf)?; // std::fs::read(path)
    Ok(buf)
}
```

- Writing many small pieces to a `File` or `TcpStream` with `write!` and no `BufWriter`.
- `BufReader::new(stdin())`, double buffering, or `BufWriter` around a `BufWriter` or around a writer that is fed by your own batch buffer.

## Verify

- `cargo clippy -- -W clippy::unbuffered_bytes` (on by default). `verbose_file_reads` is a restriction lint: enable it with `-W clippy::verbose_file_reads`.
- Search for `File::create`, `File::open`, and `TcpStream` followed by loops of `write!`, `read`, or `bytes()` with no buffer wrapper.
- Test I/O code against `Vec<u8>` and `std::io::Cursor`, and assert on the bytes. For timing, use `strace -c` or the profiler to compare syscall counts.
