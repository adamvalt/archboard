//! A plan: the edits made on the board (move, rename, create, note), what
//! they mean for every node's path, and the planned [`Model`] the architecture
//! rules are checked against before anything is changed.

use std::collections::{BTreeMap, HashMap};
use std::fmt::Write as _;

use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};

use crate::config::Config;
use crate::lang::Lang;
use crate::model::{self, Edge, Model, Node, NodeKind, WORKSPACE};
use crate::rules::{self, Violation};

/// One edit on the board. Node ids are always the scanned ids (or a created
/// node's `new:…` id), never planned paths, so edits compose.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum Op {
    /// Put `node` inside `to`.
    Move {
        /// The node moved.
        node: String,
        /// Its new parent.
        to: String,
    },
    /// Give `node` a new name.
    Rename {
        /// The node renamed.
        node: String,
        /// The new module or package name.
        name: String,
    },
    /// A new, empty folder (or a package, under the workspace).
    Create {
        /// Its id, `new:<n>`.
        id: String,
        /// Where it goes.
        parent: String,
        /// Its name.
        name: String,
    },
    /// Free-text instruction about a node for Claude Code.
    Note {
        /// The node.
        node: String,
        /// What to do.
        text: String,
    },
}

/// A plan as the board sends it.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Plan {
    /// Edits in the order they were made.
    pub ops: Vec<Op>,
    /// Instructions for the whole change.
    #[serde(default)]
    pub instructions: String,
}

/// What one node becomes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Change {
    /// The scanned id (or `new:…`).
    pub id: String,
    /// Its path now (`None` when created).
    pub from: Option<String>,
    /// Its path after the plan.
    pub to: String,
    /// The node's kind after the plan.
    pub kind: NodeKind,
    /// `move`, `rename`, `create`, `extract` (a folder or file becoming a
    /// package) or `inline` (a package becoming a folder).
    pub action: String,
}

/// A plan worked out against the scanned model.
#[derive(Debug, Clone, Serialize)]
pub struct Resolved {
    /// Top-most changed nodes; their contents move with them.
    pub changes: Vec<Change>,
    /// Every node's planned path, by scanned id.
    pub paths: BTreeMap<String, String>,
    /// The planned codebase.
    #[serde(skip)]
    pub planned: Model,
    /// Violations of the planned codebase that the current one does not have.
    pub added: Vec<Violation>,
    /// Current violations the plan removes.
    pub removed: Vec<Violation>,
    /// All violations of the planned codebase.
    pub violations: Vec<Violation>,
}

#[derive(Clone)]
struct Draft {
    name: String,
    kind: NodeKind,
    parent: Option<String>,
    src_root: Option<String>,
    langs: Vec<Lang>,
}

