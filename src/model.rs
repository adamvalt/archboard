//! The scanned codebase: a tree of nodes (workspace → packages → folders and
//! files) and the dependency edges between them, read from the imports in
//! each language's source ([`crate::lang`]).
//!
//! A node's id is its repository-relative path, without the extension for a
//! file (`src/plugins/viewer`, `web/src/api/client`); a package's id is its
//! directory (`.` for one at the root) and the workspace is [`WORKSPACE`]. A
//! file that stands for its folder (`mod.rs`, `index.ts`, `__init__.py`) is
//! that folder's root file rather than a node, and a Rust crate's `src/` is
//! skipped, so its modules sit right under the crate.

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::path::Path;

use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::config::Config;
use crate::glob;
use crate::lang::{self, Index, Lang, Package, join, rust};

/// The id of the workspace node, the root of the tree.
pub const WORKSPACE: &str = "@";

/// What a node is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NodeKind {
    /// The root.
    Workspace,
    /// A directory with a manifest: a Cargo crate, an npm package, a Go module, a Python project.
    Package,
    /// A folder.
    Dir,
    /// A single file.
    File,
}

/// A package, folder or file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Node {
    /// Repository-relative path, see the module doc.
    pub id: String,
    /// The folder, file or package name.
    pub name: String,
    /// What it is.
    pub kind: NodeKind,
    /// The containing node; `None` only for the workspace.
    pub parent: Option<String>,
    /// The file that stands for it (a crate root, `mod.rs`, `index.ts`, `__init__.py`, the file itself), if any.
    pub root_file: Option<String>,
    /// Lines of source in this node and everything inside it.
    pub loc: usize,
    /// Lines of the longest single file in it.
    pub max_file_loc: usize,
    /// Whether it says what it is for: its root file opens with a doc, or its folder has a README.
    pub has_doc: bool,
    /// The layer the architecture rules put it in, if any.
    pub layer: Option<String>,
    /// The languages of the source in it.
    #[serde(default)]
    pub langs: Vec<Lang>,
    /// For a package whose modules live in a subfolder the board skips (a Rust crate's `src`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub src_root: Option<String>,
}

impl Node {
    const fn new(id: String, name: String, kind: NodeKind, parent: Option<String>) -> Self {
        Self { id, name, kind, parent, root_file: None, loc: 0, max_file_loc: 0, has_doc: false, layer: None, langs: Vec::new(), src_root: None }
    }
}

/// Uses of one node by another, at the deepest nodes the imports resolve to.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Edge {
    /// The node whose code holds the imports.
    pub from: String,
    /// The node they resolve to.
    pub to: String,
    /// How many imports or paths.
    pub count: usize,
    /// Up to three `file:line` locations.
    pub samples: Vec<String>,
}

/// The scanned codebase.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Model {
    /// Every node, keyed by id.
    pub nodes: BTreeMap<String, Node>,
    /// Every dependency edge.
    pub edges: Vec<Edge>,
    /// Files that did not parse.
    pub warnings: Vec<String>,
}

impl Model {
    /// The chain of ids from `id` up to the workspace, `id` first.
    pub fn ancestors<'a>(&'a self, id: &'a str) -> impl Iterator<Item = &'a str> + 'a {
        std::iter::successors(Some(id), |cur| self.nodes.get(*cur).and_then(|n| n.parent.as_deref()))
    }

    /// Whether `inner` is `outer` or lies inside it.
    #[must_use]
    pub fn is_within(&self, inner: &str, outer: &str) -> bool {
        self.ancestors(inner).any(|a| a == outer)
    }

    /// Every package's id and languages.
    #[must_use]
    pub fn packages(&self) -> Vec<(String, Vec<Lang>)> {
        self.nodes.values().filter(|n| n.kind == NodeKind::Package).map(|n| (n.id.clone(), n.langs.clone())).collect()
    }

    /// Every language in the codebase.
    pub fn langs(&self) -> Vec<Lang> {
        self.nodes.get(WORKSPACE).map_or_else(Vec::new, |n| n.langs.clone())
    }
}

/// One source file and where it sits in the tree.
struct SourceFile {
    pkg: usize,
    lang: Lang,
    /// Repository-relative path.
    path: String,
    /// The node it belongs to (itself, its folder or its package).
    owner: String,
    /// Its module path below the package's `src_root` (Rust only).
    module: Vec<String>,
}

