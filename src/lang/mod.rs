//! The languages archboard reads, and the packages it finds them in.
//!
//! A package is a directory with a manifest (`Cargo.toml` with a `[package]`,
//! `package.json`, `go.mod`, `pyproject.toml`, `setup.py` or `setup.cfg`); one
//! directory with several manifests is one package in several languages. Each
//! language says which files are its own, which file stands for a folder
//! (`mod.rs`, `index.ts`, `__init__.py`), how a file says what it is for, and
//! how its imports name other files ([`rust`], [`js`], [`python`], [`go`]).

pub mod go;
pub mod js;
pub mod python;
pub mod rust;

use std::collections::BTreeMap;
use std::path::Path;

use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::config::Config;
use crate::glob;

/// A language archboard reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Lang {
    /// `.rs`, modules under a crate's `src/`.
    Rust,
    /// TypeScript and JavaScript.
    Js,
    /// `.py`.
    Python,
    /// `.go`.
    Go,
}

impl Lang {
    /// Every language, in the order manifests name a package.
    pub const ALL: [Self; 4] = [Self::Rust, Self::Js, Self::Go, Self::Python];

    /// The language of a source file, by extension; TypeScript declaration
    /// files are not source.
    #[must_use]
    pub fn of_file(path: &str) -> Option<Self> {
        if path.ends_with(".d.ts") || path.ends_with(".d.mts") || path.ends_with(".d.cts") {
            return None;
        }
        match path.rsplit_once('.')?.1 {
            "rs" => Some(Self::Rust),
            "ts" | "tsx" | "mts" | "cts" | "js" | "jsx" | "mjs" | "cjs" => Some(Self::Js),
            "py" => Some(Self::Python),
            "go" => Some(Self::Go),
            _ => None,
        }
    }

    /// File stems that stand for the folder they are in.
    #[must_use]
    pub const fn dir_root_stems(self) -> &'static [&'static str] {
        match self {
            Self::Rust => &["mod"],
            Self::Js => &["index"],
            Self::Python => &["__init__"],
            Self::Go => &[],
        }
    }

    /// File stems at the top of a package's sources that stand for the package.
    #[must_use]
    pub const fn package_root_stems(self) -> &'static [&'static str] {
        match self {
            Self::Rust => &["lib", "main"],
            _ => &[],
        }
    }

    /// Whether a file opens with a doc saying what it is for: `//!` in Rust,
    /// a docstring in Python, a leading comment block in TypeScript and
    /// JavaScript, a `// Package` comment in Go.
    #[must_use]
    pub fn has_doc(self, text: &str) -> bool {
        match self {
            Self::Rust => rust::has_doc(text),
            Self::Js => js::has_doc(text),
            Self::Python => python::has_doc(text),
            Self::Go => go::has_doc(text),
        }
    }

    /// Shown on the board and in the prompt.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Rust => "Rust",
            Self::Js => "TypeScript/JavaScript",
            Self::Python => "Python",
            Self::Go => "Go",
        }
    }

    /// What Claude Code must update when this language's files move.
    #[must_use]
    pub const fn how(self) -> &'static str {
        match self {
            Self::Rust => {
                "Rust: `a/b` is `a/b.rs`, or a folder `a/b/` with `a/b.rs` or `a/b/mod.rs`; keep each moved module's \
                 file style. Update every `mod` declaration, `use` path, `pub use` re-export, `#[path]`, \
                 `include_str!`/`include_bytes!` path, Cargo.toml (members, dependencies, features) and intra-doc link."
            }
            Self::Js => {
                "TypeScript/JavaScript: `a/b` is `a/b.ts` (or `.tsx`, `.js`, …), or a folder `a/b/` with an `index` file. \
                 Update every `import`, `export … from`, `require()` and dynamic `import()` specifier, barrel `index` \
                 files, `tsconfig.json` `paths`, and `package.json` (`main`, `exports`, `workspaces`, dependencies)."
            }
            Self::Python => {
                "Python: `a/b` is `a/b.py`, or a package folder `a/b/` with `__init__.py`. Update every absolute and \
                 relative import, `__init__.py` re-exports and `__all__`, dotted paths in strings (`mock.patch(\"a.b\")`, \
                 entry points, settings), and the package lists in `pyproject.toml`/`setup.cfg`."
            }
            Self::Go => {
                "Go: a package is a folder; `a/b` is the folder `a/b/` (or the file `a/b.go` when it is one file). Update \
                 every import path, the `package` clause when a folder's name changes, and `go.mod`/`go.work`."
            }
        }
    }

    /// Commands Claude Code may run for this language, beyond reading and editing.
    #[must_use]
    pub const fn tools(self) -> &'static [&'static str] {
        match self {
            Self::Rust => &["Bash(cargo check:*)", "Bash(cargo test:*)", "Bash(cargo nextest:*)", "Bash(cargo build:*)"],
            Self::Js => &[
                "Bash(npx tsc:*)", "Bash(npm run:*)", "Bash(npm test:*)", "Bash(pnpm:*)", "Bash(yarn:*)", "Bash(bun:*)",
                "Bash(npx eslint:*)", "Bash(npx vitest:*)", "Bash(npx jest:*)",
            ],
            Self::Python => &["Bash(python -m:*)", "Bash(python3 -m:*)", "Bash(pytest:*)", "Bash(uv run:*)", "Bash(ruff:*)", "Bash(mypy:*)"],
            Self::Go => &["Bash(go build:*)", "Bash(go vet:*)", "Bash(go test:*)", "Bash(gofmt:*)", "Bash(goimports:*)"],
        }
    }
}

