//! Argument parsing and command dispatch. Every command returns the text for stdout.

use crate::blocks;
use crate::catalog;
use crate::check;
use crate::guide::{self, Guide};
use crate::json::Value;
use crate::msrv::{self, Version};
use crate::retrieve;
use crate::scan;
use crate::search::{self, Index};
use std::env;
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

pub struct Failure {
    pub code: u8,
    pub message: String,
}

impl From<String> for Failure {
    fn from(message: String) -> Self {
        Self { code: 1, message }
    }
}

fn usage(message: impl Into<String>) -> Failure {
    Failure {
        code: 2,
        message: message.into(),
    }
}

type Output = Result<String, Failure>;

const HELP: &str = "\
mrg: search and retrieve verified modern Rust guidance

Usage: mrg <command> [options]

Commands:
  list      List guides (hubs first)
  search    Rank guides for a task description
  retrieve  Print guides with an MSRV banner
  msrv      Detect the project's MSRV
  scan      Run clippy with every guide's lints and group findings by guide
  lints     Print the lint-to-guide table
  index     Print the markdown catalog (index.md)
  check     Validate the guides (authoring)
  help      Show help for a command

Options:
  --version   Print the version
  --guides-dir <dir>   (most commands; or MRG_GUIDES_DIR) read guides from disk

Run `mrg help <command>` for details.
";

fn command_help(command: &str) -> Option<&'static str> {
    Some(match command {
        "list" => {
            "Usage: mrg list [--category C] [--json] [--guides-dir D]\n\nOne line per guide: `id  [category, rust ≥ X]  description`. Hubs first.\n"
        }
        "search" => {
            "Usage: mrg search <query…> [--limit N] [--json] [--guides-dir D]\n\nBM25 search over guide ids, descriptions, headings, lints and bodies. Default limit 8.\nScores are relative to the best hit (1.00); hits below 0.35 are dropped.\n"
        }
        "retrieve" => {
            "Usage: mrg retrieve <id>[,<id>…] [--msrv X.Y] [--guides-dir D]\n\nPrints each guide with an MSRV banner (MSRV from the current project unless --msrv is given).\nThe `## Older MSRV` section is dropped when the MSRV is high enough. Unknown id: exit 2.\n"
        }
        "msrv" => {
            "Usage: mrg msrv [--manifest-path P]\n\nPrints the MSRV from Cargo.toml (`rust-version`, workspace inheritance) or clippy.toml.\n"
        }
        "scan" => {
            "Usage: mrg scan [PATH] [--all-targets] [--json] [--guides-dir D]\n\nRuns `cargo clippy` in PATH (default `.`) with every guide's lints enabled and groups the\nfindings by guide. This builds the project: build scripts and proc macros run.\nExit 0 when clippy ran (with or without findings), 1 when cargo or clippy failed.\n"
        }
        "lints" => {
            "Usage: mrg lints [--format list|flags|toml] [--guides-dir D]\n\nlist: lint and guide table. flags: `-W clippy::a -W clippy::b …`. toml: a `[lints.clippy]` block.\n"
        }
        "index" => {
            "Usage: mrg index [--guides-dir D]\n\nPrints the markdown catalog stored in skills/modern-rust-guidance/index.md.\n"
        }
        "check" => {
            "Usage: mrg check [--compile] [--clippy] [--guides-dir D]\n\nValidates guide structure. --compile builds every rust block with rustc; --clippy runs\nclippy-driver and verifies `// lint:` markers and unknown lints. Exit 1 on any error.\n"
        }
        _ => return None,
    })
}