/// Scans the packages `config` names (or finds them) under `repo`.
///
/// # Errors
///
/// When a package's manifest cannot be parsed.
#[expect(clippy::too_many_lines, reason = "the file walk, the import pass and the totals share their indexes")]
pub fn scan(repo: &Path, config: &Config) -> Result<Model> {
    let packages = lang::discover(repo, config)?;
    let package_dirs: HashSet<&str> = packages.iter().map(|p| p.id.as_str()).collect();

    let mut nodes = BTreeMap::new();
    let ws_name = repo.canonicalize().ok().and_then(|p| p.file_name().map(|f| f.to_string_lossy().into_owned())).unwrap_or_else(|| "workspace".into());
    nodes.insert(WORKSPACE.to_string(), Node::new(WORKSPACE.into(), ws_name, NodeKind::Workspace, None));

    let mut files: Vec<SourceFile> = Vec::new();
    let mut readme_dirs: HashSet<String> = HashSet::new();
    for (i, pkg) in packages.iter().enumerate() {
        let mut node = Node::new(pkg.id.clone(), pkg.name.clone(), NodeKind::Package, Some(WORKSPACE.into()));
        node.src_root.clone_from(&pkg.src_root);
        nodes.insert(pkg.id.clone(), node);
        // A crate that is only Rust keeps its modules in `src/`; nothing else needs walking.
        let walk_root = match (&pkg.src_root, pkg.langs.as_slice()) {
            (Some(src), [Lang::Rust]) => join(&pkg.id, &[src]),
            _ => pkg.id.clone(),
        };
        let walk_dir = repo.join(&walk_root);
        if !walk_dir.is_dir() {
            continue;
        }
        let pkg_id = pkg.id.clone();
        let nested: HashSet<String> = package_dirs.iter().filter(|d| **d != pkg_id).map(|d| (*d).to_string()).collect();
        let mut walker = lang::walker(&walk_dir);
        let repo_owned = repo.to_path_buf();
        let config_owned = config.clone();
        walker.filter_entry(move |e| {
            let rel = lang::rel(&repo_owned, e.path());
            let is_dir = e.file_type().is_some_and(|t| t.is_dir());
            let skipped_dir = is_dir && e.file_name().to_str().is_some_and(|n| lang::SKIP_DIRS.contains(&n));
            !(skipped_dir || (is_dir && nested.contains(&rel)) || lang::excluded(&config_owned, &rel))
        });
        for entry in walker.build().filter_map(Result::ok) {
            if !entry.file_type().is_some_and(|t| t.is_file()) {
                continue;
            }
            let path = lang::rel(repo, entry.path());
            let file_name = entry.file_name().to_string_lossy();
            if file_name.eq_ignore_ascii_case("readme.md") || file_name.eq_ignore_ascii_case("readme") {
                readme_dirs.insert(path.rsplit_once('/').map_or(".", |(d, _)| d).to_string());
                continue;
            }
            let Some(lang) = Lang::of_file(&path).filter(|l| pkg.langs.contains(l)) else { continue };
            if let Some(file) = place(repo, &mut nodes, pkg, i, lang, &path) {
                files.push(file);
            }
        }
    }

    let mut warnings = Vec::new();
    let mut refs: Vec<(String, String, String)> = Vec::new();
    let mut file_stats: Vec<(String, usize, Lang)> = Vec::new();
    let ids: HashSet<String> = nodes.keys().cloned().collect();
    let index = Index { ids: &ids, packages: &packages, python_roots: python_roots(&packages, &ids) };
    let rust_index = rust_module_index(&packages, &files);
    let lib_names: HashMap<String, String> = packages.iter().filter_map(|p| Some((p.rust_lib.clone()?, p.id.clone()))).collect();
    let mut rust_scan = rust::Scan::new(&rust_index, lib_names);

    for f in &files {
        let Ok(text) = std::fs::read_to_string(repo.join(&f.path)) else {
            warnings.push(format!("{}: not UTF-8", f.path));
            continue;
        };
        file_stats.push((f.owner.clone(), text.lines().count(), f.lang));
        mark_doc(&mut nodes, f, &text);
        let pkg = &packages[f.pkg];
        match f.lang {
            Lang::Rust => {
                if let Err(e) = rust_scan.file(&pkg.id, &f.owner, &f.module, &f.path, &text) {
                    warnings.push(format!("{}: {e}", f.path));
                }
            }
            Lang::Js => refs.extend(
                lang::js::imports(&text)
                    .into_iter()
                    .filter_map(|(spec, line)| Some((f.owner.clone(), lang::js::resolve(&spec, &f.path, pkg, &index)?, format!("{}:{line}", f.path)))),
            ),
            Lang::Python => {
                for import in lang::python::imports(&text) {
                    let at = format!("{}:{}", f.path, import.line);
                    refs.extend(lang::python::resolve(&import, &f.path, &index).into_iter().map(|t| (f.owner.clone(), t, at.clone())));
                }
            }
            Lang::Go => refs.extend(
                lang::go::imports(&text)
                    .into_iter()
                    .filter_map(|(path, line)| Some((f.owner.clone(), lang::go::resolve(&path, &index)?, format!("{}:{line}", f.path)))),
            ),
        }
    }
    refs.extend(rust_scan.finish());

    for dir in readme_dirs {
        if let Some(n) = nodes.get_mut(&dir).filter(|n| matches!(n.kind, NodeKind::Dir | NodeKind::Package)) {
            n.has_doc = true;
        }
    }
    for (owner, loc, file_lang) in &file_stats {
        let chain: Vec<String> = std::iter::successors(Some(owner.clone()), |c| nodes.get(c).and_then(|n| n.parent.clone())).collect();
        for a in chain {
            if let Some(n) = nodes.get_mut(&a) {
                n.loc += loc;
                n.max_file_loc = n.max_file_loc.max(*loc);
                if !n.langs.contains(file_lang) {
                    n.langs.push(*file_lang);
                    n.langs.sort();
                }
            }
        }
    }
    // A package found without a manifest (the root's leftovers) that holds no code is noise.
    let empty_implicit: Vec<String> =
        nodes.values().filter(|n| n.kind == NodeKind::Package && n.loc == 0 && n.langs.is_empty() && packages.iter().any(|p| p.id == n.id && p.implicit)).map(|n| n.id.clone()).collect();
    for id in empty_implicit {
        nodes.remove(&id);
    }

    let mut raw: BTreeMap<(String, String), (usize, Vec<String>)> = BTreeMap::new();
    for (from, to, at) in refs {
        let entry = raw.entry((from, to)).or_default();
        entry.0 += 1;
        if entry.1.len() < 3 && !entry.1.contains(&at) {
            entry.1.push(at);
        }
    }
    let mut model = Model { nodes, edges: Vec::new(), warnings };
    model.edges = raw
        .into_iter()
        .filter(|((from, to), _)| model.nodes.contains_key(to) && !model.is_within(from, to) && !model.is_within(to, from))
        .map(|((from, to), (count, samples))| Edge { from, to, count, samples })
        .collect();
    assign_layers(&mut model, config);
    Ok(model)
}

