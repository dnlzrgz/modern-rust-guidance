//! Runs the built `mrg` binary against fixture guides.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

fn guides() -> PathBuf {
    fixtures().join("guides")
}

fn mrg(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_mrg"))
        .args(args)
        .arg("--guides-dir")
        .arg(guides())
        .env_remove("MRG_GUIDES_DIR")
        .output()
        .expect("run mrg")
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

#[test]
fn list_shows_hubs_first_and_filters_by_category() {
    let out = mrg(&["list"]);
    assert!(out.status.success());
    let text = stdout(&out);
    let ids: Vec<&str> = text
        .lines()
        .filter_map(|l| l.split_whitespace().next())
        .collect();
    assert_eq!(
        ids,
        [
            "std",
            "performance",
            "testing",
            "early-return",
            "allocation-reuse",
            "test-assertions"
        ]
    );
    assert!(text.contains("early-return  [std, rust ≥ 1.80]  Return early"));

    let only = stdout(&mrg(&["list", "--category", "testing"]));
    assert_eq!(only.lines().count(), 2);

    let json = stdout(&mrg(&["list", "--json"]));
    assert!(json.starts_with("[{\"id\":\"std\""));
}

#[test]
fn search_ranks_and_limits() {
    let text = stdout(&mrg(&["search", "reuse", "buffers", "allocation"]));
    assert!(text.starts_with("allocation-reuse  ["), "{text}");
    let text = stdout(&mrg(&["search", "let", "else", "--limit", "1"]));
    assert_eq!(text.lines().count(), 1);
    assert!(text.starts_with("early-return"));
    let none = mrg(&["search", "zzzzqqq"]);
    assert!(none.status.success());
    assert!(stdout(&none).contains("No guides matched"));
    assert!(stdout(&mrg(&["search", "manual_let_else", "--json"])).contains("\"score\":"));
}

#[test]
fn retrieve_applies_msrv_banner() {
    let new = stdout(&mrg(&["retrieve", "early-return", "--msrv", "1.100"]));
    assert!(new.starts_with("<!-- mrg: id=early-return, category=std, rust-version=1.80 -->"));
    assert!(new.contains("> Project MSRV 1.100 ≥ 1.80: use the main guidance."));
    assert!(!new.contains("## Older MSRV"));
    assert!(new.contains("## Verify"));
    assert!(!new.contains("clippy-lints:"));

    let old = stdout(&mrg(&["retrieve", "early-return", "--msrv", "1.74"]));
    assert!(old.contains("[!IMPORTANT]"));
    assert!(old.contains("## Older MSRV"));

    let both = stdout(&mrg(&[
        "retrieve",
        "std,allocation-reuse",
        "--msrv",
        "1.80",
    ]));
    assert!(both.contains("id=std,") && both.contains("id=allocation-reuse,"));
}

#[test]
fn retrieve_without_declared_msrv_keeps_older_section() {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join("no-msrv");
    std::fs::create_dir_all(&dir).unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_mrg"))
        .args(["retrieve", "early-return", "--guides-dir"])
        .arg(guides())
        .current_dir(&dir)
        .output()
        .unwrap();
    let text = stdout(&out);
    // Only meaningful when no ancestor of the temp dir declares an MSRV.
    if text.contains("No MSRV declared") {
        assert!(text.contains("## Older MSRV"));
    }
}

#[test]
fn retrieve_unknown_id_exits_2_with_suggestions() {
    let out = mrg(&["retrieve", "early-retrun"]);
    assert_eq!(out.status.code(), Some(2));
    let err = stderr(&out);
    assert!(err.contains("did you mean `early-return`"), "{err}");
    assert!(err.contains("mrg search"));
}

#[test]
fn msrv_reads_manifest_path() {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join("msrv-project");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("Cargo.toml"),
        "[package]\nname = \"p\"\nrust-version = \"1.75\"\n",
    )
    .unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_mrg"))
        .args(["msrv", "--manifest-path"])
        .arg(dir.join("Cargo.toml"))
        .output()
        .unwrap();
    assert!(out.status.success());
    assert!(stdout(&out).starts_with("MSRV 1.75 (from "));
}