/// Applies `plan` to `model` and checks the result.
#[expect(clippy::too_many_lines, reason = "one pass per step: apply the edits, derive paths, list changes, check")]
pub fn resolve(model: &Model, config: &Config, plan: &Plan) -> Result<Resolved> {
    let mut drafts: BTreeMap<String, Draft> = model
        .nodes
        .values()
        .map(|n| (n.id.clone(), Draft { name: n.name.clone(), kind: n.kind, parent: n.parent.clone(), src_root: n.src_root.clone(), langs: n.langs.clone() }))
        .collect();
    let mut touched: HashMap<String, &'static str> = HashMap::new();

    for op in &plan.ops {
        match op {
            Op::Create { id, parent, name } => {
                if !drafts.contains_key(parent) {
                    bail!("create {name}: no node {parent}");
                }
                let (kind, src_root, langs) = if parent == WORKSPACE {
                    let lang = main_lang(model);
                    (NodeKind::Package, (lang == Some(Lang::Rust)).then(|| "src".to_string()), lang.into_iter().collect())
                } else {
                    (NodeKind::Dir, None, drafts[parent].langs.clone())
                };
                drafts.insert(id.clone(), Draft { name: name.clone(), kind, parent: Some(parent.clone()), src_root, langs });
                touched.insert(id.clone(), "create");
            }
            Op::Move { node, to } => {
                if node == WORKSPACE || node == "." {
                    bail!("{node} cannot be moved");
                }
                if !drafts.contains_key(node) || !drafts.contains_key(to) {
                    bail!("move {node} → {to}: unknown node");
                }
                if ancestors(&drafts, to).any(|a| a == node) {
                    bail!("cannot move {node} into itself");
                }
                let d = drafts.get_mut(node).expect("checked above");
                d.parent = Some(to.clone());
                match (d.kind, to.as_str()) {
                    (NodeKind::Package, WORKSPACE) => {}
                    (_, WORKSPACE) => {
                        d.kind = NodeKind::Package;
                        d.src_root = d.langs.contains(&Lang::Rust).then(|| "src".to_string());
                    }
                    (NodeKind::Package, _) => {
                        d.kind = NodeKind::Dir;
                        d.src_root = None;
                    }
                    _ => {}
                }
                if drafts[to].kind == NodeKind::File {
                    drafts.get_mut(to).expect("exists").kind = NodeKind::Dir;
                }
                touched.entry(node.clone()).or_insert("move");
            }
            Op::Rename { node, name } => {
                let Some(d) = drafts.get_mut(node) else { bail!("rename: no node {node}") };
                if !is_ident_like(name) {
                    bail!("{name:?} is not a valid module or package name");
                }
                d.name.clone_from(name);
                touched.entry(node.clone()).or_insert("rename");
            }
            Op::Note { .. } => {}
        }
    }

    let mut paths = BTreeMap::new();
    for id in drafts.keys() {
        paths.insert(id.clone(), planned_path(model, &drafts, id));
    }
    for (id, path) in &paths {
        if let Some(other) = paths.iter().find(|(o, p)| *o != id && *p == path) {
            bail!("{id} and {} would both be at {path}", other.0);
        }
    }

    let mut changes = Vec::new();
    for (id, draft) in &drafts {
        let to = paths[id].clone();
        let from = model.nodes.get(id).map(|n| n.id.clone());
        let renamed_in_place = touched.get(id) == Some(&"rename") && model.nodes.get(id).is_some_and(|n| n.name != draft.name);
        if from.as_deref() == Some(to.as_str()) && !renamed_in_place {
            continue;
        }
        // A node that only moved because its parent did is covered by the parent's change.
        let parent_explains = !touched.contains_key(id)
            && draft.parent.as_ref().is_some_and(|p| model.nodes.get(id).and_then(|n| n.parent.as_ref()) == Some(p));
        if parent_explains {
            continue;
        }
        let was_package = model.nodes.get(id).is_some_and(|n| n.kind == NodeKind::Package);
        let action = match touched.get(id).copied() {
            Some("create") => "create",
            _ if draft.kind == NodeKind::Package && !was_package => "extract",
            _ if was_package && draft.kind != NodeKind::Package => "inline",
            Some(a) => a,
            None => "move",
        };
        changes.push(Change { id: id.clone(), from, to, kind: draft.kind, action: action.into() });
    }

    let planned = planned_model(model, config, &drafts, &paths);
    let current = rules::check(model, config);
    let violations = rules::check(&planned, config);
    let remapped: std::collections::HashSet<String> = current
        .iter()
        .map(|v| {
            let nodes: Vec<String> = v.nodes.iter().map(|n| paths.get(n).cloned().unwrap_or_else(|| n.clone())).collect();
            Violation::make_key(&v.rule, &nodes)
        })
        .collect();
    let planned_keys: std::collections::HashSet<&str> = violations.iter().map(|v| v.key.as_str()).collect();
    let added = violations.iter().filter(|v| !remapped.contains(&v.key)).cloned().collect();
    let removed = current
        .iter()
        .filter(|v| {
            let nodes: Vec<String> = v.nodes.iter().map(|n| paths.get(n).cloned().unwrap_or_else(|| n.clone())).collect();
            !planned_keys.contains(Violation::make_key(&v.rule, &nodes).as_str())
        })
        .cloned()
        .collect();

    Ok(Resolved { changes, paths, planned, added, removed, violations })
}

fn is_ident_like(name: &str) -> bool {
    !name.is_empty()
        && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
        && !name.starts_with(|c: char| c.is_ascii_digit())
}

fn ancestors<'a>(drafts: &'a BTreeMap<String, Draft>, id: &'a str) -> impl Iterator<Item = &'a str> + 'a {
    std::iter::successors(Some(id), |cur| drafts.get(*cur).and_then(|d| d.parent.as_deref()))
}