/// Adds the nodes for one source file and says which it belongs to.
fn place(repo: &Path, nodes: &mut BTreeMap<String, Node>, pkg: &Package, pkg_index: usize, lang: Lang, path: &str) -> Option<SourceFile> {
    let in_pkg = lang::segments(path.strip_prefix(&format!("{}/", pkg.id)).unwrap_or(if pkg.id == "." { path } else { "" }));
    let (base, mut segs) = match &pkg.src_root {
        Some(src) if in_pkg.first() == Some(src) => (join(&pkg.id, &[src]), in_pkg[1..].to_vec()),
        // Rust outside `src/` (tests, benches, build scripts) is not module code.
        _ if lang == Lang::Rust => return None,
        _ => (pkg.id.clone(), in_pkg),
    };
    let file_name = segs.pop()?;
    let stem = file_name.rsplit_once('.').map_or(file_name.as_str(), |(s, _)| s).to_string();
    let is_root = lang.dir_root_stems().contains(&stem.as_str()) || (segs.is_empty() && lang.package_root_stems().contains(&stem.as_str()));
    if !is_root {
        segs.push(stem);
    }
    let mut parent = pkg.id.clone();
    for depth in 1..=segs.len() {
        let node_id = join(&base, &segs[..depth]);
        let leaf = depth == segs.len();
        let kind = if leaf && !is_root && !repo.join(&node_id).is_dir() { NodeKind::File } else { NodeKind::Dir };
        let entry = nodes.entry(node_id.clone()).or_insert_with(|| Node::new(node_id.clone(), segs[depth - 1].clone(), kind, Some(parent.clone())));
        if leaf && (is_root || entry.root_file.is_none()) {
            entry.root_file = Some(path.to_string());
        }
        parent = node_id;
    }
    if segs.is_empty()
        && let Some(p) = nodes.get_mut(&pkg.id)
        && p.root_file.is_none()
    {
        p.root_file = Some(path.to_string());
    }
    let owner = if segs.is_empty() { pkg.id.clone() } else { join(&base, &segs) };
    Some(SourceFile { pkg: pkg_index, lang, path: path.to_string(), owner, module: segs })
}

