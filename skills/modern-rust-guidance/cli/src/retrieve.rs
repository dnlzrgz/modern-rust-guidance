//! Rendering a guide for `mrg retrieve`: header, MSRV banner and `## Older MSRV` stripping.

use crate::fence::{self, Kind};
use crate::guide::Guide;
use crate::msrv::Version;

pub const OLDER_MSRV: &str = "## Older MSRV";

/// The banner for one guide, and whether its `## Older MSRV` section is kept.
fn banner(guide: &Guide, msrv: Option<Version>) -> (String, bool) {
    match msrv {
        Some(msrv) if msrv >= guide.version => (
            format!(
                "> Project MSRV {msrv} ≥ {}: use the main guidance.",
                guide.version
            ),
            false,
        ),
        Some(msrv) => (
            format!(
                "> [!IMPORTANT]\n> Project MSRV {msrv} < {} required by this guide. Follow `{OLDER_MSRV}`, or ask the user before raising `rust-version`.",
                guide.version
            ),
            true,
        ),
        None => (NO_MSRV_BANNER.to_owned(), true),
    }
}

const NO_MSRV_BANNER: &str = "> No MSRV declared (`rust-version`); main guidance assumes current stable. If the project must support older toolchains, see `## Older MSRV`.";

fn page(guide: &Guide, banner: Option<&str>, keep_older: bool) -> String {
    let body = if keep_older {
        guide.body.clone()
    } else {
        strip_section(&guide.body, OLDER_MSRV)
    };
    let banner = banner.map_or_else(String::new, |b| format!("{b}\n\n"));
    format!(
        "<!-- mrg: id={}, category={}, rust-version={} -->\n\n{banner}{}\n",
        guide.id(),
        guide.category,
        guide.version,
        body.trim_matches('\n')
    )
}

/// Renders several guides separated by `---`. With more than one guide, a banner that would
/// be the same for all of them is printed once at the top: "No MSRV declared", or a declared
/// MSRV that satisfies every guide. Otherwise each guide carries its own banner.
pub fn render_many(guides: &[&Guide], msrv: Option<Version>) -> String {
    let shared = if guides.len() < 2 {
        None
    } else {
        match msrv {
            None => Some(NO_MSRV_BANNER.to_owned()),
            Some(msrv) => guides
                .iter()
                .map(|g| g.version)
                .max()
                .filter(|&highest| msrv >= highest)
                .map(|highest| {
                    format!(
                        "> Project MSRV {msrv} ≥ {highest}, the highest rust-version of these guides: use the main guidance."
                    )
                }),
        }
    };
    let pages: Vec<String> = guides
        .iter()
        .map(|&guide| {
            let (own, keep_older) = banner(guide, msrv);
            let shown = if shared.is_some() {
                None
            } else {
                Some(own.as_str())
            };
            page(guide, shown, keep_older)
        })
        .collect();
    let joined = pages.join("\n---\n\n");
    match shared {
        Some(banner) => format!("{banner}\n\n{joined}"),
        None => joined,
    }
}

/// Removes the section starting at the prose line `heading`, up to the next `#`/`##` heading.
pub fn strip_section(body: &str, heading: &str) -> String {
    let mut skipping = false;
    let mut kept: Vec<&str> = Vec::new();
    for (kind, line) in fence::classify(body) {
        if kind == Kind::Prose {
            if line.trim_end() == heading {
                skipping = true;
                continue;
            }
            if skipping && (line.starts_with("# ") || line.starts_with("## ")) {
                skipping = false;
            }
        }
        if !skipping {
            kept.push(line);
        }
    }
    let mut out = kept.join("\n");
    out.push('\n');
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::guide::tests::source;

    const BODY: &str = "# T\n\nIntro.\n\n## Older MSRV\n\n```text\n## not a heading\n```\n\nOld way.\n\n## Verify\n\nRun clippy.\n";

    fn guide() -> Guide {
        Guide::parse(&source("std/a.md", "1.80", BODY)).unwrap()
    }

    #[test]
    fn strips_only_the_older_msrv_section() {
        let out = strip_section(BODY, OLDER_MSRV);
        assert_eq!(out, "# T\n\nIntro.\n\n## Verify\n\nRun clippy.\n");
        assert_eq!(strip_section("# T\n", OLDER_MSRV), "# T\n");
    }

    #[test]
    fn strips_a_trailing_section() {
        assert_eq!(
            strip_section("# T\n\n## Older MSRV\n\nold\n", OLDER_MSRV),
            "# T\n\n"
        );
    }

    #[test]
    fn newer_msrv_uses_main_guidance_and_drops_section() {
        let out = render_many(&[&guide()], Version::parse("1.100"));
        assert!(out.starts_with("<!-- mrg: id=a, category=std, rust-version=1.80 -->\n\n"));
        assert!(out.contains("> Project MSRV 1.100 ≥ 1.80: use the main guidance."));
        assert!(!out.contains("Old way."));
        assert!(out.contains("Run clippy."));
    }

    #[test]
    fn older_msrv_warns_and_keeps_section() {
        let out = render_many(&[&guide()], Version::parse("1.74"));
        assert!(out.contains("[!IMPORTANT]"));
        assert!(out.contains("Project MSRV 1.74 < 1.80 required by this guide."));
        assert!(out.contains("Old way."));
    }

    fn guide_at(id: &str, version: &str) -> Guide {
        Guide::parse(&source(&format!("std/{id}.md"), version, BODY)).unwrap()
    }

    #[test]
    fn undeclared_msrv_banner_is_printed_once_for_many_guides() {
        let (a, b) = (guide_at("a", "1.80"), guide_at("b", "1.70"));
        let out = render_many(&[&a, &b], None);
        assert!(out.starts_with("> No MSRV declared"));
        assert_eq!(out.matches("No MSRV declared").count(), 1);
        assert!(out.contains("id=a,") && out.contains("id=b,"));
        assert_eq!(out.matches("Old way.").count(), 2);
    }

    #[test]
    fn satisfied_msrv_banner_is_printed_once_and_drops_sections() {
        let (a, b) = (guide_at("a", "1.80"), guide_at("b", "1.70"));
        let out = render_many(&[&a, &b], Version::parse("1.85"));
        assert!(out.starts_with("> Project MSRV 1.85 ≥ 1.80, the highest"));
        assert_eq!(out.matches("Project MSRV").count(), 1);
        assert!(!out.contains("Old way."));
    }

    #[test]
    fn mixed_msrv_keeps_banners_per_guide() {
        let (a, b) = (guide_at("a", "1.80"), guide_at("b", "1.70"));
        let out = render_many(&[&a, &b], Version::parse("1.75"));
        assert!(out.starts_with("<!-- mrg: id=a,"));
        assert_eq!(out.matches("Project MSRV 1.75").count(), 2);
        assert!(out.contains("[!IMPORTANT]"));
        assert_eq!(out.matches("Old way.").count(), 1);
    }

    #[test]
    fn a_single_guide_keeps_its_own_banner() {
        let a = guide_at("a", "1.80");
        let out = render_many(&[&a], None);
        assert!(out.starts_with("<!-- mrg:"));
        assert_eq!(out.matches("No MSRV declared").count(), 1);
    }

    #[test]
    fn undeclared_msrv_keeps_section() {
        let out = render_many(&[&guide()], None);
        assert!(out.contains("No MSRV declared"));
        assert!(out.contains("Old way."));
    }
}