fn planned_path(model: &Model, drafts: &BTreeMap<String, Draft>, id: &str) -> String {
    let d = &drafts[id];
    match (d.kind, d.parent.as_deref()) {
        (NodeKind::Workspace, _) => WORKSPACE.into(),
        (NodeKind::Package, _) => match model.nodes.get(id) {
            Some(n) if n.kind == NodeKind::Package && n.name == d.name => n.id.clone(),
            Some(n) if n.kind == NodeKind::Package => match n.id.rsplit_once('/') {
                Some((dir, _)) => format!("{dir}/{}", d.name),
                None if n.id == "." => ".".into(),
                None => d.name.clone(),
            },
            _ => crate::lang::join(&package_home(model), &[&d.name]),
        },
        (_, Some(parent)) => {
            let p = &drafts[parent];
            let src_root = if p.kind == NodeKind::Package { p.src_root.as_deref() } else { None };
            model::child_path(&planned_path(model, drafts, parent), src_root, &d.name)
        }
        (_, None) => d.name.clone(),
    }
}

/// The language most packages are in.
fn main_lang(model: &Model) -> Option<Lang> {
    let mut counts: BTreeMap<Lang, usize> = BTreeMap::new();
    for (_, langs) in model.packages() {
        for l in langs {
            *counts.entry(l).or_default() += 1;
        }
    }
    counts.into_iter().max_by_key(|(l, n)| (*n, std::cmp::Reverse(*l))).map(|(l, _)| l)
}

/// Where a new package goes: the folder most packages are in, or `crates`
/// (Rust) or `packages` when they are all at the root.
fn package_home(model: &Model) -> String {
    let mut counts: BTreeMap<&str, usize> = BTreeMap::new();
    for n in model.nodes.values().filter(|n| n.kind == NodeKind::Package) {
        if let Some((dir, _)) = n.id.rsplit_once('/') {
            *counts.entry(dir).or_default() += 1;
        }
    }
    counts.into_iter().max_by_key(|(_, n)| *n).map_or_else(
        || if main_lang(model) == Some(Lang::Rust) { "crates".into() } else { "packages".into() },
        |(dir, _)| dir.to_string(),
    )
}

fn planned_model(model: &Model, config: &Config, drafts: &BTreeMap<String, Draft>, paths: &BTreeMap<String, String>) -> Model {
    let map = |id: &str| paths.get(id).cloned().unwrap_or_else(|| id.to_string());
    let mut nodes = BTreeMap::new();
    for (id, d) in drafts {
        let base = model.nodes.get(id);
        let new_id = map(id);
        nodes.insert(
            new_id.clone(),
            Node {
                id: new_id,
                name: d.name.clone(),
                kind: d.kind,
                parent: d.parent.as_deref().map(map),
                root_file: base.and_then(|b| b.root_file.clone()),
                loc: base.map_or(0, |b| b.loc),
                max_file_loc: base.map_or(0, |b| b.max_file_loc),
                has_doc: base.is_some_and(|b| b.has_doc),
                layer: None,
                langs: d.langs.clone(),
                src_root: d.src_root.clone(),
            },
        );
    }
    let edges = model
        .edges
        .iter()
        .map(|e| Edge { from: map(&e.from), to: map(&e.to), count: e.count, samples: e.samples.clone() })
        .collect();
    let mut planned = Model { nodes, edges, warnings: Vec::new() };
    model::assign_layers(&mut planned, config);
    planned
}

