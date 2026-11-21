//! Tokenizer, BM25 ranking over weighted guide fields, and "did you mean" suggestions.

use crate::fence::{self, Kind};
use crate::guide::Guide;
use std::collections::{HashMap, HashSet};

const K1: f64 = 1.2;
const B: f64 = 0.75;

const STOPWORDS: &[&str] = &[
    "a", "an", "and", "are", "as", "at", "be", "by", "do", "for", "from", "how", "i", "in", "is",
    "it", "my", "of", "on", "or", "that", "the", "this", "to", "with",
];

/// Lowercases, splits on non-alphanumerics, keeps whole `snake_case`/`CamelCase` words
/// next to their parts, splits camelCase, strips a trailing plural `s`, drops stopwords.
pub fn tokenize(text: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    for word in text.split(|c: char| !(c.is_alphanumeric() || c == '_')) {
        let parts = split_word(word);
        if parts.len() > 1 {
            tokens.push(word.to_owned());
        }
        tokens.extend(parts);
    }
    tokens
        .into_iter()
        .map(|t| t.to_lowercase())
        .filter(|t| !t.is_empty() && !STOPWORDS.contains(&t.as_str()))
        .map(|t| {
            if t.len() > 3 && t.ends_with('s') {
                t[..t.len() - 1].to_owned()
            } else {
                t
            }
        })
        .collect()
}

/// Splits on `_` and camelCase boundaries (`HTTPServer` → `HTTP`, `Server`).
fn split_word(word: &str) -> Vec<String> {
    let mut parts = Vec::new();
    for piece in word.split('_').filter(|p| !p.is_empty()) {
        let chars: Vec<char> = piece.chars().collect();
        let mut start = 0;
        for i in 1..chars.len() {
            let boundary = (chars[i - 1].is_lowercase() && chars[i].is_uppercase())
                || (chars[i - 1].is_uppercase()
                    && chars[i].is_uppercase()
                    && chars.get(i + 1).is_some_and(|c| c.is_lowercase()));
            if boundary {
                parts.push(chars[start..i].iter().collect());
                start = i;
            }
        }
        parts.push(chars[start..].iter().collect());
    }
    parts
}

#[derive(Debug)]
struct Doc {
    weighted_tf: HashMap<String, f64>,
    len: f64,
}

/// Counts tokens: `(field text, weight)` → weighted term frequencies.
fn doc_of(fields: &[(String, f64)]) -> Doc {
    let mut weighted_tf: HashMap<String, f64> = HashMap::new();
    let mut len = 0.0;
    for (text, weight) in fields {
        for token in tokenize(text) {
            *weighted_tf.entry(token).or_default() += weight;
            len += weight;
        }
    }
    Doc { weighted_tf, len }
}

fn headings(body: &str) -> String {
    let lines: Vec<&str> = fence::classify(body)
        .into_iter()
        .filter(|&(kind, line)| kind == Kind::Prose && line.starts_with('#'))
        .map(|(_, line)| line.trim_start_matches('#').trim())
        .collect();
    lines.join("\n")
}

#[derive(Debug)]
pub struct Index {
    docs: Vec<Doc>,
    doc_freq: HashMap<String, usize>,
    avg_len: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Hit {
    /// Position in the slice passed to [`Index::new`].
    pub index: usize,
    pub score: f64,
}

fn count(n: usize) -> f64 {
    f64::from(u32::try_from(n).unwrap_or(u32::MAX))
}

impl Index {
    pub fn new(guides: &[Guide]) -> Self {
        let docs: Vec<Doc> = guides
            .iter()
            .map(|g| {
                doc_of(&[
                    (g.id().to_owned(), 3.0),
                    (g.front.description.clone(), 3.0),
                    (headings(&g.body), 2.0),
                    (g.front.clippy_lints.join(" "), 2.0),
                    (g.body.clone(), 1.0),
                ])
            })
            .collect();
        let mut doc_freq: HashMap<String, usize> = HashMap::new();
        for doc in &docs {
            for term in doc.weighted_tf.keys() {
                *doc_freq.entry(term.clone()).or_default() += 1;
            }
        }
        let avg_len = if docs.is_empty() {
            1.0
        } else {
            docs.iter().map(|d| d.len).sum::<f64>() / count(docs.len())
        };
        Self {
            docs,
            doc_freq,
            avg_len,
        }
    }

