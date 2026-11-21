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
        for raw in text.lines() {
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
    fn ignores_other_values() {
        let t = Toml::parse("[package]\nversion = 1\nfeatures = [\"a\"]\n");
        assert_eq!(t.string("package", "version"), None);
        assert_eq!(t.bool("package", "features"), None);
    }
}
