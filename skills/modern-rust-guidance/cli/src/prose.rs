//! Prose-level rules for guide bodies: Rust version mentions and cross-references.

use crate::fence::{self, Kind};
use crate::retrieve::OLDER_MSRV;

/// Prose lines (outside code fences) with their 0-based body line index. The
/// `## Older MSRV` section is left out unless `include_older`.
fn prose_lines(body: &str, include_older: bool) -> Vec<(usize, &str)> {
    let mut in_older = false;
    let mut out = Vec::new();
    for (i, (kind, line)) in fence::classify(body).into_iter().enumerate() {
        if kind != Kind::Prose {
            continue;
        }
        if line.trim_end() == OLDER_MSRV {
            in_older = true;
        } else if in_older && (line.starts_with("# ") || line.starts_with("## ")) {
            in_older = false;
        }
        if include_older || !in_older {
            out.push((i, line));
        }
    }
    out
}

/// A `1.N[.P][+]` token found on a line.
#[derive(Debug, PartialEq, Eq)]
struct VersionToken {
    minor: u32,
    /// Starts right after `needs `.
    after_needs: bool,
    /// Followed by `+`.
    plus: bool,
}

fn version_tokens(line: &str) -> Vec<VersionToken> {
    let bytes = line.as_bytes();
    let digits = |from: usize| {
        bytes[from..]
            .iter()
            .take_while(|b| b.is_ascii_digit())
            .count()
    };
    let is_word = |b: u8| b.is_ascii_alphanumeric() || b == b'_';
    let mut tokens = Vec::new();
    let mut i = 0;
    while i + 2 < bytes.len() {
        let standalone_start = i == 0 || !(is_word(bytes[i - 1]) || bytes[i - 1] == b'.');
        if !(standalone_start && bytes[i] == b'1' && bytes[i + 1] == b'.') {
            i += 1;
            continue;
        }
        let minor_len = digits(i + 2);
        let mut end = i + 2 + minor_len;
        if !(1..=3).contains(&minor_len) {
            i = end.max(i + 1);
            continue;
        }
        let minor: u32 = line[i + 2..end].parse().expect("1-3 ASCII digits");
        // An optional `.P` patch part.
        if bytes.get(end) == Some(&b'.') {
            let patch_len = digits(end + 1);
            if (1..=3).contains(&patch_len) {
                end += 1 + patch_len;
            }
        }
        let plus = bytes.get(end) == Some(&b'+');
        if plus {
            end += 1;
        }
        if bytes.get(end).is_some_and(|&b| {
            is_word(b) || b == b'.' && bytes.get(end + 1).is_some_and(u8::is_ascii_digit)
        }) {
            // `1.88x`, `1.2.3.4`: not a version token.
            i = end;
            continue;
        }
        let after_needs = line[..i].to_ascii_lowercase().ends_with("needs ");
        tokens.push(VersionToken {
            minor,
            after_needs,
            plus,
        });
        i = end;
    }
    tokens
}

/// Mentions of a Rust version newer than `rust_minor` in the main text: `(body line
/// index, minor)`. A line may name a newer version if it marks it `needs 1.N` or `1.N+`.
pub fn newer_version_mentions(body: &str, rust_minor: u32) -> Vec<(usize, u32)> {
    let mut found = Vec::new();
    for (i, line) in prose_lines(body, false) {
        let tokens = version_tokens(line);
        for token in &tokens {
            let marked = tokens
                .iter()
                .any(|t| t.minor == token.minor && (t.after_needs || t.plus));
            if token.minor > rust_minor && !marked && !found.contains(&(i, token.minor)) {
                found.push((i, token.minor));
            }
        }
    }
    found
}

/// `^[a-z0-9]+(-[a-z0-9]+)*$`
pub fn is_id_shaped(token: &str) -> bool {
    !token.is_empty()
        && token.split('-').all(|part| {
            !part.is_empty()
                && part
                    .bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit())
        })
}

