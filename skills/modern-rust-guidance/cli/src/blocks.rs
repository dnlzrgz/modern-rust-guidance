//! `mrg check --compile` / `--clippy`: compile and lint every fenced Rust block with
//! `rustc` and `clippy-driver` (authoring commands; they only run when asked).

use crate::diag::{self, Diag};
use crate::fence::{self, Fence, Lang};
use crate::guide::Guide;
use crate::toml::Toml;
use std::collections::BTreeSet;
use std::env;
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

const SHOWN_LINES: usize = 8;

/// A compiler invocation result.
struct Outcome {
    success: bool,
    diags: Vec<Diag>,
    stderr: String,
}

impl Outcome {
    fn has_warning(&self) -> bool {
        self.diags.iter().any(|d| d.level == "warning")
    }

    /// The first lines of the compiler's rendered output.
    fn excerpt(&self) -> String {
        let rendered: Vec<&str> = self
            .diags
            .iter()
            .filter(|d| d.is_warning_or_error())
            .flat_map(|d| d.rendered.lines())
            .collect();
        let lines = if rendered.is_empty() {
            self.stderr.lines().collect()
        } else {
            rendered
        };
        lines
            .iter()
            .take(SHOWN_LINES)
            .map(|l| format!("    | {l}"))
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn fired(&self, lint: &str) -> bool {
        self.diags
            .iter()
            .any(|d| d.code.as_deref().and_then(|c| c.strip_prefix("clippy::")) == Some(lint))
    }
}

struct Runner {
    work_dir: PathBuf,
    clippy_driver: String,
}

impl Runner {
    fn invoke(
        &self,
        program: &str,
        file: &Path,
        test: bool,
        lints: &[String],
    ) -> Result<Outcome, String> {
        let mut command = Command::new(program);
        command
            .args([
                "--edition",
                "2024",
                "--crate-type",
                "lib",
                "--crate-name",
                "mrg_block",
            ])
            .args([
                "-A",
                "dead_code",
                "-A",
                "unused",
                "--emit=metadata",
                "--error-format=json",
            ])
            .arg("--out-dir")
            .arg(&self.work_dir);
        if test {
            command.arg("--test");
        }
        for lint in lints {
            command.arg("-W").arg(format!("clippy::{lint}"));
        }
        let output = command
            .arg(file)
            .output()
            .map_err(|e| format!("cannot run `{program}`: {e}"))?;
        let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
        Ok(Outcome {
            success: output.status.success(),
            diags: diag::from_json_lines(&stderr),
            stderr,
        })
    }
}

/// The directory blocks are written to: a fresh per-run directory, removed on drop.
struct WorkDir {
    path: PathBuf,
}

impl Drop for WorkDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

/// The guides repo containing `guides_dir`: an ancestor of it that holds
/// `skills/modern-rust-guidance/guides` and a root `Cargo.toml` with a `[workspace]` table.
fn guides_repo(guides_dir: &Path) -> Option<PathBuf> {
    let dir = fs::canonicalize(guides_dir).ok()?;
    dir.ancestors()
        .find(|ancestor| {
            ancestor.join("skills/modern-rust-guidance/guides").is_dir()
                && fs::read_to_string(ancestor.join("Cargo.toml"))
                    .is_ok_and(|text| Toml::parse(&text).has_section("workspace"))
        })
        .map(Path::to_path_buf)
}

/// Creates a new directory named by `name(attempt)` under `parent`. The path is never
/// followed if it already exists: `create_dir` fails and the next name is tried.
fn create_fresh_dir(parent: &Path, mut name: impl FnMut(u32) -> String) -> Result<PathBuf, String> {
    const ATTEMPTS: u32 = 5;
    let mut last = None;
    for attempt in 0..ATTEMPTS {
        let path = parent.join(name(attempt));
        match fs::create_dir(&path) {
            Ok(()) => return Ok(path),
            Err(e) if e.kind() == ErrorKind::AlreadyExists => last = Some(e),
            Err(e) => return Err(format!("cannot create {}: {e}", path.display())),
        }
    }
    Err(format!(
        "cannot create a fresh directory in {}: {}",
        parent.display(),
        last.map(|e| e.to_string()).unwrap_or_default()
    ))
}

/// A fresh `<repo>/target/mrg-check/<pid>-<nanos>` when checking guides inside the guides
/// repo (so concurrent runs cannot clobber each other), else a fresh
/// `temp_dir()/mrg-check-<pid>-<nanos>`. Either is removed afterwards.
fn work_dir(guides_dir: Option<&Path>) -> Result<WorkDir, String> {
    let pid = std::process::id();
    let nanos = |attempt: u32| {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos())
            + u128::from(attempt)
    };
    let path = if let Some(repo) = guides_dir.and_then(guides_repo) {
        let base = repo.join("target/mrg-check");
        fs::create_dir_all(&base).map_err(|e| format!("cannot create {}: {e}", base.display()))?;
        // `create_dir_all` follows symlinks; refuse a `target` or `target/mrg-check` that is one.
        for dir in [repo.join("target"), base.clone()] {
            let is_real_dir = fs::symlink_metadata(&dir).is_ok_and(|m| m.file_type().is_dir());
            if !is_real_dir {
                return Err(format!(
                    "{} is not a real directory (symlink?)",
                    dir.display()
                ));
            }
        }
        create_fresh_dir(&base, |attempt| format!("{pid}-{}", nanos(attempt)))?
    } else {
        create_fresh_dir(&env::temp_dir(), |attempt| {
            format!("mrg-check-{pid}-{}", nanos(attempt))
        })?
    };
    Ok(WorkDir { path })
}