/// Records a file's doc on the node it stands for (for Go, the folder its package is).
fn mark_doc(nodes: &mut BTreeMap<String, Node>, f: &SourceFile, text: &str) {
    if !f.lang.has_doc(text) {
        return;
    }
    let target = if f.lang == Lang::Go { nodes.get(&f.owner).and_then(|n| if n.kind == NodeKind::File { n.parent.clone() } else { Some(n.id.clone()) }) } else { Some(f.owner.clone()) };
    if let Some(n) = target.and_then(|t| nodes.get_mut(&t)) {
        if f.lang == Lang::Go && n.root_file.is_none() {
            n.root_file = Some(f.path.clone());
        }
        if n.root_file.as_deref() == Some(f.path.as_str()) || f.lang == Lang::Go {
            n.has_doc = true;
        }
    }
}

/// (crate id, module segments) → node id, for Rust path resolution.
fn rust_module_index(packages: &[Package], files: &[SourceFile]) -> HashMap<rust::AbsPath, String> {
    let mut index = HashMap::new();
    for p in packages.iter().filter(|p| p.langs.contains(&Lang::Rust)) {
        index.insert((p.id.clone(), Vec::new()), p.id.clone());
    }
    for f in files.iter().filter(|f| f.lang == Lang::Rust) {
        let pkg = &packages[f.pkg];
        let base = join(&pkg.id, &[pkg.src_root.as_deref().unwrap_or("src")]);
        for depth in 1..=f.module.len() {
            index.insert((pkg.id.clone(), f.module[..depth].to_vec()), join(&base, &f.module[..depth]));
        }
    }
    index
}

/// Where absolute Python imports start: each Python package's directory and
/// its `src/`, then the repository root.
fn python_roots(packages: &[Package], ids: &HashSet<String>) -> Vec<String> {
    let mut roots: BTreeSet<String> = BTreeSet::new();
    for p in packages.iter().filter(|p| p.langs.contains(&Lang::Python)) {
        let src = join(&p.id, &["src"]);
        if ids.contains(&src) {
            roots.insert(src);
        }
        roots.insert(p.id.clone());
    }
    roots.insert(".".into());
    // Deeper roots first, so `src/app` wins over the repository root's `src`.
    let mut v: Vec<String> = roots.into_iter().collect();
    v.sort_by_key(|r| std::cmp::Reverse(lang::segments(r).len()));
    v
}

/// `[verify] commands`, or the usual checks for the codebase's languages when unset.
#[must_use]
pub fn verify_commands(config: &Config, repo: &Path, model: &Model) -> Vec<String> {
    config.verify.commands.clone().unwrap_or_else(|| lang::default_verify(repo, &model.packages()))
}

/// Puts each node in the layer whose most specific pattern matches it or its
/// nearest ancestor.
pub fn assign_layers(model: &mut Model, config: &Config) {
    let layer_of = |id: &str| -> Option<String> {
        config
            .layers
            .iter()
            .flat_map(|l| l.paths.iter().map(move |p| (l, p)))
            .filter(|(_, p)| glob::matches(p, id))
            .max_by_key(|(_, p)| glob::specificity(p))
            .map(|(l, _)| l.name.clone())
    };
    let ids: Vec<String> = model.nodes.keys().cloned().collect();
    for id in ids {
        let layer = model.ancestors(&id).find_map(layer_of);
        if let Some(n) = model.nodes.get_mut(&id) {
            n.layer = layer;
        }
    }
}

