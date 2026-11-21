//! Minimal JSON value, parser and compact writer (enough for cargo/rustc messages).

use std::fmt;

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    String(String),
    Array(Vec<Value>),
    Object(Vec<(String, Value)>),
}

impl Value {
    pub fn get(&self, key: &str) -> Option<&Value> {
        match self {
            Self::Object(pairs) => pairs.iter().find(|(k, _)| k == key).map(|(_, v)| v),
            _ => None,
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            Self::String(s) => Some(s),
            _ => None,
        }
    }

    pub fn as_array(&self) -> Option<&[Value]> {
        match self {
            Self::Array(items) => Some(items),
            _ => None,
        }
    }

    pub fn as_u64(&self) -> Option<u64> {
        match self {
            Self::Int(n) => u64::try_from(*n).ok(),
            _ => None,
        }
    }

    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Self::Bool(b) => Some(*b),
            _ => None,
        }
    }

    pub fn string(s: impl Into<String>) -> Self {
        Self::String(s.into())
    }

    pub fn object<const N: usize>(pairs: [(&str, Value); N]) -> Self {
        Self::Object(pairs.into_iter().map(|(k, v)| (k.to_owned(), v)).collect())
    }

    pub fn strings(items: &[String]) -> Self {
        Self::Array(items.iter().map(Self::string).collect())
    }
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Null => f.write_str("null"),
            Self::Bool(b) => write!(f, "{b}"),
            Self::Int(n) => write!(f, "{n}"),
            // JSON has no NaN or infinity.
            Self::Float(n) => {
                if n.is_finite() {
                    write!(f, "{n}")
                } else {
                    f.write_str("null")
                }
            }
            Self::String(s) => write_string(f, s),
            Self::Array(items) => {
                f.write_str("[")?;
                for (i, item) in items.iter().enumerate() {
                    if i > 0 {
                        f.write_str(",")?;
                    }
                    write!(f, "{item}")?;
                }
                f.write_str("]")
            }
            Self::Object(pairs) => {
                f.write_str("{")?;
                for (i, (key, value)) in pairs.iter().enumerate() {
                    if i > 0 {
                        f.write_str(",")?;
                    }
                    write_string(f, key)?;
                    write!(f, ":{value}")?;
                }
                f.write_str("}")
            }
        }
    }
}

fn write_string(f: &mut fmt::Formatter<'_>, s: &str) -> fmt::Result {
    f.write_str("\"")?;
    for c in s.chars() {
        match c {
            '"' => f.write_str("\\\"")?,
            '\\' => f.write_str("\\\\")?,
            '\n' => f.write_str("\\n")?,
            '\r' => f.write_str("\\r")?,
            '\t' => f.write_str("\\t")?,
            c if u32::from(c) < 0x20 => write!(f, "\\u{:04x}", u32::from(c))?,
            c => write!(f, "{c}")?,
        }
    }
    f.write_str("\"")
}

const MAX_DEPTH: usize = 128;

/// Parses one complete JSON document.
pub fn parse(input: &str) -> Result<Value, String> {
    let mut parser = Parser {
        chars: input.chars().collect(),
        pos: 0,
    };
    let value = parser.value(0)?;
    parser.skip_ws();
    if parser.pos < parser.chars.len() {
        return Err(parser.error("trailing characters"));
    }
    Ok(value)
}

struct Parser {
    chars: Vec<char>,
    pos: usize,
}

impl Parser {
    fn error(&self, what: &str) -> String {
        format!("invalid JSON at character {}: {what}", self.pos)
    }

    fn peek(&self) -> Option<char> {
        self.chars.get(self.pos).copied()
    }

    fn skip_ws(&mut self) {
        while self.peek().is_some_and(char::is_whitespace) {
            self.pos += 1;
        }
    }

    fn eat(&mut self, expected: char) -> Result<(), String> {
        if self.peek() == Some(expected) {
            self.pos += 1;
            Ok(())
        } else {
            Err(self.error(&format!("expected `{expected}`")))
        }
    }

    fn literal(&mut self, word: &str, value: Value) -> Result<Value, String> {
        for expected in word.chars() {
            self.eat(expected)?;
        }
        Ok(value)
    }

    fn value(&mut self, depth: usize) -> Result<Value, String> {
        if depth > MAX_DEPTH {
            return Err(self.error("nesting too deep"));
        }
        self.skip_ws();
        match self.peek() {
            Some('{') => self.object(depth),
            Some('[') => self.array(depth),
            Some('"') => self.string().map(Value::String),
            Some('t') => self.literal("true", Value::Bool(true)),
            Some('f') => self.literal("false", Value::Bool(false)),
            Some('n') => self.literal("null", Value::Null),
            Some(c) if c == '-' || c.is_ascii_digit() => self.number(),
            _ => Err(self.error("unexpected token")),
        }
    }

    fn object(&mut self, depth: usize) -> Result<Value, String> {
        self.eat('{')?;
        let mut pairs = Vec::new();
        self.skip_ws();
        if self.peek() == Some('}') {
            self.pos += 1;
            return Ok(Value::Object(pairs));
        }
        loop {
            self.skip_ws();
            let key = self.string()?;
            self.skip_ws();
            self.eat(':')?;
            pairs.push((key, self.value(depth + 1)?));
            self.skip_ws();
            match self.peek() {
                Some(',') => self.pos += 1,
                Some('}') => {
                    self.pos += 1;
                    return Ok(Value::Object(pairs));
                }
                _ => return Err(self.error("expected `,` or `}`")),
            }
        }
    }