/// A package: a directory with one or more manifests.
#[derive(Debug, Clone)]
pub struct Package {
    /// Its directory, repository-relative (`.` for the root).
    pub id: String,
    /// The name its manifest gives, or the directory's.
    pub name: String,
    /// The languages its manifests are for.
    pub langs: Vec<Lang>,
    /// `src` for a Rust crate: modules live there and the board skips that level.
    pub src_root: Option<String>,
    /// The name Rust code uses for the crate (`[lib] name` or the package name with `_`).
    pub rust_lib: Option<String>,
    /// `package.json`'s `name`.
    pub npm_name: Option<String>,
    /// `go.mod`'s module path.
    pub go_module: Option<String>,
    /// `tsconfig.json`/`jsconfig.json` path aliases.
    pub ts: Option<js::TsConfig>,
    /// Made up for the code outside every package found, when none is at the root.
    pub implicit: bool,
}

/// What a manifest says, as far as archboard cares.
#[derive(Debug, Clone, Default)]
pub struct Manifest {
    /// The package's name, if it gives one.
    pub name: Option<String>,
}

/// Directories never scanned: dependencies, build output, caches.
pub const SKIP_DIRS: &[&str] = &[
    "node_modules", "target", "dist", "build", "out", "vendor", "__pycache__", ".venv", "venv", ".tox", ".mypy_cache",
    ".pytest_cache", ".next", ".nuxt", ".svelte-kit", ".turbo", "coverage", ".git",
];

/// Whether `rel` (or a folder it is in) is excluded by `[scan] exclude`.
#[must_use]
pub fn excluded(config: &Config, rel: &str) -> bool {
    config.scan.exclude.iter().any(|g| glob::matches(g, rel) || ancestors_of(rel).any(|a| glob::matches(g, a)))
}

fn ancestors_of(rel: &str) -> impl Iterator<Item = &str> {
    std::iter::successors(rel.rsplit_once('/').map(|(a, _)| a), |cur| cur.rsplit_once('/').map(|(a, _)| a))
}

/// A walker over `dir` that honours `.gitignore` and skips [`SKIP_DIRS`].
pub fn walker(dir: &Path) -> ignore::WalkBuilder {
    let mut w = ignore::WalkBuilder::new(dir);
    w.hidden(true).git_ignore(true).git_exclude(true).require_git(false).parents(true).sort_by_file_name(std::cmp::Ord::cmp);
    w.filter_entry(|e| !(e.file_type().is_some_and(|t| t.is_dir()) && e.file_name().to_str().is_some_and(|n| SKIP_DIRS.contains(&n))));
    w
}

/// Repository-relative, `/`-separated, `.` for the root.
#[must_use]
pub fn rel(repo: &Path, path: &Path) -> String {
    let s = path.strip_prefix(repo).unwrap_or(path).to_string_lossy().replace('\\', "/");
    let s = s.trim_start_matches("./").trim_end_matches('/');
    if s.is_empty() { ".".into() } else { s.to_string() }
}

/// `base` joined with `segs`, where `base` may be `.`.
pub fn join(base: &str, segs: &[impl AsRef<str>]) -> String {
    let tail = segs.iter().map(AsRef::as_ref).collect::<Vec<_>>().join("/");
    match (base, tail.is_empty()) {
        (_, true) => base.to_string(),
        ("." | "", false) => tail,
        (_, false) => format!("{base}/{tail}"),
    }
}

/// The segments of a repository-relative path; none for `.`.
pub fn segments(rel: &str) -> Vec<String> {
    rel.split('/').filter(|s| !s.is_empty() && *s != ".").map(String::from).collect()
}

