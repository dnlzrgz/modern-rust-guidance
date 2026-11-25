//! Structure checks for the guide set (`mrg check` without `--compile`/`--clippy`).

use crate::catalog;
use crate::fence;
use crate::guide::{CATEGORIES, Guide, Source};
use crate::msrv::Version;
use crate::prose;
use crate::retrieve::OLDER_MSRV;
use std::collections::HashSet;

const MAX_DESCRIPTION: usize = 200;

/// The outcome of the structure checks.
pub struct Structure {
    /// The guides that parsed.
    pub guides: Vec<Guide>,
    /// One message per problem.
    pub errors: Vec<String>,
    /// Paths of guides that have at least one problem of their own.
    pub flawed: HashSet<String>,
}

/// Collects problems and remembers which guide each belongs to.
#[derive(Default)]
struct Problems {
    messages: Vec<String>,
    flawed: HashSet<String>,
}

impl Problems {
    /// A problem in `guide`; `msg` follows `<path>: `.
    fn guide(&mut self, guide: &Guide, msg: &str) {
        self.at(guide, &format!("{}: {msg}", guide.path));
    }

    /// A problem in `guide` with a fully formatted message.
    fn at(&mut self, guide: &Guide, message: &str) {
        self.flawed.insert(guide.path.clone());
        self.messages.push(message.to_owned());
    }
}

/// Parses every source and validates the set. `index_md` is the on-disk `index.md`, if any.
pub fn structure(sources: &[Source], index_md: Option<&str>) -> Structure {
    let mut problems = Problems::default();
    let mut guides = Vec::new();
    for source in sources {
        match Guide::parse(source) {
            Ok(guide) => guides.push(guide),
            Err(e) => problems.messages.push(format!("{}: {e}", source.path)),
        }
    }
    let mut names = HashSet::new();
    for guide in &guides {
        guide_errors(guide, &mut problems);
        if !names.insert(guide.id()) {
            problems.guide(guide, &format!("duplicate guide name `{}`", guide.id()));
        }
    }
    let ids: HashSet<&str> = guides.iter().map(Guide::id).collect();
    for guide in &guides {
        for (i, target) in prose::see_references(&guide.body) {
            if !ids.contains(target.as_str()) {
                let line = guide.body_line + i;
                problems.at(
                    guide,
                    &format!(
                        "{}:{line}: `see `{target}`` does not name an existing guide",
                        guide.path
                    ),
                );
            }
        }
    }
    for category in CATEGORIES {
        let hub = guides.iter().find(|g| g.category == category && g.is_hub());
        let Some(hub) = hub else {
            problems.messages.push(format!(
                "{category}/{category}.md: category `{category}` has no hub guide"
            ));
            continue;
        };
        for guide in guides
            .iter()
            .filter(|g| g.category == category && !g.is_hub())
        {
            if !hub.body.contains(&format!("`{}`", guide.id())) {
                problems.guide(
                    guide,
                    &format!(
                        "not mentioned (as `{}`) in its hub {}",
                        guide.id(),
                        hub.path
                    ),
                );
            }
        }
    }
    // A stale index is only worth reporting once the guides themselves are valid.
    let stale = problems.messages.is_empty()
        && index_md.is_some_and(|on_disk| on_disk != catalog::index(&guides));
    if stale {
        problems
            .messages
            .push("index.md: out of date; regenerate it with `mrg index > index.md`".to_owned());
    }
    Structure {
        guides,
        errors: problems.messages,
        flawed: problems.flawed,
    }
}

fn guide_errors(guide: &Guide, problems: &mut Problems) {
    let mut err = |msg: String| problems.guide(guide, &msg);
    if guide.front.name != guide.stem {
        err(format!(
            "`name: {}` must equal the file stem `{}`",
            guide.front.name, guide.stem
        ));
    }
    if !CATEGORIES.contains(&guide.category.as_str()) {
        err(format!(
            "`{}` is not a known category ({})",
            guide.category,
            CATEGORIES.join(", ")
        ));
    }
    description_errors(&guide.front.description, &mut err);
    let needs_older = guide.version > Version::parse("1.70").expect("valid literal");
    if needs_older != guide.has_heading(OLDER_MSRV) {
        err(if needs_older {
            format!(
                "rust-version {} > 1.70 requires a `{OLDER_MSRV}` section",
                guide.version
            )
        } else {
            format!("`{OLDER_MSRV}` is only allowed when rust-version > 1.70")
        });
    }
    if !guide.is_hub() && !guide.has_heading("## Verify") {
        err("missing `## Verify` section".to_owned());
    }
    if guide.body.contains("](http") {
        err("external link in body (`](http`); credit goes in `sources`".to_owned());
    }
    for fence in fence::fences(&guide.body) {
        let line = guide.body_line + fence.line;
        if !fence.closed {
            err(format!("line {line}: unclosed code fence"));
        } else if fence.lang().is_none() {
            err(format!(
                "line {line}: fence language `{}` is not allowed",
                fence.info
            ));
        }
    }
    if !guide.is_hub() {
        version_errors(guide, problems);
    }
}

