//! Line-based TOML reader: section headers, `key = "string"` and `key = true` only.

#[derive(Debug, PartialEq)]
enum Scalar {
    Str(String),
    Bool(bool),
    Other,
}

#[derive(Debug, Default)]
pub struct Toml {
    sections: Vec<String>,
    entries: Vec<(String, String, Scalar)>,
}

impl Toml {
    pub fn parse(text: &str) -> Self {
        let mut toml = Self::default();
        let mut section = String::new();
        let mut state = Continuation::default();
        for raw in text.lines() {
            // Lines inside a multi-line array or string are not headers or entries.
            if state.is_open() {
                state.scan(raw);
                continue;
            }
            state.scan(raw);
            let line = strip_comment(raw).trim();
            if let Some(header) = line.strip_prefix('[') {
                let name = header.trim_start_matches('[').trim_end_matches(']').trim();
                name.clone_into(&mut section);
                toml.sections.push(section.clone());
            } else if let Some((key, value)) = line.split_once('=') {
                toml.entries
                    .push((section.clone(), key.trim().to_owned(), scalar(value.trim())));
            }
        }
        toml
    }

    /// True for `[name]` and any `[name.sub]` table.
    pub fn has_section(&self, name: &str) -> bool {
        self.sections.iter().any(|s| {
            s == name
                || s.strip_prefix(name)
                    .is_some_and(|rest| rest.starts_with('.'))
        })
    }

    fn find(&self, section: &str, key: &str) -> Option<&Scalar> {
        self.entries
            .iter()
            .find(|(s, k, _)| s == section && k == key)
            .map(|(_, _, v)| v)
    }

    pub fn string(&self, section: &str, key: &str) -> Option<&str> {
        match self.find(section, key) {
            Some(Scalar::Str(s)) => Some(s),
            _ => None,
        }
    }

    pub fn bool(&self, section: &str, key: &str) -> Option<bool> {
        match self.find(section, key) {
            Some(Scalar::Bool(b)) => Some(*b),
            _ => None,
        }
    }
}

/// Tracks what spans lines: `"""`/`'''` strings and `[` … `]` arrays (outside strings).
#[derive(Default)]
struct Continuation {
    /// Inside a multi-line string, the quote character that closes it.
    multiline: Option<char>,
    depth: usize,
}

impl Continuation {
    fn is_open(&self) -> bool {
        self.multiline.is_some() || self.depth > 0
    }

    /// Advances over one line.
    fn scan(&mut self, line: &str) {
        let chars: Vec<char> = line.chars().collect();
        let triple = |i: usize, q: char| chars[i..].starts_with(&[q, q, q]);
        let mut i = 0;
        while i < chars.len() {
            let c = chars[i];
            if let Some(q) = self.multiline {
                if q == '"' && c == '\\' {
                    i += 2;
                } else if triple(i, q) {
                    self.multiline = None;
                    i += 3;
                    // Up to two more quotes may end the string (`""""`).
                    while i < chars.len() && chars[i] == q {
                        i += 1;
                    }
                } else {
                    i += 1;
                }
                continue;
            }
            match c {
                '"' | '\'' if triple(i, c) => {
                    self.multiline = Some(c);
                    i += 3;
                }
                '"' | '\'' => {
                    // A single-line string: skip to its closing quote.
                    i += 1;
                    while i < chars.len() && chars[i] != c {
                        i += if c == '"' && chars[i] == '\\' { 2 } else { 1 };
                    }
                    i += 1;
                }
                '#' => return,
                '[' => {
                    self.depth += 1;
                    i += 1;
                }
                ']' => {
                    self.depth = self.depth.saturating_sub(1);
                    i += 1;
                }
                _ => i += 1,
            }
        }
    }
}

/// Cuts a trailing `# comment`, ignoring `#` inside quotes.
fn strip_comment(line: &str) -> &str {
    let mut quote = None;
    for (i, c) in line.char_indices() {
        match (quote, c) {
            (None, '"' | '\'') => quote = Some(c),
            (Some(q), _) if q == c => quote = None,
            (None, '#') => return &line[..i],
            _ => {}
        }
    }
    line
}

fn scalar(value: &str) -> Scalar {
    match value.chars().next() {
        Some('"' | '\'') if value.starts_with("\"\"\"") || value.starts_with("'''") => {
            Scalar::Other
        }
        Some(q @ ('"' | '\'')) => {
            let inner = &value[1..];
            Scalar::Str(inner.find(q).map_or(inner, |end| &inner[..end]).to_owned())
        }
        _ if value == "true" || is_workspace_inline(value) => Scalar::Bool(true),
        _ if value == "false" => Scalar::Bool(false),
        _ => Scalar::Other,
    }
}

/// `{ workspace = true }`, the inline-table spelling of `key.workspace = true`.
fn is_workspace_inline(value: &str) -> bool {
    value.starts_with('{') && value.replace(' ', "").contains("workspace=true")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_sections_strings_and_bools() {
        let t = Toml::parse(
            "# top\nmsrv = \"1.70\" # trailing\n[package]\nname = 'x'\nrust-version.workspace = true\nurl = \"a#b\"\n[workspace.package]\nrust-version = \"1.80\"\n",
        );
        assert_eq!(t.string("", "msrv"), Some("1.70"));
        assert_eq!(t.string("package", "name"), Some("x"));
        assert_eq!(t.bool("package", "rust-version.workspace"), Some(true));
        let inline = Toml::parse("[package]\nrust-version = { workspace = true }\n");
        assert_eq!(inline.bool("package", "rust-version"), Some(true));
        assert_eq!(t.string("package", "url"), Some("a#b"));
        assert_eq!(t.string("workspace.package", "rust-version"), Some("1.80"));
        assert!(t.has_section("package") && t.has_section("workspace"));
        assert!(!t.has_section("dependencies"));
    }

    #[test]
    fn skips_headers_inside_multiline_arrays_and_strings() {
        let t = Toml::parse(
            "[package]\nkeywords = [\n  \"a\",\n[not-a-section]\n  \"]\",\n  [\"nested\"], # [also-not]\n]\ndescription = \"\"\"\n[workspace]\nx = 1\n\"\"\"\nlit = '''\n[lit-section]\n'''\nrust-version = \"1.80\"\n[bin]\n",
        );
        for name in ["not-a-section", "workspace", "also-not", "lit-section"] {
            assert!(!t.has_section(name), "{name}");
        }
        assert_eq!(t.string("package", "rust-version"), Some("1.80"));
        assert_eq!(t.string("package", "x"), None);
        assert!(t.has_section("package") && t.has_section("bin"));
    }

    #[test]
    fn single_line_strings_do_not_open_continuations() {
        let t = Toml::parse("a = \"[\"\nb = '['\n[s]\nk = \"v\"\n");
        assert_eq!(t.string("s", "k"), Some("v"));
        let t = Toml::parse("a = \"\"\"x\"\"\"\n[s]\nk = \"v\"\n");
        assert_eq!(t.string("s", "k"), Some("v"));
    }

    #[test]
    fn ignores_other_values() {
        let t = Toml::parse("[package]\nversion = 1\nfeatures = [\"a\"]\n");
        assert_eq!(t.string("package", "version"), None);
        assert_eq!(t.bool("package", "features"), None);
    }
}