/// Id-shaped targets of ``see `id` `` / ``see also `id` `` (any case of `see`), as
/// `(body line index, id)`. Looks at all prose, including `## Older MSRV`.
pub fn see_references(body: &str) -> Vec<(usize, String)> {
    let mut found = Vec::new();
    for (i, line) in prose_lines(body, true) {
        let lower = line.to_ascii_lowercase();
        let mut from = 0;
        while let Some(pos) = lower[from..].find("see") {
            let start = from + pos;
            from = start + 3;
            let word_start = start == 0 || !lower.as_bytes()[start - 1].is_ascii_alphanumeric();
            if !word_start {
                continue;
            }
            let mut rest = lower[from..].trim_start_matches(' ');
            if from == lower.len() || rest.len() == lower.len() - from {
                // `see` must be followed by a space (or `also `).
                continue;
            }
            if let Some(after) = rest.strip_prefix("also") {
                if after.starts_with(' ') {
                    rest = after.trim_start_matches(' ');
                }
            }
            let Some(quoted) = rest.strip_prefix('`') else {
                continue;
            };
            let Some(end) = quoted.find('`') else {
                continue;
            };
            let token = &quoted[..end];
            if is_id_shaped(token) {
                found.push((i, token.to_owned()));
            }
        }
    }
    found
}

#[cfg(test)]
mod tests {
    use super::*;

    const NONE: [(usize, u32); 0] = [];

    fn mentions(body: &str, minor: u32) -> Vec<(usize, u32)> {
        newer_version_mentions(body, minor)
    }

    #[test]
    fn flags_newer_versions_in_forms() {
        let body = "a (1.88) b\nuse 1.91.0 here\nplain 1.100.\nold 1.70 and 1.80\n";
        assert_eq!(mentions(body, 80), [(0, 88), (1, 91), (2, 100)]);
    }

    #[test]
    fn allows_needs_and_plus_markers_for_the_same_version() {
        assert_eq!(mentions("`x` (needs 1.91) panics\n", 80), NONE);
        assert_eq!(mentions("Needs 1.91 for `x`\n", 80), NONE);
        assert_eq!(mentions("`x` (1.91+) panics\n", 80), NONE);
        assert_eq!(mentions("`x` (1.91) is fine on 1.91+\n", 80), NONE);
        // A marker for another version does not cover this one.
        assert_eq!(mentions("`x` (1.92) and needs 1.91\n", 80), [(0, 92)]);
        assert_eq!(mentions("`x` (1.92) and 1.91+\n", 80), [(0, 92)]);
        assert_eq!(
            mentions("`x` (1.91)\nneeds 1.91 on the next line\n", 80),
            [(0, 91)]
        );
    }

    #[test]
    fn ignores_non_versions() {
        let body = "v1.99 and 21.99 and 1.5x and 0.1.99 and 1.9999 and 1.2.3.4 and 1.\n";
        assert_eq!(mentions(body, 70), NONE);
        assert_eq!(mentions("(1.99)\n1.99.\n", 70), [(0, 99), (1, 99)]);
    }

    #[test]
    fn skips_code_fences_and_older_msrv_section() {
        let body = "# T\n\n```rust\n// 1.99\n```\n\nmain 1.99\n\n## Older MSRV\n\nuse 1.99 here\n\n## Verify\n\nlast 1.98\n";
        assert_eq!(mentions(body, 80), [(6, 99), (14, 98)]);
    }

    #[test]
    fn finds_see_references() {
        let body = "See `a-guide`. Also see also `b2` and SEE `std`; seen `no`.\nsee `Vec::new`, see `with_underscore`, see `x`\nsee [x](y) see `\n```text\nsee `in-fence`\n```\nThe see`nospace` and Foresee `ok-id`.\n";
        let refs = see_references(body);
        let ids: Vec<(usize, &str)> = refs.iter().map(|(i, s)| (*i, s.as_str())).collect();
        assert_eq!(ids, [(0, "a-guide"), (0, "b2"), (0, "std"), (1, "x")]);
    }

    #[test]
    fn id_shape() {
        for ok in ["a", "a-b", "a1-b2-c3", "std"] {
            assert!(is_id_shaped(ok), "{ok}");
        }
        for bad in ["", "-a", "a-", "a--b", "A", "a_b", "a::b", "a b"] {
            assert!(!is_id_shaped(bad), "{bad}");
        }
    }
}