#[test]
fn lints_formats() {
    assert_eq!(
        stdout(&mrg(&["lints"])),
        "manual_let_else  early-return\nneedless_range_loop  allocation-reuse\nshould_panic_without_expect  test-assertions\n"
    );
    assert_eq!(
        stdout(&mrg(&["lints", "--format", "flags"])),
        "-W clippy::manual_let_else -W clippy::needless_range_loop -W clippy::should_panic_without_expect\n"
    );
    let toml = stdout(&mrg(&["lints", "--format=toml"]));
    assert!(toml.starts_with("[lints.clippy]\nmanual_let_else = \"warn\"\n"));
    assert_eq!(mrg(&["lints", "--format", "nope"]).status.code(), Some(2));
}

#[test]
fn index_prints_catalog() {
    let text = stdout(&mrg(&["index"]));
    assert!(text.contains("## std\n"));
    assert!(text.contains("| `std` (hub) | 1.70 |"));
    assert!(text.contains("| `early-return` | 1.80 |"));
    assert!(text.contains("`manual_let_else`"));
}

#[test]
fn check_passes_on_fixtures() {
    let out = mrg(&["check"]);
    assert!(out.status.success(), "{}", stderr(&out));
    assert!(stdout(&out).starts_with("ok: 6 guides passed (structure)"));
}

#[test]
fn check_reports_structure_errors() {
    let copy = Path::new(env!("CARGO_TARGET_TMPDIR")).join("broken-guides");
    let _ = std::fs::remove_dir_all(&copy);
    for category in ["std", "performance", "testing"] {
        std::fs::create_dir_all(copy.join(category)).unwrap();
        for entry in std::fs::read_dir(guides().join(category)).unwrap() {
            let entry = entry.unwrap();
            std::fs::copy(entry.path(), copy.join(category).join(entry.file_name())).unwrap();
        }
    }
    let path = copy.join("std/early-return.md");
    let text = std::fs::read_to_string(&path)
        .unwrap()
        .replace("## Verify", "## Check")
        .replace("```rust,legacy", "```python");
    std::fs::write(&path, text).unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_mrg"))
        .args(["check", "--guides-dir"])
        .arg(&copy)
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1));
    let err = stderr(&out);
    assert!(err.contains("missing `## Verify`"), "{err}");
    assert!(err.contains("`python` is not allowed"), "{err}");
}

#[test]
fn check_compile_and_clippy_on_fixtures() {
    if !tool_ok("clippy-driver", "--version") {
        eprintln!("note: clippy-driver not available; skipping check --compile --clippy");
        return;
    }
    let out = mrg(&["check", "--compile", "--clippy"]);
    assert!(out.status.success(), "{}", stderr(&out));
    assert!(stdout(&out).contains("structure, compile, clippy"));
}

#[test]
fn check_detects_wrong_fences() {
    if !tool_ok("rustc", "--version") {
        return;
    }
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join("bad-blocks");
    let _ = std::fs::remove_dir_all(&dir);
    for category in ["std", "performance", "testing"] {
        std::fs::create_dir_all(dir.join(category)).unwrap();
        for entry in std::fs::read_dir(guides().join(category)).unwrap() {
            let entry = entry.unwrap();
            std::fs::copy(entry.path(), dir.join(category).join(entry.file_name())).unwrap();
        }
    }
    let path = dir.join("std/early-return.md");
    // A compile_fail block that compiles, and a rust block that doesn't.
    let text = std::fs::read_to_string(&path)
        .unwrap()
        .replace("pub fn gen() {}", "pub fn fine() {}")
        .replace(
            "    v + 1\n}\n```\n\n## Avoid",
            "    v + undefined_name\n}\n```\n\n## Avoid",
        );
    std::fs::write(&path, text).unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_mrg"))
        .args(["check", "--compile", "--guides-dir"])
        .arg(&dir)
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1));
    let err = stderr(&out);
    assert!(err.contains("block does not compile"), "{err}");
    assert!(err.contains("compiled successfully"), "{err}");
}

#[test]
fn check_real_guides_structure() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../guides");
    if !has_markdown(&dir) {
        eprintln!(
            "note: {} has no guides yet; skipping real-guides check",
            dir.display()
        );
        return;
    }
    let out = Command::new(env!("CARGO_BIN_EXE_mrg"))
        .args(["check", "--guides-dir"])
        .arg(&dir)
        .output()
        .unwrap();
    assert!(out.status.success(), "{}", stderr(&out));
}

