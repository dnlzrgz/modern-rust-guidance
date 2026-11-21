//! Compiler diagnostics extracted from rustc/cargo JSON messages.

use crate::json::{self, Value};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diag {
    pub level: String,
    /// e.g. `clippy::manual_let_else`; absent for most plain errors.
    pub code: Option<String>,
    pub message: String,
    /// File and 1-based line of the primary span.
    pub location: Option<(String, u64)>,
    pub rendered: String,
}

impl Diag {
    /// Reads a rustc diagnostic object (also the `message` of cargo's `compiler-message`).
    pub fn from_value(value: &Value) -> Option<Self> {
        let level = value.get("level")?.as_str()?.to_owned();
        let message = value.get("message")?.as_str()?.to_owned();
        let code = value
            .get("code")
            .and_then(|c| c.get("code"))
            .and_then(Value::as_str)
            .map(str::to_owned);
        let location = value
            .get("spans")
            .and_then(Value::as_array)
            .and_then(|spans| {
                spans
                    .iter()
                    .find(|s| s.get("is_primary").and_then(Value::as_bool) == Some(true))
            })
            .and_then(|span| {
                let file = span.get("file_name")?.as_str()?.to_owned();
                Some((file, span.get("line_start")?.as_u64()?))
            });
        let rendered = value
            .get("rendered")
            .and_then(Value::as_str)
            .unwrap_or(&message)
            .to_owned();
        Some(Self {
            level,
            code,
            message,
            location,
            rendered,
        })
    }

    pub fn is_warning_or_error(&self) -> bool {
        matches!(self.level.as_str(), "warning" | "error") || self.level.starts_with("error:")
    }
}

/// Parses `--error-format=json` output (one JSON object per line); other lines are skipped.
pub fn from_json_lines(text: &str) -> Vec<Diag> {
    text.lines()
        .filter_map(|line| json::parse(line).ok())
        .filter_map(|v| Diag::from_value(&v))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const RUSTC: &str = r#"{"$message_type":"diagnostic","message":"unknown lint: `clippy::nope`","code":{"code":"E0602","explanation":null},"level":"warning","spans":[],"children":[],"rendered":"warning[E0602]: unknown lint\n"}
{"$message_type":"diagnostic","message":"this could be `let...else`","code":{"code":"clippy::manual_let_else","explanation":null},"level":"warning","spans":[{"file_name":"a.rs","line_start":4,"is_primary":false},{"file_name":"a.rs","line_start":5,"is_primary":true}],"children":[],"rendered":"warning: this could be `let...else`\n"}
not json at all
{"artifact":"a.rmeta","emit":"metadata"}
{"$message_type":"diagnostic","message":"mismatched types","code":null,"level":"error","spans":[],"children":[],"rendered":null}"#;

    #[test]
    fn extracts_diagnostics_and_skips_noise() {
        let diags = from_json_lines(RUSTC);
        assert_eq!(diags.len(), 3);
        assert_eq!(diags[0].code.as_deref(), Some("E0602"));
        assert!(diags[0].message.starts_with("unknown lint"));
        assert_eq!(diags[1].code.as_deref(), Some("clippy::manual_let_else"));
        assert_eq!(diags[1].location, Some(("a.rs".to_owned(), 5)));
        assert_eq!(diags[2].code, None);
        assert_eq!(diags[2].rendered, "mismatched types");
        assert!(diags[2].is_warning_or_error());
    }
}