/// A file-name-safe form of a guide's file stem (never the frontmatter `name`).
fn sanitize(stem: &str) -> String {
    stem.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

fn find_clippy_driver() -> String {
    Command::new("rustup")
        .args(["which", "clippy-driver"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_owned())
        .filter(|path| !path.is_empty())
        .unwrap_or_else(|| "clippy-driver".to_owned())
}

/// Compiles and/or lints every block; returns one message per problem.
/// `guides_dir` is the on-disk guides directory in use, if any; it decides where blocks are written.
pub fn check(
    guides: &[&Guide],
    compile: bool,
    clippy: bool,
    guides_dir: Option<&Path>,
) -> Result<Vec<String>, String> {
    let work = work_dir(guides_dir)?;
    let runner = Runner {
        work_dir: work.path.clone(),
        clippy_driver: find_clippy_driver(),
    };
    let mut errors = Vec::new();
    for guide in guides {
        let mut unknown_lints = BTreeSet::new();
        for fence in fence::fences(&guide.body) {
            let Some(lang @ (Lang::Rust | Lang::Legacy | Lang::CompileFail)) = fence.lang() else {
                continue;
            };
            let line = guide.body_line + fence.line;
            let at = format!("{}:{line}", guide.path);
            let file = runner
                .work_dir
                .join(format!("{}-{line}.rs", sanitize(&guide.stem)));
            fs::write(&file, &fence.code)
                .map_err(|e| format!("cannot write {}: {e}", file.display()))?;
            let test = fence.code.contains("#[test]") || fence.code.contains("#[cfg(test)]");
            if compile {
                let outcome = runner.invoke("rustc", &file, test, &[])?;
                errors.extend(compile_error(&at, lang, &outcome));
            }
            if clippy && matches!(lang, Lang::Rust | Lang::Legacy) {
                let mut lints = guide.front.clippy_lints.clone();
                let markers = fence.lint_markers();
                for marker in &markers {
                    if !lints.contains(marker) {
                        lints.push(marker.clone());
                    }
                }
                let outcome = runner.invoke(&runner.clippy_driver, &file, test, &lints)?;
                unknown_lints.extend(unknown_lint_messages(&outcome));
                errors.extend(clippy_errors(&at, guide, &fence, &markers, &outcome));
            }
        }
        errors.extend(
            unknown_lints
                .into_iter()
                .map(|m| format!("{}: {m} (check `clippy-lints`)", guide.path)),
        );
    }
    Ok(errors)
}

fn compile_error(at: &str, lang: Lang, outcome: &Outcome) -> Option<String> {
    let problem = match lang {
        Lang::CompileFail if outcome.success => {
            "block tagged `compile_fail` compiled successfully".to_owned()
        }
        Lang::Rust if !outcome.success => "block does not compile".to_owned(),
        Lang::Rust if outcome.has_warning() => "block compiles with warnings".to_owned(),
        Lang::Legacy if !outcome.success => "legacy block does not compile".to_owned(),
        _ => return None,
    };
    let excerpt = outcome.excerpt();
    Some(if excerpt.is_empty() {
        format!("{at}: {problem}")
    } else {
        format!("{at}: {problem}\n{excerpt}")
    })
}

fn unknown_lint_messages(outcome: &Outcome) -> Vec<String> {
    outcome
        .diags
        .iter()
        .filter(|d| d.message.starts_with("unknown lint"))
        .map(|d| d.message.clone())
        .collect()
}

fn clippy_errors(
    at: &str,
    guide: &Guide,
    fence: &Fence,
    markers: &[String],
    outcome: &Outcome,
) -> Vec<String> {
    if !outcome.success {
        return vec![format!(
            "{at}: block does not compile under clippy\n{}",
            outcome.excerpt()
        )];
    }
    match fence.lang() {
        Some(Lang::Rust) => guide
            .front
            .clippy_lints
            .iter()
            .filter(|lint| outcome.fired(lint))
            .map(|lint| format!("{at}: recommended code triggers clippy::{lint}"))
            .collect(),
        Some(Lang::Legacy) => markers
            .iter()
            .filter(|lint| !outcome.fired(lint))
            .map(|lint| format!("{at}: legacy code should trigger clippy::{lint}, but it did not"))
            .collect(),
        _ => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    fn tempdir(tag: &str) -> PathBuf {
        static COUNTER: AtomicUsize = AtomicUsize::new(0);
        let dir = env::temp_dir().join(format!(
            "mrg-blocks-{}-{tag}-{}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&dir).unwrap();
        fs::canonicalize(dir).unwrap()
    }

    fn fake_repo(manifest: &str) -> PathBuf {
        let repo = tempdir("repo");
        fs::write(repo.join("Cargo.toml"), manifest).unwrap();
        fs::create_dir_all(repo.join("skills/modern-rust-guidance/guides/std")).unwrap();
        repo
    }

    #[test]
    fn sanitizes_stems_for_file_names() {
        assert_eq!(sanitize("lazy-initialization"), "lazy-initialization");
        assert_eq!(sanitize("../../x"), "______x");
        assert_eq!(sanitize("a b/c.d"), "a_b_c_d");
    }

    #[test]
    fn block_files_use_the_stem_not_the_frontmatter_name() {
        if Command::new("rustc").arg("--version").output().is_err() {
            return;
        }
        let mut src = crate::guide::tests::source(
            "std/a-guide.md",
            "1.70",
            "# A\n\n```rust\npub fn f() {}\n```\n",
        );
        src.text = src.text.replace("name: a-guide", "name: ../../escape");
        let guide = Guide::parse(&src).unwrap();
        let repo = fake_repo("[workspace]\nmembers = []\n");
        let dir = repo.join("skills/modern-rust-guidance/guides");
        let errors = check(&[&guide], true, false, Some(&dir)).unwrap();
        assert_eq!(errors, Vec::<String>::new());
        // Nothing escaped, and the per-run directory is gone again.
        assert!(!repo.join("target/escape").exists());
        assert!(!repo.parent().unwrap().join("escape").exists());
        assert_eq!(
            fs::read_dir(repo.join("target/mrg-check")).unwrap().count(),
            0
        );
        fs::remove_dir_all(repo).unwrap();
    }

    #[test]
    fn uses_a_fresh_dir_inside_the_repo_target_only_for_a_real_workspace_root() {
        let repo = fake_repo("[workspace]\nmembers = []\n");
        let guides = repo.join("skills/modern-rust-guidance/guides");
        assert_eq!(guides_repo(&guides), Some(repo.clone()));
        assert_eq!(guides_repo(&guides.join("std")), Some(repo.clone()));
        let work = work_dir(Some(&guides)).unwrap();
        assert_eq!(work.path.parent().unwrap(), repo.join("target/mrg-check"));
        let path = work.path.clone();
        assert!(path.is_dir());
        drop(work);
        assert!(!path.exists());
        assert!(repo.join("target/mrg-check").is_dir());
        fs::remove_dir_all(repo).unwrap();
    }

    #[test]
    fn workspace_in_a_comment_or_string_does_not_count() {
        for manifest in [
            "# [workspace]\n[package]\nname = \"x\"\n",
            "[package]\nname = \"x\"\ndescription = \"\"\"\n[workspace]\n\"\"\"\n",
        ] {
            let repo = fake_repo(manifest);
            let guides = repo.join("skills/modern-rust-guidance/guides");
            assert_eq!(guides_repo(&guides), None, "{manifest}");
            fs::remove_dir_all(repo).unwrap();
        }
        // No Cargo.toml at all, and no guides directory.
        assert_eq!(guides_repo(&tempdir("plain")), None);
    }

    #[test]
    fn falls_back_to_a_fresh_removed_temp_dir() {
        let guides = tempdir("outside");
        let path = {
            let work = work_dir(Some(&guides)).unwrap();
            assert!(work.path.is_dir());
            assert_eq!(work.path.parent().unwrap(), env::temp_dir());
            let name = work
                .path
                .file_name()
                .unwrap()
                .to_string_lossy()
                .into_owned();
            assert!(
                name.starts_with(&format!("mrg-check-{}-", std::process::id())),
                "{name}"
            );
            work.path.clone()
        };
        assert!(!path.exists());
        let embedded = work_dir(None).unwrap();
        assert_eq!(embedded.path.parent().unwrap(), env::temp_dir());
    }

    #[test]
    fn fresh_dir_never_reuses_an_existing_path() {
        let parent = tempdir("fresh");
        fs::create_dir(parent.join("taken")).unwrap();
        fs::write(parent.join("file"), "x").unwrap();
        // A pre-existing directory or file is skipped, not followed.
        let names = ["taken", "file", "free"];
        let made = create_fresh_dir(&parent, |i| names[i as usize].to_owned()).unwrap();
        assert_eq!(made, parent.join("free"));
        // A symlink planted at the name is not followed either.
        #[cfg(unix)]
        {
            let target = tempdir("link-target");
            std::os::unix::fs::symlink(&target, parent.join("planted")).unwrap();
            let names = ["planted", "after"];
            let made = create_fresh_dir(&parent, |i| names[i as usize].to_owned()).unwrap();
            assert_eq!(made, parent.join("after"));
            assert_eq!(fs::read_dir(&target).unwrap().count(), 0);
        }
        let err = create_fresh_dir(&parent, |_| "taken".to_owned()).unwrap_err();
        assert!(err.contains("fresh directory"), "{err}");
        fs::remove_dir_all(parent).unwrap();
    }
}