fn has_markdown(dir: &Path) -> bool {
    std::fs::read_dir(dir).is_ok_and(|entries| {
        entries.flatten().any(|e| {
            let path = e.path();
            path.extension().is_some_and(|x| x == "md") || (path.is_dir() && has_markdown(&path))
        })
    })
}

fn tool_ok(program: &str, arg: &str) -> bool {
    // `clippy-driver` may only be reachable through rustup.
    let path = Command::new("rustup")
        .args(["which", program])
        .output()
        .ok()
        .filter(|o| o.status.success());
    let program = path.map_or_else(
        || program.to_owned(),
        |o| String::from_utf8_lossy(&o.stdout).trim().to_owned(),
    );
    Command::new(program)
        .arg(arg)
        .output()
        .is_ok_and(|o| o.status.success())
}

#[test]
fn scan_groups_findings_by_guide() {
    if !Command::new("cargo")
        .args(["clippy", "--version"])
        .output()
        .is_ok_and(|o| o.status.success())
    {
        eprintln!("note: `cargo clippy` not available; skipping scan test");
        return;
    }
    let target = Path::new(env!("CARGO_TARGET_TMPDIR")).join("scan-target");
    let run = |extra: &[&str]| {
        Command::new(env!("CARGO_BIN_EXE_mrg"))
            .arg("scan")
            .arg(fixtures().join("scan-crate"))
            .args(extra)
            .arg("--guides-dir")
            .arg(guides())
            .env("CARGO_TARGET_DIR", &target)
            .output()
            .unwrap()
    };
    let out = run(&[]);
    assert!(out.status.success(), "{}", stderr(&out));
    let text = stdout(&out);
    assert!(text.contains("early-return (1 finding)"), "{text}");
    assert!(
        text.contains("src/lib.rs:3  clippy::manual_let_else"),
        "{text}"
    );
    assert!(text.contains("confirm each finding before changing code"));
    assert!(stderr(&out).contains("builds the project"));
    assert!(!text.contains("allocation-reuse"));

    let json = stdout(&run(&["--json"]));
    assert!(json.starts_with("[{\"guide\":\"early-return\""), "{json}");
}

#[test]
fn scan_fails_with_exit_1_outside_a_cargo_project() {
    if !Command::new("cargo")
        .args(["clippy", "--version"])
        .output()
        .is_ok_and(|o| o.status.success())
    {
        return;
    }
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join("empty-dir");
    std::fs::create_dir_all(&dir).unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_mrg"))
        .arg("scan")
        .arg(&dir)
        .arg("--guides-dir")
        .arg(guides())
        .output()
        .unwrap();
    // Unless a parent directory happens to be a cargo project, cargo fails.
    if !out.status.success() {
        assert_eq!(out.status.code(), Some(1));
    }
}

#[test]
fn version_help_and_usage_errors() {
    let run = |args: &[&str]| {
        Command::new(env!("CARGO_BIN_EXE_mrg"))
            .args(args)
            .output()
            .unwrap()
    };
    assert!(stdout(&run(&["--version"])).starts_with("mrg 0."));
    assert!(stdout(&run(&["help"])).contains("Commands:"));
    assert!(stdout(&run(&["help", "scan"])).contains("builds the project"));
    assert_eq!(run(&["nope"]).status.code(), Some(2));
    assert_eq!(run(&["list", "--bogus"]).status.code(), Some(2));
    assert_eq!(run(&["search"]).status.code(), Some(2));
}

#[test]
fn embedded_guides_work_without_guides_dir() {
    let out = Command::new(env!("CARGO_BIN_EXE_mrg"))
        .arg("list")
        .env_remove("MRG_GUIDES_DIR")
        .output()
        .unwrap();
    assert!(out.status.success());
}

/// Copies the fixture guides to a fresh directory under the test tmp dir.
fn copy_fixtures(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = std::fs::remove_dir_all(&dir);
    for category in ["std", "performance", "testing"] {
        std::fs::create_dir_all(dir.join(category)).unwrap();
        for entry in std::fs::read_dir(guides().join(category)).unwrap() {
            let entry = entry.unwrap();
            std::fs::copy(entry.path(), dir.join(category).join(entry.file_name())).unwrap();
        }
    }
    dir
}

fn check_in(dir: &Path, extra: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_mrg"))
        .arg("check")
        .args(extra)
        .arg("--guides-dir")
        .arg(dir)
        .env_remove("MRG_GUIDES_DIR")
        .output()
        .unwrap()
}

