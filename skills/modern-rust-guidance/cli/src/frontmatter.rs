//! Parser for the YAML subset used in guide frontmatter.
//!
//! Supported: `key: value` pairs, quoted or bare strings, inline lists (`[a, b]`),
//! block lists (`- item`) and `#` comments. Nothing else.

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Frontmatter {
    pub name: String,
    pub description: String,
    pub rust_version: String,
    pub clippy_lints: Vec<String>,
    pub sources: Vec<String>,
}

/// A parsed guide file: frontmatter plus the markdown body.
#[derive(Debug)]
pub struct Document<'a> {
    pub front: Frontmatter,
    pub body: &'a str,
    /// 1-based line number in the file of the first body line.
    pub body_line: usize,
}

#[derive(Debug)]
enum Raw {
    Scalar(String),
    List(Vec<String>),
}

pub fn parse(text: &str) -> Result<Document<'_>, String> {
    let mut lines = text.split_inclusive('\n');
    let first = lines.next().unwrap_or_default();
    if first.trim_end() != "---" {
        return Err("missing frontmatter: the file must start with `---`".to_owned());
    }
    let mut offset = first.len();
    let mut entries: Vec<(String, Raw)> = Vec::new();
    let mut closed_at = None;
    for (i, line) in lines.enumerate() {
        let line_no = i + 2;
        offset += line.len();
        let bare = line.trim_end();
        if bare == "---" {
            closed_at = Some(line_no);
            break;
        }
        entry(&mut entries, strip_comment(bare).trim_end())
            .map_err(|e| format!("frontmatter line {line_no}: {e}"))?;
    }
    let Some(closed_at) = closed_at else {
        return Err("frontmatter is not closed by a second `---` line".to_owned());
    };
    let front = Frontmatter {
        name: scalar(&entries, "name")?,
        description: scalar(&entries, "description")?,
        rust_version: scalar(&entries, "rust-version")?,
        clippy_lints: list(&entries, "clippy-lints")?,
        sources: list(&entries, "sources")?,
    };
    Ok(Document {
        front,
        body: &text[offset..],
        body_line: closed_at + 1,
    })
}

fn entry(entries: &mut Vec<(String, Raw)>, line: &str) -> Result<(), String> {
    if line.trim().is_empty() {
        return Ok(());
    }
    let trimmed = line.trim_start();
    if let Some(item) = trimmed
        .strip_prefix('-')
        .filter(|rest| rest.is_empty() || rest.starts_with(' '))
    {
        return match entries.last_mut() {
            Some((_, Raw::List(items))) => {
                items.push(unquote(item.trim()));
                Ok(())
            }
            _ => Err("list item without a list key".to_owned()),
        };
    }
    if line.starts_with(char::is_whitespace) {
        return Err("unexpected indentation".to_owned());
    }
    let (key, value) = line.split_once(':').ok_or("expected `key: value`")?;
    let key = key.trim();
    if key.is_empty() || entries.iter().any(|(k, _)| k == key) {
        return Err(format!("empty or duplicate key `{key}`"));
    }
    let value = value.trim();
    let raw = if value.is_empty() {
        Raw::List(Vec::new())
    } else if let Some(inner) = value.strip_prefix('[') {
        let inner = inner
            .strip_suffix(']')
            .ok_or("inline list is missing `]`")?;
        Raw::List(split_list(inner))
    } else {
        Raw::Scalar(unquote(value))
    };
    entries.push((key.to_owned(), raw));
    Ok(())
}

fn scalar(entries: &[(String, Raw)], key: &str) -> Result<String, String> {
    match entries.iter().find(|(k, _)| k == key) {
        Some((_, Raw::Scalar(s))) if !s.is_empty() => Ok(s.clone()),
        Some(_) => Err(format!("field `{key}` must be a non-empty string")),
        None => Err(format!("missing required field `{key}`")),
    }
}

fn list(entries: &[(String, Raw)], key: &str) -> Result<Vec<String>, String> {
    match entries.iter().find(|(k, _)| k == key) {
        Some((_, Raw::List(items))) => Ok(items.clone()),
        Some(_) => Err(format!("field `{key}` must be a list")),
        None => Err(format!("missing required field `{key}`")),
    }
}

