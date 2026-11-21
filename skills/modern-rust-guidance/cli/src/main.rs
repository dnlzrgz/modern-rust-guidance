//! `mrg`: search and retrieve verified modern Rust guidance, and scan a crate for legacy patterns.

#![forbid(unsafe_code)]

mod blocks;
mod catalog;
mod check;
mod cli;
mod diag;
mod fence;
mod frontmatter;
mod guide;
mod json;
mod msrv;
mod retrieve;
mod scan;
mod search;
mod toml;

use std::io::Write as _;
use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match cli::run(&args) {
        Ok(output) => {
            // A closed pipe (e.g. `mrg list | head`) is not an error worth reporting.
            let _ = std::io::stdout().write_all(output.as_bytes());
            ExitCode::SUCCESS
        }
        Err(failure) => {
            eprintln!("mrg: {}", failure.message);
            ExitCode::from(failure.code)
        }
    }
}