    fn array(&mut self, depth: usize) -> Result<Value, String> {
        self.eat('[')?;
        let mut items = Vec::new();
        self.skip_ws();
        if self.peek() == Some(']') {
            self.pos += 1;
            return Ok(Value::Array(items));
        }
        loop {
            items.push(self.value(depth + 1)?);
            self.skip_ws();
            match self.peek() {
                Some(',') => self.pos += 1,
                Some(']') => {
                    self.pos += 1;
                    return Ok(Value::Array(items));
                }
                _ => return Err(self.error("expected `,` or `]`")),
            }
        }
    }

    fn number(&mut self) -> Result<Value, String> {
        let start = self.pos;
        while self
            .peek()
            .is_some_and(|c| c.is_ascii_digit() || "+-.eE".contains(c))
        {
            self.pos += 1;
        }
        let text: String = self.chars[start..self.pos].iter().collect();
        if !text.contains(['.', 'e', 'E']) {
            if let Ok(n) = text.parse::<i64>() {
                return Ok(Value::Int(n));
            }
        }
        text.parse::<f64>()
            .map(Value::Float)
            .map_err(|_| self.error("bad number"))
    }

    fn hex4(&mut self) -> Result<u32, String> {
        let mut n = 0;
        for _ in 0..4 {
            let digit = self
                .peek()
                .and_then(|c| c.to_digit(16))
                .ok_or_else(|| self.error("bad \\u escape"))?;
            n = n * 16 + digit;
            self.pos += 1;
        }
        Ok(n)
    }

    fn string(&mut self) -> Result<String, String> {
        self.eat('"')?;
        let mut out = String::new();
        loop {
            let c = self
                .peek()
                .ok_or_else(|| self.error("unterminated string"))?;
            self.pos += 1;
            match c {
                '"' => return Ok(out),
                '\\' => {
                    let esc = self
                        .peek()
                        .ok_or_else(|| self.error("unterminated escape"))?;
                    self.pos += 1;
                    match esc {
                        '"' | '\\' | '/' => out.push(esc),
                        'b' => out.push('\u{8}'),
                        'f' => out.push('\u{c}'),
                        'n' => out.push('\n'),
                        'r' => out.push('\r'),
                        't' => out.push('\t'),
                        'u' => out.push(self.unicode_escape()?),
                        _ => return Err(self.error("bad escape")),
                    }
                }
                c => out.push(c),
            }
        }
    }

    /// Decodes the part after `\u`, combining surrogate pairs.
    fn unicode_escape(&mut self) -> Result<char, String> {
        let first = self.hex4()?;
        if (0xD800..0xDC00).contains(&first) && self.peek() == Some('\\') {
            let save = self.pos;
            self.pos += 1;
            if self.peek() == Some('u') {
                self.pos += 1;
                let second = self.hex4()?;
                if (0xDC00..0xE000).contains(&second) {
                    let code = 0x10000 + ((first - 0xD800) << 10) + (second - 0xDC00);
                    return Ok(char::from_u32(code).unwrap_or(char::REPLACEMENT_CHARACTER));
                }
            }
            self.pos = save;
        }
        Ok(char::from_u32(first).unwrap_or(char::REPLACEMENT_CHARACTER))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_scalars_and_containers() {
        let v = parse(r#" {"a": [1, -2, 3.5, true, null], "b": {"c": "x"}} "#).unwrap();
        let a = v.get("a").unwrap().as_array().unwrap();
        assert_eq!(a[0], Value::Int(1));
        assert_eq!(a[1], Value::Int(-2));
        assert_eq!(a[2], Value::Float(3.5));
        assert_eq!(a[3].as_bool(), Some(true));
        assert_eq!(a[4], Value::Null);
        assert_eq!(v.get("b").unwrap().get("c").unwrap().as_str(), Some("x"));
    }

    #[test]
    fn parses_escapes_and_surrogates() {
        let v = parse(r#""a\n\"b\" \u00e9 \ud83d\ude00""#).unwrap();
        assert_eq!(v.as_str(), Some("a\n\"b\" é 😀"));
    }

    #[test]
    fn parses_clippy_message_sample() {
        let line = r#"{"reason":"compiler-message","package_id":"x 0.1.0","message":{"code":{"code":"clippy::manual_let_else","explanation":null},"level":"warning","message":"this could be rewritten as `let...else`","spans":[{"file_name":"src/lib.rs","line_start":3,"is_primary":true}]}}"#;
        let v = parse(line).unwrap();
        assert_eq!(v.get("reason").unwrap().as_str(), Some("compiler-message"));
        let message = v.get("message").unwrap();
        assert_eq!(
            message.get("code").unwrap().get("code").unwrap().as_str(),
            Some("clippy::manual_let_else")
        );
        let span = &message.get("spans").unwrap().as_array().unwrap()[0];
        assert_eq!(span.get("line_start").unwrap().as_u64(), Some(3));
        assert_eq!(span.get("is_primary").unwrap().as_bool(), Some(true));
    }

    #[test]
    fn rejects_malformed_input() {
        for bad in [
            "",
            "{",
            "[1,]",
            r#"{"a" 1}"#,
            "tru",
            r#""abc"#,
            "1 2",
            r#""\q""#,
        ] {
            assert!(parse(bad).is_err(), "{bad:?} should fail");
        }
        assert!(parse(&"[".repeat(500)).is_err());
    }

    #[test]
    fn writes_compact_escaped_output() {
        let v = Value::object([
            ("s", Value::string("a\"b\\c\n\u{1}")),
            ("n", Value::Int(3)),
            ("l", Value::strings(&["x".to_owned()])),
        ]);
        assert_eq!(v.to_string(), r#"{"s":"a\"b\\c\n\u0001","n":3,"l":["x"]}"#);
        assert_eq!(parse(&v.to_string()).unwrap(), v);
    }
}