/// `spec` (`./a`, `../b/c`) applied to the folder `base`; `None` if it leaves the repository.
#[must_use]
pub fn normalize(mut base: Vec<String>, spec: &str) -> Option<Vec<String>> {
    for part in spec.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                base.pop()?;
            }
            p => base.push(p.to_string()),
        }
    }
    Some(base)
}

/// What import resolution looks paths up in: every node id and package.
#[derive(Debug)]
pub struct Index<'a> {
    /// Every node id.
    pub ids: &'a std::collections::HashSet<String>,
    /// Every package.
    pub packages: &'a [Package],
    /// Folders absolute Python imports start from.
    pub python_roots: Vec<String>,
}

impl Index<'_> {
    /// The deepest node along the repository path `segs`, or `None` when not
    /// even its first segment is one.
    #[must_use]
    pub fn deepest(&self, segs: &[String]) -> Option<String> {
        (1..=segs.len()).rev().map(|n| segs[..n].join("/")).find(|id| self.ids.contains(id))
    }

    /// Whether a node has this id.
    #[must_use]
    pub fn has(&self, id: &str) -> bool {
        self.ids.contains(id)
    }
}

/// Drops a source file's extension and a folder-root stem (`index`,
/// `__init__`) from the end of an import's path.
#[must_use]
pub fn strip_module_suffix(mut segs: Vec<String>, lang: Lang) -> Vec<String> {
    if let Some(last) = segs.last_mut()
        && Lang::of_file(last).is_some()
        && let Some((stem, _)) = last.rsplit_once('.')
    {
        *last = stem.to_string();
    }
    if segs.last().is_some_and(|l| lang.dir_root_stems().contains(&l.as_str())) {
        segs.pop();
    }
    segs
}

/// Line numbers by byte offset.
#[derive(Debug)]
pub struct Lines(Vec<usize>);

impl Lines {
    /// Indexes `text`'s line starts.
    #[must_use]
    pub fn new(text: &str) -> Self {
        Self(std::iter::once(0).chain(text.match_indices('\n').map(|(i, _)| i + 1)).collect())
    }

    /// The 1-based line holding byte `offset`.
    #[must_use]
    pub fn at(&self, offset: usize) -> usize {
        self.0.partition_point(|&start| start <= offset)
    }
}

/// The packages `config` names, or every one found under `repo`.
///
/// Without a package list, a repository with no package at its root gets one
/// for the code outside the others (dropped later if it holds none).
///
/// # Errors
///
/// When a manifest cannot be parsed.
pub fn discover(repo: &Path, config: &Config) -> Result<Vec<Package>> {
    let mut dirs: Vec<String> = Vec::new();
    match &config.scan.packages {
        Some(entries) => {
            for entry in entries {
                if let Some(base) = entry.strip_suffix("/*") {
                    let Ok(read) = std::fs::read_dir(repo.join(base)) else { continue };
                    let mut found: Vec<String> = read.filter_map(Result::ok).filter(|e| e.path().is_dir()).map(|e| rel(repo, &e.path())).collect();
                    found.sort();
                    dirs.extend(found);
                } else {
                    dirs.push(rel(repo, &repo.join(entry)));
                }
            }
        }
        None => {
            for entry in walker(repo).max_depth(Some(8)).build().filter_map(Result::ok) {
                if entry.file_type().is_some_and(|t| t.is_file())
                    && entry.file_name().to_str().is_some_and(|n| MANIFESTS.contains(&n))
                    && let Some(dir) = entry.path().parent()
                {
                    dirs.push(rel(repo, dir));
                }
            }
        }
    }
    dirs.sort();
    dirs.dedup();
    dirs.retain(|d| !excluded(config, d));

    let mut packages: BTreeMap<String, Package> = BTreeMap::new();
    for dir in dirs {
        if let Some(p) = read_package(repo, &dir)? {
            packages.insert(dir, p);
        }
    }
    if config.scan.packages.is_none() && !packages.contains_key(".") {
        let name = repo.canonicalize().ok().and_then(|p| p.file_name().map(|f| f.to_string_lossy().into_owned())).unwrap_or_else(|| "root".into());
        packages.insert(
            ".".into(),
            Package {
                id: ".".into(),
                name,
                langs: Lang::ALL.to_vec(),
                src_root: None,
                rust_lib: None,
                npm_name: None,
                go_module: None,
                ts: js::TsConfig::read(repo, "."),
                implicit: true,
            },
        );
    }
    Ok(packages.into_values().collect())
}