#[test]
fn check_reports_version_mentions_and_unknown_see_targets() {
    let dir = copy_fixtures("bad-prose");
    let path = dir.join("std/early-return.md");
    let text = std::fs::read_to_string(&path).unwrap().replace(
        "## Avoid",
        "Uses `x` (1.99) and see `no-such-guide`.\n\nOptional `y` (needs 1.98) and see also `std`.\n\n## Avoid",
    );
    std::fs::write(&path, text).unwrap();
    let out = check_in(&dir, &[]);
    assert_eq!(out.status.code(), Some(1));
    let err = stderr(&out);
    assert!(
        err.contains("mentions Rust 1.99 but rust-version is 1.80 (mark it 'needs 1.99' if optional, or raise rust-version)"),
        "{err}"
    );
    assert!(
        err.contains("`see `no-such-guide`` does not name an existing guide"),
        "{err}"
    );
    assert!(!err.contains("1.98"), "{err}");
    assert!(!err.contains("`std``"), "{err}");
    assert!(err.contains("2 problems found"), "{err}");
}

#[test]
fn check_skips_block_checks_for_guides_with_structure_errors() {
    if !tool_ok("rustc", "--version") {
        return;
    }
    let dir = copy_fixtures("skipped-blocks");
    let path = dir.join("std/early-return.md");
    // A broken rust block, plus a structure error (no `## Verify`).
    let text = std::fs::read_to_string(&path)
        .unwrap()
        .replace("## Verify", "## Check")
        .replace(
            "    v + 1\n}\n```\n\n## Avoid",
            "    v + undefined_name\n}\n```\n\n## Avoid",
        );
    std::fs::write(&path, text).unwrap();
    let out = check_in(&dir, &["--compile"]);
    assert_eq!(out.status.code(), Some(1));
    let err = stderr(&out);
    assert!(err.contains("missing `## Verify`"), "{err}");
    assert!(
        err.contains("skipped compile/clippy checks for std/early-return.md"),
        "{err}"
    );
    assert!(!err.contains("block does not compile"), "{err}");
}

#[test]
fn check_compile_leaves_no_temp_dir_behind() {
    if !tool_ok("rustc", "--version") {
        return;
    }
    // Private TMPDIR so other tests' work dirs can't interfere; the child's pid identifies
    // its work dir in the repo's shared `target/mrg-check`.
    let tmp = Path::new(env!("CARGO_TARGET_TMPDIR")).join("leak-check-tmp");
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(&tmp).unwrap();
    let child = Command::new(env!("CARGO_BIN_EXE_mrg"))
        .args(["check", "--compile", "--guides-dir"])
        .arg(guides())
        .env_remove("MRG_GUIDES_DIR")
        .env("TMPDIR", &tmp)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .expect("run mrg");
    let pid = child.id();
    let out = child.wait_with_output().unwrap();
    assert!(out.status.success(), "{}", stderr(&out));
    let leaked_tmp: Vec<_> = std::fs::read_dir(&tmp).unwrap().flatten().collect();
    assert!(leaked_tmp.is_empty(), "{leaked_tmp:?}");
    let repo_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../target/mrg-check");
    let prefix = format!("{pid}-");
    let leaked_repo: Vec<_> = std::fs::read_dir(&repo_dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| e.file_name().to_string_lossy().starts_with(&prefix))
        .collect();
    assert!(leaked_repo.is_empty(), "{leaked_repo:?}");
}

#[test]
fn msrv_prefers_rust_version_and_prints_a_differing_clippy_msrv() {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join("msrv-both");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("Cargo.toml"),
        "[package]\nname = \"p\"\nrust-version = \"1.80\"\n",
    )
    .unwrap();
    std::fs::write(dir.join("clippy.toml"), "msrv = \"1.70\"\n").unwrap();
    std::fs::write(dir.join(".clippy.toml"), "msrv = \"1.60\"\n").unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_mrg"))
        .args(["msrv", "--manifest-path"])
        .arg(dir.join("Cargo.toml"))
        .output()
        .unwrap();
    let text = stdout(&out);
    assert!(text.starts_with("MSRV 1.80 (from "), "{text}");
    assert!(text.contains("clippy msrv 1.70 (from "), "{text}");
    assert!(text.contains("differs; mrg uses `rust-version`"), "{text}");
    assert!(!text.contains("1.60"), "{text}");
}
