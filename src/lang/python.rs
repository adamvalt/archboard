//! Python imports.
//!
//! `import a.b` and `from a.b import c` (relative ones too) are resolved
//! against the folders absolute imports start from: each Python package's
//! directory, its `src/`, and the repository root. A module no node matches is
//! from outside (the standard library, an installed package).

use std::path::Path;
use std::sync::LazyLock;

use regex::Regex;

use super::{Index, Lines, Manifest, segments};

static IMPORT: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?m)^[ \t]*import[ \t]+([^\n#;]+)").expect("valid regex"));
static FROM: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?m)^[ \t]*from[ \t]+([.\w]+)[ \t]+import[ \t]+(\([^)]*\)|[^\n#;]*)").expect("valid regex"));

/// `pyproject.toml` (named in `[project]` or `[tool.poetry]`), `setup.py` or
/// `setup.cfg`, if `dir` has one.
pub fn read_manifest(dir: &Path) -> Option<Manifest> {
    if let Ok(text) = std::fs::read_to_string(dir.join("pyproject.toml")) {
        let toml: toml::Value = toml::from_str(&text).unwrap_or_else(|_| toml::Value::Table(toml::Table::new()));
        let name = toml
            .get("project")
            .or_else(|| toml.get("tool").and_then(|t| t.get("poetry")))
            .and_then(|p| p.get("name"))
            .and_then(toml::Value::as_str)
            .map(String::from);
        return Some(Manifest { name });
    }
    (dir.join("setup.py").is_file() || dir.join("setup.cfg").is_file()).then(Manifest::default)
}

/// One import statement: the module and the names taken from it.
#[derive(Debug, PartialEq, Eq)]
pub struct Import {
    /// `a.b`, `.`, `..c`.
    pub module: String,
    /// What `from … import` takes; empty for `import a.b`.
    pub names: Vec<String>,
    /// 1-based.
    pub line: usize,
}

/// The import statements in `text`.
pub fn imports(text: &str) -> Vec<Import> {
    let lines = Lines::new(text);
    let mut out = Vec::new();
    for c in IMPORT.captures_iter(text) {
        let line = lines.at(c.get(0).map_or(0, |m| m.start()));
        for part in c[1].split(',') {
            let module = part.split_whitespace().next().unwrap_or_default();
            if !module.is_empty() {
                out.push(Import { module: module.to_string(), names: Vec::new(), line });
            }
        }
    }
    for c in FROM.captures_iter(text) {
        let line = lines.at(c.get(0).map_or(0, |m| m.start()));
        let names = c[2]
            .trim_matches(|ch: char| ch == '(' || ch == ')' || ch.is_whitespace())
            .split(',')
            .filter_map(|n| n.split_whitespace().next())
            .filter(|n| *n != "*" && *n != "\\")
            .map(String::from)
            .collect();
        out.push(Import { module: c[1].to_string(), names, line });
    }
    out.sort_by_key(|i| i.line);
    out
}

/// The nodes an import in the file at `file` names: one per imported name
/// that is a module, else the module itself.
pub fn resolve(import: &Import, file: &str, index: &Index) -> Vec<String> {
    let dots = import.module.chars().take_while(|c| *c == '.').count();
    let rest: Vec<String> = import.module[dots..].split('.').filter(|s| !s.is_empty()).map(String::from).collect();
    let base = if dots > 0 {
        let mut dir = segments(file);
        dir.pop();
        for _ in 1..dots {
            if dir.pop().is_none() {
                return Vec::new();
            }
        }
        dir
    } else {
        let Some(first) = rest.first() else { return Vec::new() };
        let Some(root) = index.python_roots.iter().find(|r| index.has(&super::join(r, &[first]))) else { return Vec::new() };
        segments(root)
    };
    let mut module = base;
    module.extend(rest);
    let mut targets: Vec<String> = if import.names.is_empty() {
        index.deepest(&module).into_iter().collect()
    } else {
        import
            .names
            .iter()
            .filter_map(|n| {
                let mut path = module.clone();
                path.push(n.clone());
                index.deepest(&path)
            })
            .collect()
    };
    targets.sort();
    targets.dedup();
    targets
}

/// A module docstring before any statement.
pub fn has_doc(text: &str) -> bool {
    text.lines()
        .map(str::trim)
        .find(|l| !l.is_empty() && !l.starts_with('#'))
        .is_some_and(|l| ["\"\"\"", "'''", "r\"\"\"", "\"", "'"].iter().any(|q| l.starts_with(q)))
}

#[cfg(test)]
mod tests {
    use super::{Import, imports};

    #[test]
    fn finds_imports() {
        let text = "\"\"\"Doc.\"\"\"\nimport os, app.core as c\nfrom . import util\nfrom ..models import (\n    User,\n    Group,\n)\n    from app.x import y  # local\n";
        assert_eq!(
            imports(text),
            [
                Import { module: "os".into(), names: vec![], line: 2 },
                Import { module: "app.core".into(), names: vec![], line: 2 },
                Import { module: ".".into(), names: vec!["util".into()], line: 3 },
                Import { module: "..models".into(), names: vec!["User".into(), "Group".into()], line: 4 },
                Import { module: "app.x".into(), names: vec!["y".into()], line: 8 },
            ]
        );
    }
}
