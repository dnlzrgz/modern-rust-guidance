//! Guide model: loading sources (embedded or from disk) and parsing them.

use crate::fence;
use crate::frontmatter::{self, Frontmatter};
use crate::msrv::Version;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

mod embedded {
    include!(concat!(env!("OUT_DIR"), "/guides.rs"));
}

/// The known categories, in display order.
pub const CATEGORIES: [&str; 4] = ["std", "performance", "testing", "ecosystem"];

/// A raw guide file: path relative to the guides directory (`/`-separated) plus contents.
#[derive(Debug, Clone)]
pub struct Source {
    pub path: String,
    pub text: String,
}

#[derive(Debug)]
pub struct Guide {
    pub path: String,
    pub category: String,
    /// File stem; equals `front.name` in a valid guide.
    pub stem: String,
    pub front: Frontmatter,
    pub version: Version,
    pub body: String,
    /// 1-based file line of the first body line.
    pub body_line: usize,
}

impl Guide {
    pub fn parse(source: &Source) -> Result<Self, String> {
        let (category, file) = source
            .path
            .split_once('/')
            .filter(|(_, file)| !file.contains('/'))
            .ok_or("guides must live at `<category>/<id>.md`")?;
        let stem = file
            .strip_suffix(".md")
            .ok_or("guide files must end in `.md`")?;
        let doc = frontmatter::parse(&source.text)?;
        let version = Version::parse(&doc.front.rust_version)
            .filter(|_| is_minor_version(&doc.front.rust_version))
            .ok_or_else(|| {
                format!(
                    "`rust-version: {}` must match ^1\\.\\d+$",
                    doc.front.rust_version
                )
            })?;
        Ok(Self {
            path: source.path.clone(),
            category: category.to_owned(),
            stem: stem.to_owned(),
            front: doc.front,
            version,
            body: doc.body.to_owned(),
            body_line: doc.body_line,
        })
    }

    pub fn id(&self) -> &str {
        &self.front.name
    }

    pub fn is_hub(&self) -> bool {
        self.stem == self.category
    }

    /// True when a prose line equals `heading` (e.g. `## Verify`).
    pub fn has_heading(&self, heading: &str) -> bool {
        fence::classify(&self.body)
            .iter()
            .any(|&(kind, line)| kind == fence::Kind::Prose && line.trim_end() == heading)
    }
}

/// `^1\.\d+$`
fn is_minor_version(text: &str) -> bool {
    text.strip_prefix("1.")
        .is_some_and(|minor| !minor.is_empty() && minor.bytes().all(|b| b.is_ascii_digit()))
}

/// Parses every source; the first failure is reported with its path.
pub fn parse_all(sources: &[Source]) -> Result<Vec<Guide>, String> {
    sources
        .iter()
        .map(|s| Guide::parse(s).map_err(|e| format!("{}: {e}", s.path)))
        .collect()
}

pub fn embedded_sources() -> Vec<Source> {
    embedded::GUIDES
        .iter()
        .map(|&(path, text)| Source {
            path: path.to_owned(),
            text: text.to_owned(),
        })
        .collect()
}

/// Reads every `.md` file below `dir`, sorted by relative path.
pub fn read_dir(dir: &Path) -> Result<Vec<Source>, String> {
    fn walk(dir: &Path, prefix: &str, out: &mut Vec<Source>) -> Result<(), String> {
        let entries =
            fs::read_dir(dir).map_err(|e| format!("cannot read {}: {e}", dir.display()))?;
        for entry in entries {
            let entry = entry.map_err(|e| format!("cannot read {}: {e}", dir.display()))?;
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().into_owned();
            if path.is_dir() {
                walk(&path, &format!("{prefix}{name}/"), out)?;
            } else if path.extension().is_some_and(|ext| ext == "md") {
                let text = fs::read_to_string(&path)
                    .map_err(|e| format!("cannot read {}: {e}", path.display()))?;
                out.push(Source {
                    path: format!("{prefix}{name}"),
                    text,
                });
            }
        }
        Ok(())
    }
    let mut out = Vec::new();
    walk(dir, "", &mut out)?;
    out.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(out)
}

