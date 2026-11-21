//! `mrg check --compile` / `--clippy`: compile and lint every fenced Rust block with
//! `rustc` and `clippy-driver` (authoring commands; they only run when asked).

use crate::diag::{self, Diag};
use crate::fence::{self, Fence, Lang};
use crate::guide::Guide;
use std::collections::BTreeSet;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

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

/// `<target>/mrg-check`, where `<target>` belongs to the enclosing cargo workspace.
fn work_dir() -> PathBuf {
    if let Some(target) = env::var_os("CARGO_TARGET_DIR") {
        return PathBuf::from(target).join("mrg-check");
    }
    let root = env::current_dir().ok().and_then(|cwd| {
        cwd.ancestors()
            .find(|dir| {
                fs::read_to_string(dir.join("Cargo.toml")).is_ok_and(|t| t.contains("[workspace]"))
            })
            .map(Path::to_path_buf)
    });
    root.map_or_else(
        || env::temp_dir().join("mrg-check"),
        |root| root.join("target/mrg-check"),
    )
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
pub fn check(guides: &[Guide], compile: bool, clippy: bool) -> Result<Vec<String>, String> {
    let work_dir = work_dir();
    fs::create_dir_all(&work_dir)
        .map_err(|e| format!("cannot create {}: {e}", work_dir.display()))?;
    let runner = Runner {
        work_dir,
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
            let file = runner.work_dir.join(format!("{}-{line}.rs", guide.id()));
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