/// The prompt that asks Claude Code to carry out `resolved`.
pub fn prompt(repo: &std::path::Path, model: &Model, config: &Config, plan: &Plan, resolved: &Resolved) -> String {
    let mut p = String::new();
    p.push_str(
        "You are restructuring this codebase. The target layout was drawn by a person on an \
         architecture board; carry it out exactly, without changing behaviour.\n\n## Changes\n\n",
    );
    for c in &resolved.changes {
        let loc = model.nodes.get(&c.id).map_or(0, |n| n.loc);
        let from = c.from.as_deref().unwrap_or("(new)");
        let line = match c.action.as_str() {
            "create" if c.kind == NodeKind::Package => {
                let lang = resolved.planned.nodes.get(&c.to).and_then(|n| n.langs.first()).map_or("", |l| l.label());
                format!("Create a new {lang} package at `{}`, a member of the workspace like the others.", c.to)
            }
            "create" => format!("Create a new folder `{}` (with the root file its language needs to declare or export its children, if any).", c.to),
            "extract" => format!("Extract `{from}` ({loc} lines) into a new package at `{}`; the code that used it now depends on the package.", c.to),
            "inline" => format!("Fold package `{from}` ({loc} lines) into the folder `{}` and drop the package.", c.to),
            "rename" if from == c.to => {
                let name = resolved.planned.nodes.get(&c.to).map_or("", |n| n.name.as_str());
                format!("Rename the package at `{from}` to `{name}`.")
            }
            "rename" => format!("Rename `{from}` to `{}`.", c.to),
            _ => format!("Move `{from}` ({loc} lines) with everything inside it to `{}`.", c.to),
        };
        let _ = writeln!(p, "- {line}");
    }
    for op in &plan.ops {
        if let Op::Note { node, text } = op {
            let at = resolved.paths.get(node).cloned().unwrap_or_else(|| node.clone());
            let _ = writeln!(p, "- About `{at}`: {text}");
        }
    }
    if !plan.instructions.trim().is_empty() {
        let _ = write!(p, "\n## Instructions from the person\n\n{}\n", plan.instructions.trim());
    }
    p.push_str(
        "\n## How\n\n\
         - Paths above are repository-relative, without file extensions.\n\
         - Use `git mv` so history follows the files. Update every import, re-export, build manifest, config path \
         and doc link the move breaks:\n",
    );
    let langs = changed_langs(model, resolved);
    for l in &langs {
        let _ = writeln!(p, "  - {}", l.how());
    }
    p.push_str(
        "- Update the docs that describe the layout (CLAUDE.md, AGENTS.md, READMEs, module docs) in the same change.\n\
         - Do not change behaviour, rename items, or reformat code you are not otherwise touching. No commits.\n",
    );
    let checks = model::verify_commands(config, repo, model);
    if checks.is_empty() {
        p.push_str("- When done, run the project's type checker or build for what you touched and fix what it reports.\n");
    } else {
        let _ = writeln!(p, "- When done, these must pass (fix what they report): {}", checks.iter().map(|c| format!("`{c}`")).collect::<Vec<_>>().join(", "));
    }
    if !config.layers.is_empty() {
        p.push_str("\n## Architecture rules the result must keep\n\nLayers, top first; a module may use its own layer and the layers below it, never one above:\n");
        for l in &config.layers {
            let _ = writeln!(p, "- {}: {} ({})", l.name, l.description, l.paths.join(", "));
        }
    }
    for f in &config.forbid {
        let _ = writeln!(p, "- `{}` must not use `{}`. {}", f.from, f.to, f.reason);
    }
    if !config.rules.acyclic.is_empty() {
        let _ = writeln!(p, "- No dependency cycles among: {}", config.rules.acyclic.join(", "));
    }
    if !resolved.added.is_empty() {
        p.push_str("\nThe drawn layout breaks these rules and the person chose it anyway. Keep the layout as drawn rather than redesigning around them:\n");
        for v in &resolved.added {
            let _ = writeln!(p, "- {}", v.message);
        }
    }
    p.push_str("\nWhen you finish, reply with a short summary of what moved and anything you could not do.\n");
    p
}

/// The languages in the nodes the plan changes, or in the whole codebase when
/// it only creates folders.
fn changed_langs(model: &Model, resolved: &Resolved) -> Vec<Lang> {
    let mut langs: Vec<Lang> = resolved.changes.iter().filter_map(|c| model.nodes.get(&c.id)).flat_map(|n| n.langs.iter().copied()).collect();
    if langs.is_empty() {
        langs = model.langs();
    }
    langs.sort();
    langs.dedup();
    langs
}