const MANIFESTS: &[&str] = &["Cargo.toml", "package.json", "go.mod", "pyproject.toml", "setup.py", "setup.cfg"];

fn read_package(repo: &Path, dir: &str) -> Result<Option<Package>> {
    let path = repo.join(dir);
    let rust = rust::read_manifest(&path)?;
    let npm = js::read_manifest(&path);
    let go_module = go::read_manifest(&path);
    let python = python::read_manifest(&path);
    let mut langs = Vec::new();
    let mut names = Vec::new();
    if let Some((name, _)) = &rust {
        langs.push(Lang::Rust);
        names.push(name.clone());
    }
    if let Some(m) = &npm {
        langs.push(Lang::Js);
        names.push(m.name.clone().unwrap_or_default());
    }
    if let Some(module) = &go_module {
        langs.push(Lang::Go);
        names.push(module.rsplit('/').next().unwrap_or(module).to_string());
    }
    if let Some(m) = &python {
        langs.push(Lang::Python);
        names.push(m.name.clone().unwrap_or_default());
    }
    if langs.is_empty() {
        return Ok(None);
    }
    let dir_name = if dir == "." {
        repo.canonicalize().ok().and_then(|p| p.file_name().map(|f| f.to_string_lossy().into_owned())).unwrap_or_else(|| "root".into())
    } else {
        dir.rsplit('/').next().unwrap_or(dir).to_string()
    };
    let name = names.into_iter().find(|n| !n.is_empty()).unwrap_or(dir_name);
    Ok(Some(Package {
        id: dir.to_string(),
        name,
        src_root: rust.is_some().then(|| "src".to_string()),
        rust_lib: rust.map(|(_, lib)| lib),
        ts: npm.is_some().then(|| js::TsConfig::read(repo, dir)).flatten(),
        npm_name: npm.and_then(|m| m.name),
        go_module,
        langs,
        implicit: false,
    }))
}

/// Commands that check a repository still builds, for the languages its
/// packages are in: what `[verify]` runs when it lists none.
#[must_use]
pub fn default_verify(repo: &Path, packages: &[(String, Vec<Lang>)]) -> Vec<String> {
    let mut commands = Vec::new();
    let has = |l: Lang| packages.iter().any(|(_, langs)| langs.contains(&l));
    if has(Lang::Rust) {
        if rust::read_manifest(repo).ok().flatten().is_some() || std::fs::read_to_string(repo.join("Cargo.toml")).is_ok_and(|t| t.contains("[workspace]")) {
            commands.push("cargo check --workspace --all-targets".to_string());
        } else {
            for (id, _) in packages.iter().filter(|(_, l)| l.contains(&Lang::Rust)) {
                commands.push(format!("cargo check --all-targets --manifest-path {id}/Cargo.toml"));
            }
        }
    }
    if has(Lang::Js) {
        let local_tsc = |dir: &str| repo.join(dir).join("node_modules/.bin/tsc").exists() || repo.join("node_modules/.bin/tsc").exists();
        for (id, _) in packages.iter().filter(|(_, l)| l.contains(&Lang::Js)) {
            if repo.join(id).join("tsconfig.json").is_file() && local_tsc(id) {
                commands.push(format!("npx --no-install tsc --noEmit -p {id}"));
            }
        }
    }
    if has(Lang::Go) {
        for (id, _) in packages.iter().filter(|(_, l)| l.contains(&Lang::Go)) {
            if repo.join(id).join("go.mod").is_file() {
                commands.push(if id == "." { "go build ./...".into() } else { format!("cd {id} && go build ./...") });
            }
        }
    }
    if has(Lang::Python) {
        let dirs: Vec<&str> = packages.iter().filter(|(_, l)| l.contains(&Lang::Python)).map(|(id, _)| id.as_str()).collect();
        commands.push(format!("python3 -m compileall -q {}", dirs.join(" ")));
    }
    commands
}

#[cfg(test)]
mod tests {
    use super::{Lines, join, normalize, segments};

    #[test]
    fn paths_join_and_normalize() {
        assert_eq!(join(".", &["src", "a"]), "src/a");
        assert_eq!(join("web", &["src"]), "web/src");
        assert_eq!(join("web", &[] as &[&str]), "web");
        assert_eq!(normalize(segments("web/src/a"), "../b/./c"), Some(segments("web/src/b/c")));
        assert_eq!(normalize(Vec::new(), "../x"), None);
    }

    #[test]
    fn lines_are_one_based() {
        let l = Lines::new("a\nbc\n\nd");
        assert_eq!((l.at(0), l.at(2), l.at(3), l.at(5), l.at(6)), (1, 2, 2, 3, 4));
    }
}