/// Finds `skills/modern-rust-guidance/guides` in `start` or any parent directory.
pub fn find_repo_guides(start: &Path) -> Option<PathBuf> {
    start
        .ancestors()
        .map(|dir| dir.join("skills/modern-rust-guidance/guides"))
        .find(|dir| dir.is_dir())
}

/// Where the guides came from: the directory when read from disk, `None` when embedded.
pub struct Loaded {
    pub sources: Vec<Source>,
    pub dir: Option<PathBuf>,
}

/// Chooses the guide source: `explicit` (flag or `MRG_GUIDES_DIR`), else the repo's
/// on-disk guides when `prefer_repo`, else the embedded set.
pub fn load(explicit: Option<&str>, prefer_repo: bool) -> Result<Loaded, String> {
    let from_env = env::var("MRG_GUIDES_DIR").ok().filter(|v| !v.is_empty());
    let dir = match explicit.map(str::to_owned).or(from_env) {
        Some(dir) => {
            let dir = PathBuf::from(dir);
            if !dir.is_dir() {
                return Err(format!("guides directory {} not found", dir.display()));
            }
            Some(dir)
        }
        None if prefer_repo => env::current_dir()
            .ok()
            .and_then(|cwd| find_repo_guides(&cwd)),
        None => None,
    };
    match dir {
        Some(dir) => Ok(Loaded {
            sources: read_dir(&dir)?,
            dir: Some(dir),
        }),
        None => Ok(Loaded {
            sources: embedded_sources(),
            dir: None,
        }),
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;

    pub fn source(path: &str, version: &str, body: &str) -> Source {
        let id = path.rsplit('/').next().unwrap().trim_end_matches(".md");
        let text = format!(
            "---\nname: {id}\ndescription: Do the {id} thing.\nrust-version: \"{version}\"\nclippy-lints: [manual_let_else]\nsources:\n  - \"x\"\n---\n{body}"
        );
        Source {
            path: path.to_owned(),
            text,
        }
    }

    #[test]
    fn parses_path_and_frontmatter() {
        let guide = Guide::parse(&source("std/lazy-init.md", "1.80", "# T\n")).unwrap();
        assert_eq!(
            (guide.category.as_str(), guide.stem.as_str(), guide.id()),
            ("std", "lazy-init", "lazy-init")
        );
        assert!(!guide.is_hub());
        assert!(
            Guide::parse(&source("std/std.md", "1.70", "# T\n"))
                .unwrap()
                .is_hub()
        );
    }

    #[test]
    fn rejects_bad_paths_and_versions() {
        assert!(Guide::parse(&source("README.md", "1.70", "")).is_err());
        assert!(Guide::parse(&source("a/b/c.md", "1.70", "")).is_err());
        for bad in ["1.70.1", "2.0", "1.", "1.x", "1.80 "] {
            let err = Guide::parse(&source("std/a.md", bad, "")).unwrap_err();
            assert!(err.contains("rust-version"), "{bad}: {err}");
        }
    }

    #[test]
    fn finds_headings_outside_fences_only() {
        let body = "# T\n\n```text\n## Verify\n```\n\n## Older MSRV\n";
        let guide = Guide::parse(&source("std/a.md", "1.80", body)).unwrap();
        assert!(guide.has_heading("## Older MSRV"));
        assert!(!guide.has_heading("## Verify"));
    }

    #[test]
    fn reads_sorted_markdown_from_disk() {
        let dir = std::env::temp_dir().join(format!("mrg-guide-read-{}", std::process::id()));
        fs::create_dir_all(dir.join("std")).unwrap();
        fs::write(dir.join("std/b.md"), "b").unwrap();
        fs::write(dir.join("std/a.md"), "a").unwrap();
        fs::write(dir.join("std/skip.txt"), "x").unwrap();
        let paths: Vec<_> = read_dir(&dir)
            .unwrap()
            .into_iter()
            .map(|s| s.path)
            .collect();
        assert_eq!(paths, ["std/a.md", "std/b.md"]);
        fs::remove_dir_all(&dir).unwrap();
    }
}