/// The path a child named `name` of package or folder `parent` (at
/// `parent_path`) has.
#[must_use]
pub fn child_path(parent_path: &str, parent_src_root: Option<&str>, name: &str) -> String {
    parent_src_root.map_or_else(|| join(parent_path, &[name]), |src| join(parent_path, &[src, name]))
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::{NodeKind, scan};
    use crate::config::Config;

    fn write(root: &Path, files: &[(&str, &str)]) {
        for (path, text) in files {
            let p = root.join(path);
            std::fs::create_dir_all(p.parent().expect("has a parent")).expect("mkdir");
            std::fs::write(p, text).expect("write");
        }
    }

    fn edges(root: &Path) -> Vec<(String, String)> {
        let m = scan(root, &Config::default()).expect("scans");
        m.edges.into_iter().map(|e| (e.from, e.to)).collect()
    }

    #[test]
    fn a_typescript_monorepo() {
        let dir = tempfile::tempdir().expect("tempdir");
        write(
            dir.path(),
            &[
                ("package.json", r#"{"name": "root", "workspaces": ["packages/*"]}"#),
                ("packages/core/package.json", r#"{"name": "@acme/core"}"#),
                ("packages/core/src/index.ts", "export * from './math';\n"),
                ("packages/core/src/math.ts", "/** Sums. */\nexport const add = (a: number, b: number) => a + b;\n"),
                ("packages/web/package.json", r#"{"name": "web"}"#),
                ("packages/web/tsconfig.json", "{ \"compilerOptions\": { \"paths\": { \"@/*\": [\"./src/*\"] } } }"),
                ("packages/web/src/app.tsx", "import { add } from '@acme/core';\nimport { fmt } from '@/lib/fmt';\n"),
                ("packages/web/src/lib/fmt.ts", "import { add } from '@acme/core/src/math';\nexport const fmt = String;\n"),
                ("packages/web/node_modules/x/index.js", "import '../../src/app';\n"),
            ],
        );
        let m = scan(dir.path(), &Config::default()).expect("scans");
        assert_eq!(m.nodes["packages/core"].kind, NodeKind::Package);
        assert_eq!(m.nodes["packages/core/src"].root_file.as_deref(), Some("packages/core/src/index.ts"));
        assert!(m.nodes["packages/core/src/math"].has_doc);
        assert!(!m.nodes.contains_key("packages/web/node_modules"));
        let e = edges(dir.path());
        for want in [
            ("packages/web/src/app", "packages/core"),
            ("packages/web/src/app", "packages/web/src/lib/fmt"),
            ("packages/web/src/lib/fmt", "packages/core/src/math"),
        ] {
            assert!(e.contains(&(want.0.into(), want.1.into())), "{want:?} not in {e:?}");
        }
    }

    #[test]
    fn a_python_project_with_a_src_layout() {
        let dir = tempfile::tempdir().expect("tempdir");
        write(
            dir.path(),
            &[
                ("pyproject.toml", "[project]\nname = \"shop\"\n"),
                ("src/shop/__init__.py", "\"\"\"The shop.\"\"\"\n"),
                ("src/shop/models.py", "import os\n"),
                ("src/shop/api/__init__.py", ""),
                ("src/shop/api/views.py", "from ..models import Order\nfrom shop import models\n"),
                ("tests/test_views.py", "from shop.api import views\n"),
            ],
        );
        let m = scan(dir.path(), &Config::default()).expect("scans");
        assert_eq!(m.nodes["."].name, "shop");
        assert!(m.nodes["src/shop"].has_doc);
        let e = edges(dir.path());
        assert!(e.contains(&("src/shop/api/views".into(), "src/shop/models".into())), "{e:?}");
        assert!(e.contains(&("tests/test_views".into(), "src/shop/api/views".into())), "{e:?}");
    }

    #[test]
    fn a_go_module_and_a_crate_side_by_side() {
        let dir = tempfile::tempdir().expect("tempdir");
        write(
            dir.path(),
            &[
                ("svc/go.mod", "module example.com/svc\n\ngo 1.22\n"),
                ("svc/main.go", "package main\n\nimport (\n\t\"fmt\"\n\t\"example.com/svc/internal/store\"\n)\n"),
                ("svc/internal/store/store.go", "// Package store keeps things.\npackage store\n"),
                ("engine/Cargo.toml", "[package]\nname = \"my-engine\"\n"),
                ("engine/src/lib.rs", "//! The engine.\npub mod a;\npub mod b;\n"),
                ("engine/src/a.rs", "use crate::b::Thing;\n"),
                ("engine/src/b.rs", "pub struct Thing;\n"),
                ("scripts/tool.py", "print(1)\n"),
            ],
        );
        let m = scan(dir.path(), &Config::default()).expect("scans");
        assert!(m.nodes["svc/internal/store"].has_doc);
        assert!(m.nodes["engine"].has_doc);
        // Code outside every package gets a package for the root.
        assert_eq!(m.nodes["scripts/tool"].parent.as_deref(), Some("scripts"));
        let e = edges(dir.path());
        assert!(e.contains(&("svc/main".into(), "svc/internal/store".into())), "{e:?}");
        assert!(e.contains(&("engine/src/a".into(), "engine/src/b".into())), "{e:?}");
    }
}
