//! Go: import paths under a `go.mod` module in the repository name the folder
//! below that module's directory; every other import is from outside.

use std::path::Path;
use std::sync::LazyLock;

use regex::Regex;

use super::{Index, Lines, normalize, segments};

static SINGLE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r#"(?m)^import[ \t]+(?:[\w.]+[ \t]+)?"([^"]+)""#).expect("valid regex"));
static BLOCK: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?ms)^import[ \t]*\((.*?)\)").expect("valid regex"));
static QUOTED: LazyLock<Regex> = LazyLock::new(|| Regex::new(r#""([^"]+)""#).expect("valid regex"));

/// `go.mod`'s module path.
pub fn read_manifest(dir: &Path) -> Option<String> {
    let text = std::fs::read_to_string(dir.join("go.mod")).ok()?;
    text.lines().find_map(|l| l.trim().strip_prefix("module ")).map(|m| m.trim().trim_matches('"').to_string())
}

/// The import paths in `text`, with their lines.
pub fn imports(text: &str) -> Vec<(String, usize)> {
    let lines = Lines::new(text);
    let mut out: Vec<(String, usize)> =
        SINGLE.captures_iter(text).filter_map(|c| c.get(1)).map(|m| (m.as_str().to_string(), lines.at(m.start()))).collect();
    for block in BLOCK.captures_iter(text).filter_map(|c| c.get(1)) {
        out.extend(QUOTED.captures_iter(block.as_str()).filter_map(|c| c.get(1)).map(|m| (m.as_str().to_string(), lines.at(block.start() + m.start()))));
    }
    out.sort_by_key(|(_, l)| *l);
    out
}

/// The node an import path names.
pub fn resolve(path: &str, index: &Index) -> Option<String> {
    let (pkg, rest) = index.packages.iter().find_map(|p| {
        let module = p.go_module.as_deref()?;
        let rest = path.strip_prefix(module)?;
        (rest.is_empty() || rest.starts_with('/')).then_some((p, rest))
    })?;
    index.deepest(&normalize(segments(&pkg.id), rest)?).or_else(|| Some(pkg.id.clone()))
}

/// A `// Package …` (or `/* Package …`) comment before the package clause.
pub fn has_doc(text: &str) -> bool {
    text.lines()
        .map(str::trim)
        .take_while(|l| !l.starts_with("package "))
        .any(|l| l.starts_with("// Package ") || l.starts_with("/* Package ") || l.starts_with("Package "))
}

#[cfg(test)]
mod tests {
    use super::imports;

    #[test]
    fn finds_single_and_block_imports() {
        let text = "// Package a does things.\npackage a\n\nimport \"fmt\"\nimport (\n\t\"example.com/m/b\"\n\tc \"example.com/m/c\"\n)\n";
        assert_eq!(imports(text), [("fmt".into(), 4), ("example.com/m/b".into(), 6), ("example.com/m/c".into(), 7)]);
        assert!(super::has_doc(text));
    }
}