/// Versions newer than the guide's `rust-version` in its main text.
fn version_errors(guide: &Guide, problems: &mut Problems) {
    let minor = guide.version.minor();
    for (i, newer) in prose::newer_version_mentions(&guide.body, minor) {
        let line = guide.body_line + i;
        problems.at(
            guide,
            &format!(
                "{}:{line}: mentions Rust 1.{newer} but rust-version is 1.{minor} (mark it 'needs 1.{newer}' if optional, or raise rust-version)",
                guide.path
            ),
        );
    }
}

fn description_errors(description: &str, err: &mut impl FnMut(String)) {
    if !description.chars().next().is_some_and(char::is_uppercase) {
        err("description must start with a capital letter".to_owned());
    }
    // A sentence break is `.`, `!` or `?` followed by a space and a capital letter;
    // `format! in loops` and `the ? operator` are not breaks.
    let chars: Vec<char> = description.chars().collect();
    let two_sentences = chars
        .windows(3)
        .any(|w| matches!(w[0], '.' | '!' | '?') && w[1] == ' ' && w[2].is_uppercase());
    if two_sentences {
        err("description must be one sentence".to_owned());
    }
    let len = description.chars().count();
    if len > MAX_DESCRIPTION {
        err(format!(
            "description is {len} characters; the limit is {MAX_DESCRIPTION}"
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::guide::tests::source;

    const HUB_BODY: &str = "# Hub\n\nSee `a-guide`.\n";
    const GOOD_BODY: &str = "# A\n\n```rust\nfn x() {}\n```\n\n## Verify\n\nRun it.\n";

    fn set(extra: Vec<Source>) -> Vec<Source> {
        let mut all = vec![
            source("std/std.md", "1.70", HUB_BODY),
            source("performance/performance.md", "1.70", "# P\n"),
            source("testing/testing.md", "1.70", "# T\n"),
        ];
        all.extend(extra);
        all
    }

    fn errors_for(extra: Vec<Source>) -> Vec<String> {
        structure(&set(extra), None).errors
    }

    #[test]
    fn accepts_a_valid_set() {
        assert_eq!(
            errors_for(vec![source("std/a-guide.md", "1.70", GOOD_BODY)]),
            Vec::<String>::new()
        );
    }

    #[test]
    fn reports_missing_hub_and_unlisted_guide() {
        let errors = structure(
            &[
                source("std/std.md", "1.70", "# H\n"),
                source("std/a-guide.md", "1.70", GOOD_BODY),
            ],
            None,
        )
        .errors;
        assert!(
            errors
                .iter()
                .any(|e| e.contains("category `performance` has no hub"))
        );
        assert!(
            errors
                .iter()
                .any(|e| e.contains("std/a-guide.md: not mentioned"))
        );
    }

    #[test]
    fn reports_name_category_and_duplicates() {
        let mut bad = source("std/a-guide.md", "1.70", GOOD_BODY);
        bad.text = bad.text.replace("name: a-guide", "name: other");
        let errors = errors_for(vec![bad, source("weird/z.md", "1.70", GOOD_BODY)]);
        assert!(
            errors
                .iter()
                .any(|e| e.contains("must equal the file stem"))
        );
        assert!(errors.iter().any(|e| e.contains("not a known category")));
        let mut dup = source("std/b-guide.md", "1.70", GOOD_BODY);
        dup.text = dup.text.replace("name: b-guide", "name: a-guide");
        let errors = errors_for(vec![source("std/a-guide.md", "1.70", GOOD_BODY), dup]);
        assert!(errors.iter().any(|e| e.contains("duplicate guide name")));
    }

    #[test]
    fn checks_description_rules() {
        for (desc, needle) in [
            ("lowercase start.", "capital"),
            ("One. Two sentences.", "one sentence"),
            (&format!("Long {}", "x".repeat(200)), "characters"),
        ] {
            let mut g = source("std/a-guide.md", "1.70", GOOD_BODY);
            g.text = g.text.replace("Do the a-guide thing.", desc);
            let errors = errors_for(vec![g]);
            assert!(
                errors.iter().any(|e| e.contains(needle)),
                "{desc}: {errors:?}"
            );
        }
    }

    #[test]
    fn older_msrv_section_iff_version_above_1_70() {
        let errors = errors_for(vec![source("std/a-guide.md", "1.80", GOOD_BODY)]);
        assert!(
            errors
                .iter()
                .any(|e| e.contains("requires a `## Older MSRV` section"))
        );
        let with = format!("{GOOD_BODY}\n## Older MSRV\n\nOld.\n");
        assert_eq!(
            errors_for(vec![source("std/a-guide.md", "1.80", &with)]),
            Vec::<String>::new()
        );
        let errors = errors_for(vec![source("std/a-guide.md", "1.70", &with)]);
        assert!(errors.iter().any(|e| e.contains("only allowed when")));
    }

    #[test]
    fn checks_verify_links_and_fences() {
        let body =
            "# A\n\nSee [x](https://example.com).\n\n```python\nx\n```\n\n```rust\nfn a() {}\n";
        let errors = errors_for(vec![source("std/a-guide.md", "1.70", body)]);
        for needle in [
            "`## Verify`",
            "external link",
            "`python` is not allowed",
            "unclosed code fence",
        ] {
            assert!(
                errors.iter().any(|e| e.contains(needle)),
                "{needle}: {errors:?}"
            );
        }
    }

    #[test]
    fn compares_index_md() {
        let sources = set(vec![source("std/a-guide.md", "1.70", GOOD_BODY)]);
        let Structure { guides, errors, .. } = structure(&sources, None);
        assert_eq!(errors, Vec::<String>::new());
        let index = catalog::index(&guides);
        assert_eq!(
            structure(&sources, Some(&index)).errors,
            Vec::<String>::new()
        );
        let stale = structure(&sources, Some("old")).errors;
        assert_eq!(stale.len(), 1);
        assert!(stale[0].starts_with("index.md: out of date"));
    }

    #[test]
    fn reports_version_mentions_newer_than_rust_version() {
        // Line 4 of the file is body line 1 + 7 frontmatter lines + ... computed below.
        let body = "# A\n\nUse `x` (1.88) here.\n\n```rust\n// 1.99\nfn x() {}\n```\n\n`y` (needs 1.91) is optional.\n\n## Verify\n\nRun it.\n\n## Older MSRV\n\nBefore 1.99 do z.\n";
        let src = source("std/a-guide.md", "1.80", body);
        let first = src.text.lines().position(|l| l == "# A").unwrap() + 1;
        let errors = errors_for(vec![src]);
        let expected = format!(
            "std/a-guide.md:{}: mentions Rust 1.88 but rust-version is 1.80 (mark it 'needs 1.88' if optional, or raise rust-version)",
            first + 2
        );
        assert_eq!(errors, [expected]);
        // Hubs are exempt.
        let mut sources = set(vec![source("std/a-guide.md", "1.70", GOOD_BODY)]);
        sources[0] = source("std/std.md", "1.70", "# Hub\n\n`a-guide` (1.99)\n");
        assert_eq!(structure(&sources, None).errors, Vec::<String>::new());
    }

    #[test]
    fn reports_unknown_see_targets() {
        let body =
            "# A\n\nSee `nope-guide` and see also `std`; see `Vec::new`.\n\n## Verify\n\nRun it.\n";
        let src = source("std/a-guide.md", "1.70", body);
        let first = src.text.lines().position(|l| l == "# A").unwrap() + 1;
        let errors = errors_for(vec![src]);
        assert_eq!(
            errors,
            [format!(
                "std/a-guide.md:{}: `see `nope-guide`` does not name an existing guide",
                first + 2
            )]
        );
        let ok = "# A\n\nSee `a-guide`.\n\n## Verify\n\nRun it.\n";
        assert_eq!(
            errors_for(vec![source("std/a-guide.md", "1.70", ok)]),
            Vec::<String>::new()
        );
    }

    #[test]
    fn tracks_guides_with_problems() {
        let bad = source("std/a-guide.md", "1.70", "# A\n");
        let good = source("std/b-guide.md", "1.70", GOOD_BODY);
        let mut hub = source("std/std.md", "1.70", "# Hub\n\n`a-guide` `b-guide`\n");
        hub.text.push('\n');
        let result = structure(
            &set(vec![bad, good])
                .into_iter()
                .skip(1)
                .chain([hub])
                .collect::<Vec<_>>(),
            None,
        );
        assert!(result.flawed.contains("std/a-guide.md"));
        assert!(!result.flawed.contains("std/b-guide.md"));
    }
}