pub fn run(args: &[String]) -> Output {
    let Some((command, rest)) = args.split_first() else {
        return Ok(HELP.to_owned());
    };
    match command.as_str() {
        "--version" | "-V" | "version" => Ok(format!("mrg {}\n", env!("CARGO_PKG_VERSION"))),
        "--help" | "-h" => Ok(HELP.to_owned()),
        "help" => match rest.first() {
            None => Ok(HELP.to_owned()),
            Some(name) => command_help(name)
                .map(str::to_owned)
                .ok_or_else(|| usage(format!("no help for `{name}`"))),
        },
        "list" => list(&Parsed::new(rest, &["json"], &["category", "guides-dir"])?),
        "search" => search_cmd(&Parsed::new(rest, &["json"], &["limit", "guides-dir"])?),
        "retrieve" => retrieve_cmd(&Parsed::new(rest, &[], &["msrv", "guides-dir"])?),
        "msrv" => msrv_cmd(&Parsed::new(rest, &[], &["manifest-path"])?),
        "scan" => scan_cmd(&Parsed::new(
            rest,
            &["all-targets", "json"],
            &["guides-dir"],
        )?),
        "lints" => lints(&Parsed::new(rest, &[], &["format", "guides-dir"])?),
        "index" => index(&Parsed::new(rest, &[], &["guides-dir"])?),
        "check" => check_cmd(&Parsed::new(rest, &["compile", "clippy"], &["guides-dir"])?),
        other => Err(usage(format!("unknown command `{other}`; run `mrg help`"))),
    }
}

/// Options and positional arguments of one command.
struct Parsed {
    positional: Vec<String>,
    flags: Vec<String>,
    values: Vec<(String, String)>,
}

impl Parsed {
    /// `bools` are `--flag`s, `valued` are `--name value` / `--name=value` options.
    fn new(args: &[String], bools: &[&str], valued: &[&str]) -> Result<Self, Failure> {
        let mut parsed = Self {
            positional: Vec::new(),
            flags: Vec::new(),
            values: Vec::new(),
        };
        let mut iter = args.iter();
        while let Some(arg) = iter.next() {
            if arg == "--" {
                parsed.positional.extend(iter.by_ref().cloned());
                break;
            }
            let Some(option) = arg.strip_prefix("--") else {
                parsed.positional.push(arg.clone());
                continue;
            };
            let (name, inline) = option
                .split_once('=')
                .map_or((option, None), |(n, v)| (n, Some(v.to_owned())));
            if bools.contains(&name) && inline.is_none() {
                parsed.flags.push(name.to_owned());
            } else if valued.contains(&name) {
                let value = match inline {
                    Some(v) => v,
                    None => iter
                        .next()
                        .cloned()
                        .ok_or_else(|| usage(format!("`--{name}` needs a value")))?,
                };
                parsed.values.push((name.to_owned(), value));
            } else {
                return Err(usage(format!("unknown option `--{name}`")));
            }
        }
        Ok(parsed)
    }

    fn flag(&self, name: &str) -> bool {
        self.flags.iter().any(|f| f == name)
    }

    fn value(&self, name: &str) -> Option<&str> {
        self.values
            .iter()
            .rev()
            .find(|(n, _)| n == name)
            .map(|(_, v)| v.as_str())
    }
}

fn load_guides(
    parsed: &Parsed,
    prefer_repo: bool,
) -> Result<(Vec<Guide>, Option<PathBuf>), Failure> {
    let loaded = guide::load(parsed.value("guides-dir"), prefer_repo)?;
    Ok((guide::parse_all(&loaded.sources)?, loaded.dir))
}

/// Joins items into text with one item per line, ending in a newline.
fn lines(items: impl Iterator<Item = String>) -> String {
    items.fold(String::new(), |mut out, item| {
        out.push_str(&item);
        out.push('\n');
        out
    })
}

fn json_line(value: &Value) -> String {
    format!("{value}\n")
}

fn list(parsed: &Parsed) -> Output {
    let (guides, _) = load_guides(parsed, false)?;
    let category = parsed.value("category");
    let shown: Vec<&Guide> = catalog::hubs_first(&guides)
        .into_iter()
        .filter(|g| category.is_none_or(|c| g.category == c))
        .collect();
    if parsed.flag("json") {
        return Ok(json_line(&Value::Array(
            shown.into_iter().map(catalog::to_json).collect(),
        )));
    }
    Ok(lines(shown.into_iter().map(catalog::summary)))
}