    /// Best matches first; documents matching no query term are omitted.
    pub fn search(&self, query: &str, limit: usize) -> Vec<Hit> {
        let mut seen = HashSet::new();
        let terms: Vec<String> = tokenize(query)
            .into_iter()
            .filter(|t| seen.insert(t.clone()))
            .collect();
        let n = count(self.docs.len());
        let mut hits: Vec<Hit> = self
            .docs
            .iter()
            .enumerate()
            .filter_map(|(index, doc)| {
                let score: f64 = terms
                    .iter()
                    .filter_map(|term| {
                        let tf = *doc.weighted_tf.get(term)?;
                        let df = count(*self.doc_freq.get(term)?);
                        let idf = (1.0 + (n - df + 0.5) / (df + 0.5)).ln();
                        let norm = K1 * (1.0 - B + B * doc.len / self.avg_len);
                        Some(idf * tf * (K1 + 1.0) / (tf + norm))
                    })
                    .sum();
                (score > 0.0).then_some(Hit { index, score })
            })
            .collect();
        hits.sort_by(|a, b| b.score.total_cmp(&a.score).then(a.index.cmp(&b.index)));
        hits.truncate(limit);
        hits
    }
}

pub fn edit_distance(a: &str, b: &str) -> usize {
    let b: Vec<char> = b.chars().collect();
    let mut row: Vec<usize> = (0..=b.len()).collect();
    for (i, ca) in a.chars().enumerate() {
        let mut diag = row[0];
        row[0] = i + 1;
        for (j, &cb) in b.iter().enumerate() {
            let next = (row[j + 1] + 1)
                .min(row[j] + 1)
                .min(diag + usize::from(ca != cb));
            diag = row[j + 1];
            row[j + 1] = next;
        }
    }
    row[b.len()]
}

/// Up to three IDs closest to `wanted`, nearest first.
pub fn closest<'a>(ids: impl IntoIterator<Item = &'a str>, wanted: &str) -> Vec<&'a str> {
    let limit = (wanted.len() / 2).max(3);
    let mut scored: Vec<(usize, &str)> = ids
        .into_iter()
        .map(|id| {
            let near = id.contains(wanted) || wanted.contains(id);
            (if near { 0 } else { edit_distance(id, wanted) }, id)
        })
        .filter(|&(d, _)| d <= limit)
        .collect();
    scored.sort_unstable();
    scored.into_iter().take(3).map(|(_, id)| id).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::guide::Source;

    fn guide(id: &str, description: &str, lints: &str, body: &str) -> Guide {
        let text = format!(
            "---\nname: {id}\ndescription: {description}\nrust-version: \"1.70\"\nclippy-lints: [{lints}]\nsources: []\n---\n{body}"
        );
        Guide::parse(&Source {
            path: format!("std/{id}.md"),
            text,
        })
        .unwrap()
    }

    #[test]
    fn tokenizer_keeps_whole_and_split_words() {
        assert_eq!(
            tokenize("manual_let_else"),
            ["manual_let_else", "manual", "let", "else"]
        );
        assert_eq!(tokenize("LazyLock"), ["lazylock", "lazy", "lock"]);
        assert_eq!(tokenize("HTTPServer"), ["httpserver", "http", "server"]);
        assert_eq!(
            tokenize("std::sync::OnceLock"),
            ["std", "sync", "oncelock", "once", "lock"]
        );
    }

    #[test]
    fn tokenizer_lowercases_strips_plurals_and_stopwords() {
        assert_eq!(
            tokenize("How to use the Iterators and bus"),
            ["use", "iterator", "bus"]
        );
        assert_eq!(tokenize("is_ok"), ["is_ok", "ok"]);
    }

    #[test]
    fn ranks_description_over_body_mentions() {
        let guides = vec![
            guide(
                "a-guide",
                "Reuse buffers.",
                "",
                "# A\n\nmention of allocation once\n",
            ),
            guide(
                "b-guide",
                "Reduce allocations in loops.",
                "",
                "# B\n\nother text\n",
            ),
            guide("c-guide", "Something else.", "", "# C\n\nnothing\n"),
        ];
        let hits = Index::new(&guides).search("allocation", 8);
        assert_eq!(hits.iter().map(|h| h.index).collect::<Vec<_>>(), [1, 0]);
        assert!(hits[0].score > hits[1].score);
    }

    #[test]
    fn matches_lint_names_and_headings_and_limits() {
        let guides = vec![
            guide(
                "early",
                "Return early.",
                "manual_let_else",
                "# Early\n\n## Use let else\n",
            ),
            guide("other", "Other.", "", "# Other\n"),
        ];
        let index = Index::new(&guides);
        assert_eq!(index.search("manual_let_else", 8).len(), 1);
        assert_eq!(index.search("let else", 8)[0].index, 0);
        assert_eq!(index.search("zzz nothing", 8), Vec::<Hit>::new());
        let many: Vec<Guide> = (0..5)
            .map(|i| guide(&format!("g{i}"), "Common words.", "", "# T\n"))
            .collect();
        assert_eq!(Index::new(&many).search("common", 2).len(), 2);
    }

    #[test]
    fn distance_and_suggestions() {
        assert_eq!(edit_distance("kitten", "sitting"), 3);
        assert_eq!(edit_distance("", "abc"), 3);
        let ids = ["lazy-initialization", "error-values", "std"];
        assert_eq!(closest(ids, "lazy-initalization")[0], "lazy-initialization");
        assert_eq!(closest(ids, "lazy"), ["lazy-initialization"]);
        assert_eq!(closest(ids, "qqqqqqqq"), Vec::<&str>::new());
    }
}
