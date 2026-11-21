//! Structure checks for the guide set (`mrg check` without `--compile`/`--clippy`).

use crate::catalog;
use crate::fence;
use crate::guide::{CATEGORIES, Guide, Source};
use crate::msrv::Version;
use crate::retrieve::OLDER_MSRV;
use std::collections::HashSet;

const MAX_DESCRIPTION: usize = 200;

/// Parses every source and validates the set. `index_md` is the on-disk `index.md`, if any.
/// Returns the guides that parsed, and one message per problem.
pub fn structure(sources: &[Source], index_md: Option<&str>) -> (Vec<Guide>, Vec<String>) {
    let mut errors = Vec::new();
    let mut guides = Vec::new();
    for source in sources {
        match Guide::parse(source) {
            Ok(guide) => guides.push(guide),
            Err(e) => errors.push(format!("{}: {e}", source.path)),
        }
    }
    let mut names = HashSet::new();
    for guide in &guides {
        guide_errors(guide, &mut errors);
        if !names.insert(guide.id()) {
            errors.push(format!(
                "{}: duplicate guide name `{}`",
                guide.path,
                guide.id()
            ));
        }
    }
    for category in CATEGORIES {
        let hub = guides.iter().find(|g| g.category == category && g.is_hub());
        let Some(hub) = hub else {
            errors.push(format!(
                "{category}/{category}.md: category `{category}` has no hub guide"
            ));
            continue;
        };
        for guide in guides
            .iter()
            .filter(|g| g.category == category && !g.is_hub())
        {
            if !hub.body.contains(&format!("`{}`", guide.id())) {
                errors.push(format!(
                    "{}: not mentioned (as `{}`) in its hub {}",
                    guide.path,
                    guide.id(),
                    hub.path
                ));
            }
        }
    }
    // A stale index is only worth reporting once the guides themselves are valid.
    let stale =
        errors.is_empty() && index_md.is_some_and(|on_disk| on_disk != catalog::index(&guides));
    if stale {
        errors.push("index.md: out of date; regenerate it with `mrg index > index.md`".to_owned());
    }
    (guides, errors)
}

fn guide_errors(guide: &Guide, errors: &mut Vec<String>) {
    let mut err = |msg: String| errors.push(format!("{}: {msg}", guide.path));
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
        structure(&set(extra), None).1
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
        let (_, errors) = structure(
            &[
                source("std/std.md", "1.70", "# H\n"),
                source("std/a-guide.md", "1.70", GOOD_BODY),
            ],
            None,
        );
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
        let (guides, errors) = structure(&sources, None);
        assert_eq!(errors, Vec::<String>::new());
        let index = catalog::index(&guides);
        assert_eq!(structure(&sources, Some(&index)).1, Vec::<String>::new());
        let stale = structure(&sources, Some("old")).1;
        assert_eq!(stale.len(), 1);
        assert!(stale[0].starts_with("index.md: out of date"));
    }
}
