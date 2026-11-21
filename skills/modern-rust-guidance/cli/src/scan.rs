//! `mrg scan`: run `cargo clippy` with the guides' lints enabled and map findings back to guides.

use crate::diag::Diag;
use crate::guide::Guide;
use crate::json::{self, Value};
use std::fmt::Write as _;
use std::path::Path;
use std::process::Command;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Finding {
    pub lint: String,
    pub file: String,
    pub line: u64,
    pub message: String,
}

/// Findings that belong to one guide.
pub struct Group<'a> {
    pub guide: &'a Guide,
    pub findings: Vec<&'a Finding>,
}

/// Runs clippy in `dir` and returns the deduplicated findings for `lints`.
pub fn run(dir: &Path, all_targets: bool, lints: &[String]) -> Result<Vec<Finding>, String> {
    let mut command = Command::new("cargo");
    command.args(["clippy", "--message-format=json", "--quiet"]);
    if all_targets {
        command.arg("--all-targets");
    }
    command.arg("--").args(
        lints
            .iter()
            .flat_map(|lint| ["-W".to_owned(), format!("clippy::{lint}")]),
    );
    let output = command
        .current_dir(dir)
        .output()
        .map_err(|e| format!("cannot run `cargo clippy`: {e}"))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!(
            "`cargo clippy` failed in {} ({}):\n{}",
            dir.display(),
            output.status,
            stderr.trim_end()
        ));
    }
    Ok(parse_findings(
        &String::from_utf8_lossy(&output.stdout),
        lints,
    ))
}

/// Keeps `compiler-message` lines whose `code.code` is one of `lints`.
pub fn parse_findings(stdout: &str, lints: &[String]) -> Vec<Finding> {
    let mut findings: Vec<Finding> = stdout
        .lines()
        .filter_map(|line| json::parse(line).ok())
        .filter(|v| v.get("reason").and_then(Value::as_str) == Some("compiler-message"))
        .filter_map(|v| Diag::from_value(v.get("message")?))
        .filter_map(|diag| {
            let lint = diag.code.as_deref()?.strip_prefix("clippy::")?;
            if !lints.iter().any(|l| l == lint) {
                return None;
            }
            let (file, line_no) = diag.location?;
            Some(Finding {
                lint: lint.to_owned(),
                file,
                line: line_no,
                message: diag.message,
            })
        })
        .collect();
    findings.sort();
    findings.dedup();
    findings
}

/// Groups findings by the guides listing their lint (a lint may belong to several guides).
pub fn group<'a>(guides: &[&'a Guide], findings: &'a [Finding]) -> Vec<Group<'a>> {
    guides
        .iter()
        .filter_map(|&guide| {
            let found: Vec<&Finding> = findings
                .iter()
                .filter(|f| guide.front.clippy_lints.contains(&f.lint))
                .collect();
            (!found.is_empty()).then_some(Group {
                guide,
                findings: found,
            })
        })
        .collect()
}

pub fn render(groups: &[Group<'_>], lint_count: usize) -> String {
    if groups.is_empty() {
        return format!("No findings for {lint_count} guide lints.\n");
    }
    let mut out = String::new();
    for group in groups {
        let n = group.findings.len();
        let _ = writeln!(
            out,
            "{} ({n} finding{})",
            group.guide.id(),
            if n == 1 { "" } else { "s" }
        );
        for f in &group.findings {
            let _ = writeln!(
                out,
                "  {}:{}  clippy::{}  {}",
                f.file, f.line, f.lint, f.message
            );
        }
        out.push('\n');
    }
    out.push_str("Next: retrieve the guide and confirm each finding before changing code.\n");
    out
}

pub fn to_json(groups: &[Group<'_>]) -> Value {
    Value::Array(
        groups
            .iter()
            .map(|g| {
                let findings = g
                    .findings
                    .iter()
                    .map(|f| {
                        Value::object([
                            ("lint", Value::string(format!("clippy::{}", f.lint))),
                            ("file", Value::string(f.file.as_str())),
                            (
                                "line",
                                Value::Int(i64::try_from(f.line).unwrap_or(i64::MAX)),
                            ),
                            ("message", Value::string(f.message.as_str())),
                        ])
                    })
                    .collect();
                Value::object([
                    ("guide", Value::string(g.guide.id())),
                    ("findings", Value::Array(findings)),
                ])
            })
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::guide::tests::source;

    const OUTPUT: &str = r#"{"reason":"compiler-artifact","package_id":"x"}
{"reason":"compiler-message","message":{"code":{"code":"clippy::manual_let_else","explanation":null},"level":"warning","message":"this could be rewritten as `let...else`","spans":[{"file_name":"src/lib.rs","line_start":3,"is_primary":true}],"rendered":"x"}}
{"reason":"compiler-message","message":{"code":{"code":"clippy::manual_let_else","explanation":null},"level":"warning","message":"this could be rewritten as `let...else`","spans":[{"file_name":"src/lib.rs","line_start":3,"is_primary":true}],"rendered":"x"}}
{"reason":"compiler-message","message":{"code":{"code":"clippy::needless_return","explanation":null},"level":"warning","message":"unneeded return","spans":[{"file_name":"src/lib.rs","line_start":9,"is_primary":true}],"rendered":"x"}}
{"reason":"compiler-message","message":{"code":{"code":"dead_code","explanation":null},"level":"warning","message":"unused","spans":[],"rendered":"x"}}
{"reason":"build-finished","success":true}"#;

    #[test]
    fn keeps_only_requested_lints_and_dedupes() {
        let findings = parse_findings(OUTPUT, &["manual_let_else".to_owned()]);
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].file, "src/lib.rs");
        assert_eq!(findings[0].line, 3);
        assert_eq!(findings[0].lint, "manual_let_else");
    }

    #[test]
    fn groups_by_guide_and_renders() {
        let guides = [
            Guide::parse(&source("std/a.md", "1.70", "")).unwrap(),
            Guide::parse(&source("std/b.md", "1.70", "")).unwrap(),
        ];
        let refs: Vec<&Guide> = guides.iter().collect();
        let findings = parse_findings(
            OUTPUT,
            &["manual_let_else".to_owned(), "needless_return".to_owned()],
        );
        let groups = group(&refs, &findings);
        assert_eq!(groups.len(), 2);
        let text = render(&groups, 2);
        assert!(
            text.contains("a (1 finding)\n  src/lib.rs:3  clippy::manual_let_else  this could")
        );
        assert!(text.ends_with("confirm each finding before changing code.\n"));
        assert_eq!(render(&[], 2), "No findings for 2 guide lints.\n");
        assert!(
            to_json(&groups)
                .to_string()
                .starts_with(r#"[{"guide":"a","findings":[{"lint":"clippy::manual_let_else""#)
        );
    }
}
