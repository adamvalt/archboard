//! TypeScript and JavaScript imports.
//!
//! `import`/`export … from`, `require()` and dynamic `import()` specifiers are
//! resolved relative to the file, through `tsconfig.json` `paths` and
//! `baseUrl`, or to a workspace package by its `package.json` name. Anything
//! else is a dependency from outside.

use std::path::Path;
use std::sync::LazyLock;

use regex::Regex;

use super::{Index, Lang, Lines, Manifest, Package, normalize, segments, strip_module_suffix};

static FROM: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"(?:^|[\s;}])(?:import|export)\s[^'";]*?\bfrom\s*['"]([^'"\n]+)['"]"#).expect("valid regex"));
static BARE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r#"(?:^|[\s;])import\s*['"]([^'"\n]+)['"]"#).expect("valid regex"));
static CALL: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"\b(?:require|import)\s*\(\s*['"]([^'"\n]+)['"]\s*\)"#).expect("valid regex"));

static TRAILING_COMMA: LazyLock<Regex> = LazyLock::new(|| Regex::new(r",(\s*[}\]])").expect("valid regex"));

/// `package.json`, if `dir` has one.
pub fn read_manifest(dir: &Path) -> Option<Manifest> {
    let text = std::fs::read_to_string(dir.join("package.json")).ok()?;
    let json: serde_json::Value = serde_json::from_str(&text).unwrap_or_default();
    Some(Manifest { name: json.get("name").and_then(serde_json::Value::as_str).map(String::from) })
}

/// The module specifiers in `text`, with their lines.
#[must_use]
pub fn imports(text: &str) -> Vec<(String, usize)> {
    let lines = Lines::new(text);
    let mut out: Vec<(usize, String)> = [&*FROM, &*BARE, &*CALL]
        .iter()
        .flat_map(|re| re.captures_iter(text).filter_map(|c| c.get(1)).map(|m| (m.start(), m.as_str().to_string())))
        .collect();
    out.sort();
    out.dedup();
    out.into_iter().map(|(at, spec)| (spec, lines.at(at))).collect()
}

/// The node `spec`, imported from the file at `file`, names.
#[must_use]
pub fn resolve(spec: &str, file: &str, pkg: &Package, index: &Index) -> Option<String> {
    let segs = if spec.starts_with('.') {
        let mut dir = segments(file);
        dir.pop();
        normalize(dir, spec)?
    } else if let Some(segs) = pkg.ts.as_ref().and_then(|ts| ts.alias(spec)) {
        segs
    } else if let Some((p, rest)) = index.packages.iter().find_map(|p| {
        let name = p.npm_name.as_deref()?;
        let rest = spec.strip_prefix(name)?;
        (rest.is_empty() || rest.starts_with('/')).then_some((p, rest))
    }) {
        normalize(segments(&p.id), rest)?
    } else {
        // A bare specifier under `baseUrl` only counts when it names something there.
        let base = pkg.ts.as_ref().and_then(|ts| ts.base_url.clone())?;
        let floor = base.len();
        let segs = strip_module_suffix(normalize(base, spec)?, Lang::Js);
        return index.deepest(&segs).filter(|id| segments(id).len() > floor);
    };
    index.deepest(&strip_module_suffix(segs, Lang::Js))
}

/// `compilerOptions.baseUrl` and `paths` from a package's `tsconfig.json` or
/// `jsconfig.json`.
#[derive(Debug, Clone, Default)]
pub struct TsConfig {
    /// `baseUrl` as repository path segments, when set.
    pub base_url: Option<Vec<String>>,
    /// `paths`: (prefix before `*`, suffix after it or `None` with no `*`,
    /// targets as repository paths with the `*` kept).
    pub paths: Vec<(String, Option<String>, Vec<String>)>,
}

impl TsConfig {
    /// Reads the config in `dir`, if it has one with aliases.
    pub fn read(repo: &Path, dir: &str) -> Option<Self> {
        let text = ["tsconfig.json", "jsconfig.json"].iter().find_map(|f| std::fs::read_to_string(repo.join(dir).join(f)).ok())?;
        let json: serde_json::Value = serde_json::from_str(&strip_jsonc(&text)).ok()?;
        let options = json.get("compilerOptions")?;
        let dir_segs = segments(dir);
        let base_url = options.get("baseUrl").and_then(serde_json::Value::as_str).and_then(|b| normalize(dir_segs.clone(), b));
        let target_base = base_url.clone().unwrap_or_else(|| dir_segs.clone());
        let paths = options
            .get("paths")
            .and_then(serde_json::Value::as_object)
            .into_iter()
            .flatten()
            .map(|(pattern, targets)| {
                let (prefix, suffix) = match pattern.split_once('*') {
                    Some((p, s)) => (p.to_string(), Some(s.to_string())),
                    None => (pattern.clone(), None),
                };
                let targets = targets
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(serde_json::Value::as_str)
                    .filter_map(|t| normalize(target_base.clone(), t).map(|s| s.join("/")))
                    .collect();
                (prefix, suffix, targets)
            })
            .collect::<Vec<_>>();
        (base_url.is_some() || !paths.is_empty()).then_some(Self { base_url, paths })
    }

    /// The repository path an aliased specifier stands for (its first target).
    #[must_use]
    pub fn alias(&self, spec: &str) -> Option<Vec<String>> {
        self.paths
            .iter()
            .filter_map(|(prefix, suffix, targets)| {
                let target = targets.first()?;
                match suffix {
                    None => (spec == prefix).then(|| target.clone()),
                    Some(suffix) => {
                        let middle = spec.strip_prefix(prefix.as_str())?.strip_suffix(suffix.as_str())?;
                        Some(target.replacen('*', middle, 1))
                    }
                }
                .map(|t| (prefix.len(), t))
            })
            .max_by_key(|(len, _)| *len)
            .map(|(_, t)| segments(&t))
    }
}

/// JSON with comments and trailing commas made plain JSON.
fn strip_jsonc(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    let mut in_string = false;
    while let Some(c) = chars.next() {
        if in_string {
            out.push(c);
            if c == '\\' {
                out.extend(chars.next());
            } else if c == '"' {
                in_string = false;
            }
            continue;
        }
        match (c, chars.peek()) {
            ('"', _) => {
                in_string = true;
                out.push(c);
            }
            ('/', Some('/')) => {
                for n in chars.by_ref() {
                    if n == '\n' {
                        out.push('\n');
                        break;
                    }
                }
            }
            ('/', Some('*')) => {
                chars.next();
                let mut prev = ' ';
                for n in chars.by_ref() {
                    if prev == '*' && n == '/' {
                        break;
                    }
                    prev = n;
                }
            }
            _ => out.push(c),
        }
    }
    TRAILING_COMMA.replace_all(&out, "$1").into_owned()
}

/// A leading `/** … */` block or `//` comment that is not a tool directive.
pub fn has_doc(text: &str) -> bool {
    let first = text.lines().map(str::trim).find(|l| !l.is_empty() && !l.starts_with("#!") && *l != "\"use strict\";" && *l != "'use client';");
    first.is_some_and(|l| {
        let directive = ["eslint", "@ts-", "prettier", "#region", "biome-ignore", "istanbul"].iter().any(|d| l.contains(d));
        (l.starts_with("/*") || l.starts_with("//")) && !directive
    })
}

#[cfg(test)]
mod tests {
    use super::{TsConfig, imports, strip_jsonc};

    #[test]
    fn finds_every_kind_of_import() {
        let text = "import a from './a';\nimport {\n  b,\n  c,\n} from \"../b\";\nexport * from './c'\nimport './side.css';\nconst d = require('./d');\nconst e = await import('./e');\nimport type { F } from '@/f';\n";
        let specs: Vec<(String, usize)> = imports(text);
        let names: Vec<&str> = specs.iter().map(|(s, _)| s.as_str()).collect();
        assert_eq!(names, ["./a", "../b", "./c", "./side.css", "./d", "./e", "@/f"]);
        assert_eq!(specs[1].1, 5);
    }

    #[test]
    fn reads_tsconfig_aliases_with_comments() {
        let text = "{\n  // comment\n  \"compilerOptions\": { \"baseUrl\": \".\", /* x */ \"paths\": { \"@/*\": [\"./src/*\"], }, },\n}";
        let json: serde_json::Value = serde_json::from_str(&strip_jsonc(text)).expect("parses");
        assert_eq!(json["compilerOptions"]["paths"]["@/*"][0], "./src/*");
        let ts = TsConfig { base_url: None, paths: vec![("@/".into(), Some(String::new()), vec!["web/src/*".into()])] };
        assert_eq!(ts.alias("@/lib/x"), Some(vec!["web".into(), "src".into(), "lib".into(), "x".into()]));
        assert_eq!(ts.alias("react"), None);
    }
}
