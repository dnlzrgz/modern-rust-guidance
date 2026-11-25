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

/// What clippy reported for the requested lints.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Scan {
    pub findings: Vec<Finding>,
    /// Findings without a primary span, which cannot be shown as `file:line`.
    pub unlocated: usize,
}

/// Findings that belong to one guide.
pub struct Group<'a> {
    pub guide: &'a Guide,
    pub findings: Vec<&'a Finding>,
}

/// Runs clippy in `dir` and returns the deduplicated findings for `lints`.
pub fn run(dir: &Path, all_targets: bool, lints: &[String]) -> Result<Scan, String> {
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

/// Keeps `compiler-message` lines whose `code.code` is one of `lints`. Findings without a
/// primary span are counted, not listed.
pub fn parse_findings(stdout: &str, lints: &[String]) -> Scan {
    let mut findings = Vec::new();
    let mut unlocated: Vec<(String, String)> = Vec::new();
    let diags = stdout
        .lines()
        .filter_map(|line| json::parse(line).ok())
        .filter(|v| v.get("reason").and_then(Value::as_str) == Some("compiler-message"))
        .filter_map(|v| Diag::from_value(v.get("message")?));
    for diag in diags {
        let Some(lint) = diag
            .code
            .as_deref()
            .and_then(|c| c.strip_prefix("clippy::"))
        else {
            continue;
        };
        if !lints.iter().any(|l| l == lint) {
            continue;
        }
        let lint = lint.to_owned();
        if let Some((file, line)) = diag.location {
            findings.push(Finding {
                lint,
                file,
                line,
                message: diag.message,
            });
        } else {
            unlocated.push((lint, diag.message));
        }
    }
    findings.sort();
    findings.dedup();
    unlocated.sort();
    unlocated.dedup();
    Scan {
        findings,
        unlocated: unlocated.len(),
    }
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

/// `"N finding(s) without location"`, or `None` when there are none.
pub fn unlocated_note(unlocated: usize) -> Option<String> {
    (unlocated > 0).then(|| {
        format!(
            "{unlocated} finding{} without location (no primary span); run `cargo clippy` to see {}.",
            if unlocated == 1 { "" } else { "s" },
            if unlocated == 1 { "it" } else { "them" }
        )
    })
}

pub fn render(groups: &[Group<'_>], lint_count: usize, unlocated: usize) -> String {
    let note = unlocated_note(unlocated).map_or_else(String::new, |n| format!("{n}\n"));
    if groups.is_empty() {
        return format!("No findings for {lint_count} guide lints.\n{note}");
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
    out.push_str(&note);
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
        let Scan {
            findings,
            unlocated,
        } = parse_findings(OUTPUT, &["manual_let_else".to_owned()]);
        assert_eq!(unlocated, 0);
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
        let Scan { findings, .. } = parse_findings(
            OUTPUT,
            &["manual_let_else".to_owned(), "needless_return".to_owned()],
        );
        let groups = group(&refs, &findings);
        assert_eq!(groups.len(), 2);
        let text = render(&groups, 2, 0);
        assert!(
            text.contains("a (1 finding)\n  src/lib.rs:3  clippy::manual_let_else  this could")
        );
        assert!(text.ends_with("confirm each finding before changing code.\n"));
        assert_eq!(render(&[], 2, 0), "No findings for 2 guide lints.\n");
        assert!(
            to_json(&groups)
                .to_string()
                .starts_with(r#"[{"guide":"a","findings":[{"lint":"clippy::manual_let_else""#)
        );
    }

    #[test]
    fn counts_findings_without_a_primary_span() {
        let output = r#"{"reason":"compiler-message","message":{"code":{"code":"clippy::needless_return","explanation":null},"level":"warning","message":"a","spans":[],"rendered":"x"}}
{"reason":"compiler-message","message":{"code":{"code":"clippy::needless_return","explanation":null},"level":"warning","message":"b","spans":[{"file_name":"src/lib.rs","line_start":1,"is_primary":false}],"rendered":"x"}}
{"reason":"compiler-message","message":{"code":{"code":"clippy::other","explanation":null},"level":"warning","message":"c","spans":[],"rendered":"x"}}"#;
        let scan = parse_findings(output, &["needless_return".to_owned()]);
        assert_eq!(scan.findings, []);
        assert_eq!(scan.unlocated, 2);
        let text = render(&[], 1, scan.unlocated);
        assert_eq!(
            text,
            "No findings for 1 guide lints.\n2 findings without location (no primary span); run `cargo clippy` to see them.\n"
        );
        assert!(
            unlocated_note(1)
                .unwrap()
                .starts_with("1 finding without location")
        );
        assert_eq!(unlocated_note(0), None);
    }
}