/// Cuts a `# comment` that starts a line or follows whitespace, outside quotes.
fn strip_comment(line: &str) -> &str {
    let mut quote = None;
    let mut prev_space = true;
    for (i, c) in line.char_indices() {
        match quote {
            Some(q) => {
                if q == c {
                    quote = None;
                }
            }
            None if c == '"' || c == '\'' => quote = Some(c),
            None if c == '#' && prev_space => return &line[..i],
            None => {}
        }
        prev_space = c.is_whitespace();
    }
    line
}

fn split_list(inner: &str) -> Vec<String> {
    let mut items = Vec::new();
    let mut quote = None;
    let mut current = String::new();
    for c in inner.chars() {
        match quote {
            Some(q) => {
                if q == c {
                    quote = None;
                }
            }
            None if c == '"' || c == '\'' => quote = Some(c),
            None if c == ',' => {
                items.push(std::mem::take(&mut current));
                continue;
            }
            None => {}
        }
        current.push(c);
    }
    items.push(current);
    items
        .iter()
        .map(|s| unquote(s.trim()))
        .filter(|s| !s.is_empty())
        .collect()
}

fn unquote(value: &str) -> String {
    let value = value.trim();
    if let Some(inner) = value.strip_prefix('"').and_then(|v| v.strip_suffix('"')) {
        inner.replace("\\\"", "\"").replace("\\\\", "\\")
    } else if let Some(inner) = value.strip_prefix('\'').and_then(|v| v.strip_suffix('\'')) {
        inner.replace("''", "'")
    } else {
        value.to_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const GOOD: &str = r#"---
name: lazy-initialization            # == file stem
description: Initialize global values without macros.
rust-version: "1.80"
clippy-lints: [non_std_lazy_statics, missing_const_for_thread_local]
sources:
  - "std docs: std::sync::LazyLock (MIT/Apache-2.0)"
  - 'Rust Performance Book, Heap Allocations'
---
# Title

Body.
"#;

    #[test]
    fn parses_all_fields() {
        let doc = parse(GOOD).unwrap();
        assert_eq!(doc.front.name, "lazy-initialization");
        assert_eq!(
            doc.front.description,
            "Initialize global values without macros."
        );
        assert_eq!(doc.front.rust_version, "1.80");
        assert_eq!(
            doc.front.clippy_lints,
            ["non_std_lazy_statics", "missing_const_for_thread_local"]
        );
        assert_eq!(
            doc.front.sources,
            [
                "std docs: std::sync::LazyLock (MIT/Apache-2.0)",
                "Rust Performance Book, Heap Allocations"
            ]
        );
        assert!(doc.body.starts_with("# Title"));
        assert_eq!(doc.body_line, 10);
    }

    #[test]
    fn accepts_empty_and_block_lints() {
        let text = GOOD.replace(
            "clippy-lints: [non_std_lazy_statics, missing_const_for_thread_local]",
            "clippy-lints: []",
        );
        assert_eq!(
            parse(&text).unwrap().front.clippy_lints,
            Vec::<String>::new()
        );
        let text = GOOD.replace(
            "clippy-lints: [non_std_lazy_statics, missing_const_for_thread_local]",
            "clippy-lints:\n  - a\n  - b",
        );
        assert_eq!(parse(&text).unwrap().front.clippy_lints, ["a", "b"]);
    }

    #[test]
    fn keeps_hash_and_colon_inside_values() {
        let text = GOOD.replace(
            "Initialize global values without macros.",
            "\"Use C# style: no.\" # note",
        );
        assert_eq!(parse(&text).unwrap().front.description, "Use C# style: no.");
        let text = GOOD.replace("Initialize global values without macros.", "Handle a: b #1");
        assert_eq!(parse(&text).unwrap().front.description, "Handle a: b");
    }

    #[test]
    fn reports_errors() {
        assert!(
            parse("# no frontmatter")
                .unwrap_err()
                .contains("must start with `---`")
        );
        assert!(parse("---\nname: x\n").unwrap_err().contains("not closed"));
        let missing = GOOD.replace("rust-version: \"1.80\"\n", "");
        assert!(
            parse(&missing)
                .unwrap_err()
                .contains("missing required field `rust-version`")
        );
        let dup = GOOD.replace("rust-version", "name");
        assert!(parse(&dup).unwrap_err().contains("duplicate"));
        let bad_list = GOOD.replace("clippy-lints: [", "clippy-lints: x[");
        assert!(parse(&bad_list).unwrap_err().contains("must be a list"));
    }
}