fn search_cmd(parsed: &Parsed) -> Output {
    if parsed.positional.is_empty() {
        return Err(usage("usage: mrg search <query…>"));
    }
    let limit = match parsed.value("limit") {
        Some(n) => n
            .parse::<usize>()
            .map_err(|_| usage(format!("invalid --limit `{n}`")))?,
        None => 8,
    };
    let (guides, _) = load_guides(parsed, false)?;
    let hits = Index::new(&guides).search(&parsed.positional.join(" "), limit);
    if parsed.flag("json") {
        let items = hits
            .iter()
            .map(|hit| {
                let Value::Object(mut pairs) = catalog::to_json(&guides[hit.index]) else {
                    unreachable!()
                };
                pairs.insert(0, ("score".to_owned(), Value::Float(hit.score)));
                Value::Object(pairs)
            })
            .collect();
        return Ok(json_line(&Value::Array(items)));
    }
    if hits.is_empty() {
        return Ok("No guides matched. Try other words, or run `mrg list`.\n".to_owned());
    }
    Ok(lines(hits.iter().map(|hit| {
        catalog::search_line(&guides[hit.index], hit.score)
    })))
}

fn retrieve_cmd(parsed: &Parsed) -> Output {
    let ids: Vec<&str> = parsed
        .positional
        .iter()
        .flat_map(|arg| arg.split(','))
        .map(str::trim)
        .filter(|id| !id.is_empty())
        .collect();
    if ids.is_empty() {
        return Err(usage("usage: mrg retrieve <id>[,<id>…] [--msrv X.Y]"));
    }
    let (guides, _) = load_guides(parsed, false)?;
    let project_msrv = match parsed.value("msrv") {
        Some(text) => {
            Some(Version::parse(text).ok_or_else(|| usage(format!("invalid --msrv `{text}`")))?)
        }
        None => env::current_dir()
            .ok()
            .and_then(|cwd| msrv::detect(&cwd))
            .map(|m| m.version),
    };
    let mut selected: Vec<&Guide> = Vec::new();
    for id in ids {
        let Some(guide) = guides.iter().find(|g| g.id() == id) else {
            let suggestions = search::closest(guides.iter().map(Guide::id), id);
            let mut message = format!("unknown guide `{id}`");
            if !suggestions.is_empty() {
                let list: Vec<String> = suggestions.iter().map(|s| format!("`{s}`")).collect();
                let _ = write!(message, "; did you mean {}?", list.join(", "));
            }
            message.push_str(
                "\nRun `mrg search <words>` to find a guide, or `mrg list` to see them all.",
            );
            return Err(usage(message));
        };
        selected.push(guide);
    }
    Ok(retrieve::render_many(&selected, project_msrv))
}

fn msrv_cmd(parsed: &Parsed) -> Output {
    let start = match parsed.value("manifest-path") {
        Some(path) => {
            let path = Path::new(path);
            let dir = if path.is_dir() {
                path
            } else {
                path.parent().unwrap_or(Path::new(""))
            };
            if dir.as_os_str().is_empty() {
                PathBuf::from(".")
            } else {
                dir.to_path_buf()
            }
        }
        None => PathBuf::from("."),
    };
    let start =
        fs::canonicalize(&start).map_err(|e| format!("cannot read {}: {e}", start.display()))?;
    let declared = msrv::declared(&start);
    let conflict = declared.conflicting_clippy().map(|clippy| {
        format!(
            "clippy msrv {} (from {}) differs; mrg uses `rust-version`\n",
            clippy.version, clippy.source
        )
    });
    Ok(match declared.effective() {
        Some(found) => format!(
            "MSRV {} (from {})\n{}",
            found.version,
            found.source,
            conflict.unwrap_or_default()
        ),
        None => "MSRV not declared (no `rust-version` in Cargo.toml, no `msrv` in clippy.toml)\n"
            .to_owned(),
    })
}