/// Checks a rescanned codebase against the plan: every change landed where it
/// was drawn, and no rule broke that the plan did not accept.
pub fn verify(after: &Model, config: &Config, resolved: &Resolved, baseline: &[String]) -> Vec<String> {
    let mut problems = Vec::new();
    for c in &resolved.changes {
        if !after.nodes.contains_key(&c.to) {
            problems.push(format!("`{}` does not exist; the plan puts {} there", c.to, c.from.as_deref().unwrap_or("a new module")));
        }
        if let Some(from) = &c.from
            && from != &c.to && after.nodes.contains_key(from) && !resolved.paths.values().any(|p| p == from) {
                problems.push(format!("`{from}` still exists; it should have moved to `{}`", c.to));
            }
    }
    let accepted: Vec<String> = resolved.violations.iter().map(|v| v.key.clone()).chain(baseline.iter().cloned()).collect();
    for v in rules::check(after, config) {
        if !rules::is_known(&v, &accepted) {
            let at = v.samples.first().map_or_default(|s| format!(" (at {s})"));
            problems.push(format!("architecture rule broken: {}{at}", v.message));
        }
    }
    problems
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::{Op, Plan, resolve};
    use crate::config::{Config, Layer};
    use crate::lang::Lang;
    use crate::model::{Edge, Model, Node, NodeKind, WORKSPACE};

    fn node(id: &str, kind: NodeKind, parent: Option<&str>) -> (String, Node) {
        let name = id.rsplit('/').next().unwrap_or(id).to_string();
        let src_root = (kind == NodeKind::Package).then(|| "src".to_string());
        let node = Node {
            id: id.into(),
            name,
            kind,
            parent: parent.map(String::from),
            root_file: None,
            loc: 10,
            max_file_loc: 10,
            has_doc: true,
            layer: None,
            langs: vec![Lang::Rust],
            src_root,
        };
        (id.into(), node)
    }

    /// The app crate at `.` with `src/net` (holding `src/net/retry`) and
    /// `src/util`, where `src/net/retry` uses `src/util`.
    fn model() -> Model {
        let nodes: BTreeMap<String, Node> = [
            node(WORKSPACE, NodeKind::Workspace, None),
            node(".", NodeKind::Package, Some(WORKSPACE)),
            node("src/net", NodeKind::Dir, Some(".")),
            node("src/net/retry", NodeKind::File, Some("src/net")),
            node("src/util", NodeKind::File, Some(".")),
        ]
        .into_iter()
        .collect();
        let edges = vec![Edge { from: "src/net/retry".into(), to: "src/util".into(), count: 1, samples: vec!["src/net/retry.rs:2".into()] }];
        Model { nodes, edges, warnings: Vec::new() }
    }

    fn layered() -> Config {
        Config {
            layers: vec![
                Layer { name: "features".into(), description: String::new(), paths: vec!["src/net/**".into()] },
                Layer { name: "core".into(), description: String::new(), paths: vec!["src/util".into(), "src/core/**".into()] },
            ],
            ..Config::default()
        }
    }

    fn plan(ops: Vec<Op>) -> Plan {
        Plan { ops, instructions: String::new() }
    }

    #[test]
    fn moving_into_a_new_folder_moves_the_subtree() {
        let r = resolve(
            &model(),
            &Config::default(),
            &plan(vec![
                Op::Create { id: "new:1".into(), parent: ".".into(), name: "core".into() },
                Op::Move { node: "src/net".into(), to: "new:1".into() },
            ]),
        )
        .expect("resolves");
        assert_eq!(r.paths["src/net/retry"], "src/core/net/retry");
        // The child moves with its parent, so only the parent is a change.
        let mut changed: Vec<&str> = r.changes.iter().map(|c| c.to.as_str()).collect();
        changed.sort_unstable();
        assert_eq!(changed, ["src/core", "src/core/net"]);
    }

    #[test]
    fn a_module_moved_to_the_workspace_becomes_a_package() {
        let r = resolve(&model(), &Config::default(), &plan(vec![Op::Move { node: "src/util".into(), to: WORKSPACE.into() }])).expect("resolves");
        assert_eq!(r.paths["src/util"], "crates/util");
        assert_eq!(r.changes[0].action, "extract");
    }

    #[test]
    fn a_node_cannot_move_into_itself() {
        let err = resolve(&model(), &Config::default(), &plan(vec![Op::Move { node: "src/net".into(), to: "src/net/retry".into() }]));
        assert!(err.is_err());
    }

    #[test]
    fn two_nodes_cannot_land_on_one_path() {
        let err = resolve(&model(), &Config::default(), &plan(vec![Op::Rename { node: "src/util".into(), name: "net".into() }]));
        assert!(err.is_err());
    }

    #[test]
    fn a_plan_reports_the_layer_violations_it_adds_and_removes() {
        let config = layered();
        let mut m = model();
        crate::model::assign_layers(&mut m, &config);
        // util (core) moved into net (features) is used from its own layer: legal.
        let ok = resolve(&m, &config, &plan(vec![Op::Move { node: "src/util".into(), to: "src/net".into() }])).expect("resolves");
        assert_eq!(ok.added, []);

        // core using features breaks the rule until util moves up into features.
        m.edges.push(Edge { from: "src/util".into(), to: "src/net".into(), count: 1, samples: Vec::new() });
        let bad = resolve(&m, &config, &plan(vec![])).expect("resolves");
        assert_eq!(bad.violations.len(), 1, "{:?}", bad.violations);
        let added = resolve(&m, &config, &plan(vec![Op::Move { node: "src/net/retry".into(), to: ".".into() }])).expect("resolves");
        assert!(added.added.is_empty(), "retry leaving net adds nothing: {:?}", added.added);
        let fixed = resolve(&m, &config, &plan(vec![Op::Create { id: "new:1".into(), parent: "src/net".into(), name: "util2".into() }, Op::Move { node: "src/util".into(), to: "new:1".into() }])).expect("resolves");
        assert_eq!(fixed.removed.len(), 1);
        assert_eq!(fixed.violations, []);
    }
}
