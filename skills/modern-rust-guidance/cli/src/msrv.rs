//! Version comparison and MSRV detection from `Cargo.toml` / `clippy.toml`.

use crate::toml::Toml;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

/// `MAJOR.MINOR[.PATCH]`, compared numerically (`1.100` > `1.99`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Version {
    major: u32,
    minor: u32,
    patch: u32,
}

impl Version {
    pub fn parse(text: &str) -> Option<Self> {
        let mut parts = text.trim().split('.');
        let major = parts.next()?.parse().ok()?;
        let minor = parts.next()?.parse().ok()?;
        let patch = parts.next().map_or(Some(0), |p| p.parse().ok())?;
        parts.next().is_none().then_some(Self {
            major,
            minor,
            patch,
        })
    }
}

impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}", self.major, self.minor)?;
        if self.patch != 0 {
            write!(f, ".{}", self.patch)?;
        }
        Ok(())
    }
}

#[derive(Debug, PartialEq)]
pub struct Msrv {
    pub version: Version,
    /// Human-readable origin, e.g. `/p/Cargo.toml ([package] rust-version)`.
    pub source: String,
}

fn read_toml(path: &Path) -> Option<Toml> {
    fs::read_to_string(path).ok().map(|text| Toml::parse(&text))
}

fn found(version: &str, path: &Path, what: &str) -> Option<Msrv> {
    Some(Msrv {
        version: Version::parse(version)?,
        source: format!("{} ({what})", path.display()),
    })
}

/// Detects the MSRV, starting in directory `start`.
pub fn detect(start: &Path) -> Option<Msrv> {
    let mut visited: Vec<PathBuf> = Vec::new();
    let mut first_manifest = true;
    for dir in start.ancestors() {
        visited.push(dir.to_path_buf());
        let path = dir.join("Cargo.toml");
        let Some(toml) = read_toml(&path) else {
            continue;
        };
        if first_manifest {
            first_manifest = false;
            if toml.has_section("package") {
                if let Some(v) = toml.string("package", "rust-version") {
                    return found(v, &path, "[package] rust-version");
                }
                if toml.bool("package", "rust-version.workspace") != Some(true) {
                    break;
                }
            }
        }
        if toml.has_section("workspace") {
            if let Some(v) = toml.string("workspace.package", "rust-version") {
                return found(v, &path, "[workspace.package] rust-version");
            }
            break;
        }
    }
    visited.iter().find_map(|dir| {
        [".clippy.toml", "clippy.toml"]
            .into_iter()
            .find_map(|name| {
                let path = dir.join(name);
                let toml = read_toml(&path)?;
                found(toml.string("", "msrv")?, &path, "msrv")
            })
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    fn tempdir(tag: &str) -> PathBuf {
        static COUNTER: AtomicUsize = AtomicUsize::new(0);
        let dir = std::env::temp_dir().join(format!(
            "mrg-msrv-{}-{tag}-{}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn v(text: &str) -> Version {
        Version::parse(text).unwrap()
    }

    #[test]
    fn compares_numerically() {
        assert!(v("1.100") > v("1.99"));
        assert_eq!(v("1.70"), v("1.70.0"));
        assert!(v("1.70.1") > v("1.70"));
        assert!(v("2.0") > v("1.999"));
        assert_eq!(v("1.85.0").to_string(), "1.85");
        assert_eq!(v("1.85.2").to_string(), "1.85.2");
    }

    #[test]
    fn rejects_bad_versions() {
        for bad in ["", "1", "1.x", "1.2.3.4", "a.b", "1.-2"] {
            assert!(Version::parse(bad).is_none(), "{bad:?}");
        }
    }

    #[test]
    fn reads_package_rust_version() {
        let dir = tempdir("pkg");
        fs::write(
            dir.join("Cargo.toml"),
            "[package]\nname = \"x\"\nrust-version = \"1.74\"\n",
        )
        .unwrap();
        let msrv = detect(&dir).unwrap();
        assert_eq!(msrv.version, v("1.74"));
        assert!(msrv.source.contains("[package] rust-version"));
    }

    #[test]
    fn inherits_workspace_rust_version() {
        let dir = tempdir("ws");
        fs::write(
            dir.join("Cargo.toml"),
            "[workspace]\nmembers = [\"m\"]\n[workspace.package]\nrust-version = \"1.80\"\n",
        )
        .unwrap();
        let member = dir.join("m/src");
        fs::create_dir_all(&member).unwrap();
        fs::write(
            dir.join("m/Cargo.toml"),
            "[package]\nname = \"m\"\nrust-version.workspace = true\n",
        )
        .unwrap();
        let msrv = detect(&member).unwrap();
        assert_eq!(msrv.version, v("1.80"));
        assert!(msrv.source.contains("[workspace.package]"));
    }

    #[test]
    fn reads_clippy_toml_when_manifest_has_none() {
        let dir = tempdir("clippy");
        fs::write(dir.join("Cargo.toml"), "[package]\nname = \"x\"\n").unwrap();
        fs::write(dir.join("clippy.toml"), "msrv = \"1.71\"\n").unwrap();
        assert_eq!(detect(&dir).unwrap().version, v("1.71"));
        let dir = tempdir("dotclippy");
        fs::write(dir.join(".clippy.toml"), "msrv = \"1.72\"\n").unwrap();
        assert_eq!(detect(&dir).unwrap().version, v("1.72"));
    }

    #[test]
    fn undeclared_is_none() {
        let dir = tempdir("none");
        fs::write(dir.join("Cargo.toml"), "[package]\nname = \"x\"\n").unwrap();
        assert!(detect(&dir).is_none());
    }
}