fn scan_cmd(parsed: &Parsed) -> Output {
    let dir = parsed.positional.first().map_or(".", String::as_str);
    let (guides, _) = load_guides(parsed, false)?;
    let table = catalog::lint_table(&guides);
    if table.is_empty() {
        return Ok("No guide lists any clippy lints; nothing to scan for.\n".to_owned());
    }
    let lints: Vec<String> = table.keys().cloned().collect();
    eprintln!(
        "mrg: scan builds the project in {dir} with `cargo clippy` (build scripts and proc macros run)."
    );
    let scanned = scan::run(Path::new(dir), parsed.flag("all-targets"), &lints)?;
    let ordered = catalog::hubs_first(&guides);
    let groups = scan::group(&ordered, &scanned.findings);
    Ok(if parsed.flag("json") {
        if let Some(note) = scan::unlocated_note(scanned.unlocated) {
            eprintln!("mrg: {note}");
        }
        json_line(&scan::to_json(&groups))
    } else {
        scan::render(&groups, lints.len(), scanned.unlocated)
    })
}

fn lints(parsed: &Parsed) -> Output {
    let (guides, _) = load_guides(parsed, false)?;
    let table = catalog::lint_table(&guides);
    match parsed.value("format").unwrap_or("list") {
        "list" => Ok(lines(
            table
                .iter()
                .map(|(lint, ids)| format!("{lint}  {}", ids.join(", "))),
        )),
        "flags" => Ok(format!(
            "{}\n",
            table
                .keys()
                .map(|l| format!("-W clippy::{l}"))
                .collect::<Vec<_>>()
                .join(" ")
        )),
        "toml" => {
            let mut out = String::from("[lints.clippy]\n");
            for lint in table.keys() {
                let _ = writeln!(out, "{lint} = \"warn\"");
            }
            Ok(out)
        }
        other => Err(usage(format!(
            "unknown --format `{other}` (expected list, flags or toml)"
        ))),
    }
}

fn index(parsed: &Parsed) -> Output {
    let (guides, _) = load_guides(parsed, true)?;
    Ok(catalog::index(&guides))
}

fn check_cmd(parsed: &Parsed) -> Output {
    let loaded = guide::load(parsed.value("guides-dir"), true)?;
    // `index.md` sits next to `guides/`, only checked for an on-disk guide set.
    let index_md = loaded
        .dir
        .as_deref()
        .and_then(Path::parent)
        .and_then(|p| fs::read_to_string(p.join("index.md")).ok());
    let check::Structure {
        guides,
        mut errors,
        flawed,
    } = check::structure(&loaded.sources, index_md.as_deref());
    let (compile, clippy) = (parsed.flag("compile"), parsed.flag("clippy"));
    if compile || clippy {
        let (sound, skipped): (Vec<&Guide>, Vec<&Guide>) =
            guides.iter().partition(|g| !flawed.contains(&g.path));
        for guide in skipped {
            eprintln!(
                "mrg: skipped compile/clippy checks for {} (it has structure errors)",
                guide.path
            );
        }
        errors.extend(blocks::check(
            &sound,
            compile,
            clippy,
            loaded.dir.as_deref(),
        )?);
    }
    if errors.is_empty() {
        let mut what = String::from("structure");
        if compile {
            what.push_str(", compile");
        }
        if clippy {
            what.push_str(", clippy");
        }
        return Ok(format!("ok: {} guides passed ({what})\n", guides.len()));
    }
    let n = errors.len();
    Err(Failure {
        code: 1,
        message: format!(
            "{}\n{n} problem{} found",
            errors.join("\n"),
            if n == 1 { "" } else { "s" }
        ),
    })
}
