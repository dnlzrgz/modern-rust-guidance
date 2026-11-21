//! Embeds `../guides/**/*.md` into the binary as an `include_str!` table.

use std::env;
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

fn main() {
    let manifest =
        PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").expect("cargo sets CARGO_MANIFEST_DIR"));
    let guides = manifest.join("../guides");
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed={}", guides.display());

    let mut files = Vec::new();
    collect(&guides, "", &mut files);
    files.sort();

    let mut table = String::from("pub static GUIDES: &[(&str, &str)] = &[\n");
    for rel in &files {
        let abs = guides.join(rel);
        let abs = abs.to_str().expect("guide paths are valid UTF-8");
        writeln!(table, "    ({rel:?}, include_str!({abs:?})),").expect("writing to a String");
    }
    table.push_str("];\n");

    let out = PathBuf::from(env::var_os("OUT_DIR").expect("cargo sets OUT_DIR"));
    fs::write(out.join("guides.rs"), table).expect("write guides.rs");
}

/// Collects `.md` files below `dir` as `/`-separated paths relative to the guides root.
fn collect(dir: &Path, prefix: &str, files: &mut Vec<String>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().into_owned();
        if path.is_dir() {
            collect(&path, &format!("{prefix}{name}/"), files);
        } else if path.extension().is_some_and(|ext| ext == "md") {
            files.push(format!("{prefix}{name}"));
        }
    }
}
