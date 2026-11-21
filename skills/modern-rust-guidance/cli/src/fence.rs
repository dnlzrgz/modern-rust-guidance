//! Line classification for guide bodies: prose versus fenced code blocks.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Prose,
    Open,
    Code,
    Close,
}

/// A fenced code block. `line` is the 0-based body line of the opening fence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fence {
    pub info: String,
    pub line: usize,
    pub code: String,
    pub closed: bool,
}

/// What a fence's info string means to `mrg check`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lang {
    Rust,
    Legacy,
    CompileFail,
    Ignore,
    Other,
}

impl Fence {
    /// `None` when the info string is not in the allowed set.
    pub fn lang(&self) -> Option<Lang> {
        let tokens: Vec<&str> = self.info.split(',').map(str::trim).collect();
        match tokens.as_slice() {
            ["rust"] => Some(Lang::Rust),
            ["rust", "legacy"] => Some(Lang::Legacy),
            ["rust", "compile_fail"] => Some(Lang::CompileFail),
            ["rust", "ignore"] => Some(Lang::Ignore),
            ["toml" | "sh" | "text"] => Some(Lang::Other),
            _ => None,
        }
    }

    /// Lints named by a first-line `// lint: a, b` marker.
    pub fn lint_markers(&self) -> Vec<String> {
        let first = self.code.lines().next().unwrap_or_default();
        let Some(list) = first.trim().strip_prefix("// lint:") else {
            return Vec::new();
        };
        list.split(',')
            .map(|l| l.trim().trim_start_matches("clippy::").to_owned())
            .filter(|l| !l.is_empty())
            .collect()
    }
}

fn opening_len(line: &str) -> Option<usize> {
    let trimmed = line.trim_start();
    let ticks = trimmed.chars().take_while(|&c| c == '`').count();
    (ticks >= 3 && !trimmed[ticks..].contains('`')).then_some(ticks)
}

/// Classifies every line of `body`.
pub fn classify(body: &str) -> Vec<(Kind, &str)> {
    let mut open: Option<usize> = None;
    let mut out = Vec::new();
    for line in body.lines() {
        let kind = match open {
            None => match opening_len(line) {
                Some(n) => {
                    open = Some(n);
                    Kind::Open
                }
                None => Kind::Prose,
            },
            Some(n) => {
                let t = line.trim();
                if t.len() >= n && t.chars().all(|c| c == '`') {
                    open = None;
                    Kind::Close
                } else {
                    Kind::Code
                }
            }
        };
        out.push((kind, line));
    }
    out
}

pub fn fences(body: &str) -> Vec<Fence> {
    let mut out: Vec<Fence> = Vec::new();
    let mut current: Option<Fence> = None;
    for (i, (kind, line)) in classify(body).into_iter().enumerate() {
        match kind {
            Kind::Prose => {}
            Kind::Open => {
                let info = line.trim_start().trim_start_matches('`').trim().to_owned();
                current = Some(Fence {
                    info,
                    line: i,
                    code: String::new(),
                    closed: false,
                });
            }
            Kind::Code => {
                if let Some(f) = current.as_mut() {
                    f.code.push_str(line);
                    f.code.push('\n');
                }
            }
            Kind::Close => {
                if let Some(mut f) = current.take() {
                    f.closed = true;
                    out.push(f);
                }
            }
        }
    }
    out.extend(current);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const BODY: &str = "# T\n\n```rust\nlet x = 1;\n# not a heading\n```\n\ntext\n\n````text\n```\nnested\n````\n\n```rust,legacy\n// lint: manual_let_else, clippy::question_mark\nfn a() {}\n```\n";

    #[test]
    fn extracts_fences_with_lines() {
        let fences = fences(BODY);
        assert_eq!(fences.len(), 3);
        assert_eq!((fences[0].info.as_str(), fences[0].line), ("rust", 2));
        assert_eq!(fences[0].code, "let x = 1;\n# not a heading\n");
        assert_eq!(fences[1].code, "```\nnested\n");
        assert_eq!(fences[2].line, 14);
        assert!(fences.iter().all(|f| f.closed));
    }

    #[test]
    fn classifies_prose_outside_fences_only() {
        let prose: Vec<&str> = classify(BODY)
            .into_iter()
            .filter(|(k, _)| *k == Kind::Prose)
            .map(|(_, l)| l)
            .collect();
        assert_eq!(prose, ["# T", "", "", "text", "", ""]);
    }

    #[test]
    fn detects_unclosed_fence() {
        let fences = fences("```rust\nfn a() {}\n");
        assert_eq!(fences.len(), 1);
        assert!(!fences[0].closed);
    }

    #[test]
    fn maps_languages() {
        let lang = |info: &str| {
            Fence {
                info: info.into(),
                line: 0,
                code: String::new(),
                closed: true,
            }
            .lang()
        };
        assert_eq!(lang("rust"), Some(Lang::Rust));
        assert_eq!(lang("rust,legacy"), Some(Lang::Legacy));
        assert_eq!(lang("rust, compile_fail"), Some(Lang::CompileFail));
        assert_eq!(lang("rust,ignore"), Some(Lang::Ignore));
        assert_eq!(lang("toml"), Some(Lang::Other));
        assert_eq!(lang("python"), None);
        assert_eq!(lang(""), None);
    }

    #[test]
    fn reads_lint_markers() {
        assert_eq!(
            fences(BODY)[2].lint_markers(),
            ["manual_let_else", "question_mark"]
        );
        assert_eq!(fences(BODY)[0].lint_markers(), Vec::<String>::new());
    }
}
