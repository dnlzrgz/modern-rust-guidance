//! Rendering a guide for `mrg retrieve`: header, MSRV banner and `## Older MSRV` stripping.

use crate::fence::{self, Kind};
use crate::guide::Guide;
use crate::msrv::Version;

pub const OLDER_MSRV: &str = "## Older MSRV";

pub fn render(guide: &Guide, msrv: Option<Version>) -> String {
    let (banner, keep_older) = match msrv {
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
        None => (
            format!(
                "> No MSRV declared (`rust-version`); main guidance assumes current stable. If the project must support older toolchains, see `{OLDER_MSRV}`."
            ),
            true,
        ),
    };
    let body = if keep_older {
        guide.body.clone()
    } else {
        strip_section(&guide.body, OLDER_MSRV)
    };
    format!(
        "<!-- mrg: id={}, category={}, rust-version={} -->\n\n{banner}\n\n{}\n",
        guide.id(),
        guide.category,
        guide.version,
        body.trim_matches('\n')
    )
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
        let out = render(&guide(), Version::parse("1.100"));
        assert!(out.starts_with("<!-- mrg: id=a, category=std, rust-version=1.80 -->\n\n"));
        assert!(out.contains("> Project MSRV 1.100 ≥ 1.80: use the main guidance."));
        assert!(!out.contains("Old way."));
        assert!(out.contains("Run clippy."));
    }

    #[test]
    fn older_msrv_warns_and_keeps_section() {
        let out = render(&guide(), Version::parse("1.74"));
        assert!(out.contains("[!IMPORTANT]"));
        assert!(out.contains("Project MSRV 1.74 < 1.80 required by this guide."));
        assert!(out.contains("Old way."));
    }

    #[test]
    fn undeclared_msrv_keeps_section() {
        let out = render(&guide(), None);
        assert!(out.contains("No MSRV declared"));
        assert!(out.contains("Old way."));
    }
}
