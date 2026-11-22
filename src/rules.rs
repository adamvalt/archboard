//! Checks a [`Model`] against the architecture rules in `architecture.toml`.
//!
//! Each violation has a stable `key` (`layer:a->b`, `forbid:a->b`,
//! `cycle:a,b,c`, `doc:a`, `size:file`) so known ones can be listed in
//! `architecture.baseline` and only new ones fail.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use serde::{Deserialize, Serialize};

use crate::config::Config;
use crate::glob;
use crate::model::{Model, NodeKind};

/// One broken rule.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Violation {
    /// Stable identity, see the module doc.
    pub key: String,
    /// `layer`, `forbid`, `cycle`, `doc` or `size`.
    pub rule: String,
    /// For people.
    pub message: String,
    /// The nodes involved (depending node first for edges).
    pub nodes: Vec<String>,
    /// `file:line` locations of offending paths.
    pub samples: Vec<String>,
}

impl Violation {
    /// Rebuilds the key from `rule` and `nodes`, e.g. after the nodes were
    /// renamed by a plan.
    pub fn make_key(rule: &str, nodes: &[String]) -> String {
        match rule {
            "cycle" => {
                let mut sorted = nodes.to_vec();
                sorted.sort();
                format!("cycle:{}", sorted.join(","))
            }
            "layer" | "forbid" => format!("{rule}:{}->{}", nodes[0], nodes[1]),
            _ => format!("{rule}:{}", nodes.join(",")),
        }
    }
}

/// Whether `baseline` lists `v`. A known cycle also covers every smaller
/// cycle among its members, so breaking part of a cycle never fails the check.
pub fn is_known(v: &Violation, baseline: &[String]) -> bool {
    baseline.iter().any(|k| {
        k == &v.key
            || (v.rule == "cycle"
                && k.strip_prefix("cycle:").is_some_and(|members| {
                    let members: BTreeSet<&str> = members.split(',').collect();
                    v.nodes.iter().all(|n| members.contains(n.as_str()))
                }))
    })
}

/// Baseline keys no current violation needs any more.
pub fn stale<'a>(violations: &[Violation], baseline: &'a [String]) -> Vec<&'a String> {
    baseline.iter().filter(|k| !violations.iter().any(|v| is_known(v, std::slice::from_ref(*k)))).collect()
}

/// Every violation in `model`, in key order.
pub fn check(model: &Model, config: &Config) -> Vec<Violation> {
    let mut out = Vec::new();
    let rank: HashMap<&str, usize> = config.layers.iter().enumerate().map(|(i, l)| (l.name.as_str(), i)).collect();

    let mut layer_hits: BTreeMap<(String, String), Vec<String>> = BTreeMap::new();
    let mut forbid_hits: BTreeMap<(String, String), (String, Vec<String>)> = BTreeMap::new();
    for edge in &model.edges {
        let layer = |id: &str| model.nodes.get(id).and_then(|n| n.layer.as_deref()).and_then(|l| rank.get(l).copied());
        if let (Some(a), Some(b)) = (layer(&edge.from), layer(&edge.to)) {
            // Layers are listed top first, so a node may use its own layer or later ones.
            if b < a {
                layer_hits.entry((edge.from.clone(), edge.to.clone())).or_default().extend(edge.samples.iter().cloned());
            }
        }
        for rule in &config.forbid {
            let from_hit = model.ancestors(&edge.from).any(|a| glob::matches(&rule.from, a));
            let to_hit = model.ancestors(&edge.to).any(|a| glob::matches(&rule.to, a))
                && !model.ancestors(&edge.to).any(|a| rule.except.iter().any(|g| glob::matches(g, a)));
            if from_hit && to_hit {
                forbid_hits
                    .entry((edge.from.clone(), edge.to.clone()))
                    .or_insert_with(|| (rule.reason.clone(), Vec::new()))
                    .1
                    .extend(edge.samples.iter().cloned());
            }
        }
    }
    for ((from, to), samples) in layer_hits {
        let lf = model.nodes[&from].layer.clone().unwrap_or_default();
        let lt = model.nodes[&to].layer.clone().unwrap_or_default();
        let nodes = vec![from.clone(), to.clone()];
        out.push(Violation {
            key: Violation::make_key("layer", &nodes),
            rule: "layer".into(),
            message: format!("{from} ({lf}) uses {to} ({lt}), a higher layer"),
            nodes,
            samples,
        });
    }
    for ((from, to), (reason, samples)) in forbid_hits {
        let nodes = vec![from.clone(), to.clone()];
        out.push(Violation {
            key: Violation::make_key("forbid", &nodes),
            rule: "forbid".into(),
            message: if reason.is_empty() { format!("{from} must not use {to}") } else { format!("{from} must not use {to}: {reason}") },
            nodes,
            samples,
        });
    }

    for pattern in &config.rules.acyclic {
        out.extend(cycles(model, pattern));
    }

    for node in model.nodes.values() {
        if matches!(node.kind, NodeKind::Dir | NodeKind::Package)
            && config.rules.module_doc.iter().any(|g| glob::matches(g, &node.id))
            && !node.has_doc
        {
            let nodes = vec![node.id.clone()];
            out.push(Violation {
                key: Violation::make_key("doc", &nodes),
                rule: "doc".into(),
                message: node.root_file.as_ref().map_or_else(
                    || format!("{} has no README saying what it owns", node.id),
                    |f| format!("{} has no doc saying what it owns (at the top of {f}, or a README)", node.id),
                ),
                samples: vec![node.root_file.as_ref().map_or_else(|| format!("{}/", node.id), |f| format!("{f}:1"))],
                nodes,
            });
        }
        if let (Some(max), Some(file)) = (config.rules.max_file_lines, &node.root_file) {
            let own = node.max_file_loc;
            if node.kind == NodeKind::File && own > max {
                let nodes = vec![node.id.clone()];
                out.push(Violation {
                    key: Violation::make_key("size", &nodes),
                    rule: "size".into(),
                    message: format!("{file} has {own} lines (limit {max})"),
                    samples: vec![format!("{file}:1")],
                    nodes,
                });
            }
        }
    }
    out.sort_by(|a, b| a.key.cmp(&b.key));
    out
}

/// Strongly connected components of the graph whose vertices are the nodes
/// matching `pattern` (each standing for everything inside it).
fn cycles(model: &Model, pattern: &str) -> Vec<Violation> {
    let group_of = |id: &str| model.ancestors(id).find(|a| glob::matches(pattern, a)).map(String::from);
    let mut graph: BTreeMap<String, BTreeMap<String, Vec<String>>> = BTreeMap::new();
    for edge in &model.edges {
        if let (Some(a), Some(b)) = (group_of(&edge.from), group_of(&edge.to))
            && a != b {
                graph.entry(a).or_default().entry(b.clone()).or_default().extend(edge.samples.iter().cloned());
                graph.entry(b).or_default();
            }
    }
    let names: Vec<&String> = graph.keys().collect();
    let idx: HashMap<&String, usize> = names.iter().enumerate().map(|(i, n)| (*n, i)).collect();
    let adj: Vec<Vec<usize>> = names.iter().map(|n| graph[*n].keys().map(|t| idx[t]).collect()).collect();

    tarjan(&adj)
        .into_iter()
        .filter(|c| c.len() > 1)
        .map(|comp| {
            let members: BTreeSet<usize> = comp.iter().copied().collect();
            let nodes: Vec<String> = members.iter().map(|&i| names[i].clone()).collect();
            let mut samples = Vec::new();
            let mut arrows = Vec::new();
            for &i in &members {
                for (t, s) in &graph[names[i]] {
                    if members.contains(&idx[t]) {
                        arrows.push(format!("{} → {}", names[i], t));
                        samples.extend(s.iter().take(1).cloned());
                    }
                }
            }
            Violation {
                key: Violation::make_key("cycle", &nodes),
                rule: "cycle".into(),
                message: format!("dependency cycle among {} ({} links between them)", nodes.join(", "), arrows.len()),
                nodes,
                samples,
            }
        })
        .collect()
}

fn tarjan(adj: &[Vec<usize>]) -> Vec<Vec<usize>> {
    struct State<'a> {
        adj: &'a [Vec<usize>],
        index: Vec<Option<usize>>,
        low: Vec<usize>,
        on_stack: Vec<bool>,
        stack: Vec<usize>,
        next: usize,
        out: Vec<Vec<usize>>,
    }
    fn visit(s: &mut State, v: usize) {
        s.index[v] = Some(s.next);
        s.low[v] = s.next;
        s.next += 1;
        s.stack.push(v);
        s.on_stack[v] = true;
        for i in 0..s.adj[v].len() {
            let w = s.adj[v][i];
            match s.index[w] {
                None => {
                    visit(s, w);
                    s.low[v] = s.low[v].min(s.low[w]);
                }
                Some(iw) if s.on_stack[w] => s.low[v] = s.low[v].min(iw),
                Some(_) => {}
            }
        }
        if Some(s.low[v]) == s.index[v] {
            let mut comp = Vec::new();
            while let Some(w) = s.stack.pop() {
                s.on_stack[w] = false;
                comp.push(w);
                if w == v {
                    break;
                }
            }
            s.out.push(comp);
        }
    }
    let n = adj.len();
    let mut s = State { adj, index: vec![None; n], low: vec![0; n], on_stack: vec![false; n], stack: Vec::new(), next: 0, out: Vec::new() };
    for v in 0..n {
        if s.index[v].is_none() {
            visit(&mut s, v);
        }
    }
    s.out
}

#[cfg(test)]
mod tests {
    use super::{Violation, is_known, tarjan};

    fn cycle(nodes: &[&str]) -> Violation {
        let nodes: Vec<String> = nodes.iter().map(|n| (*n).to_string()).collect();
        Violation { key: Violation::make_key("cycle", &nodes), rule: "cycle".into(), message: String::new(), nodes, samples: Vec::new() }
    }

    #[test]
    fn a_known_cycle_covers_smaller_ones_but_not_larger() {
        let baseline = vec!["cycle:a,b,c".to_string()];
        assert!(is_known(&cycle(&["c", "a", "b"]), &baseline));
        assert!(is_known(&cycle(&["a", "b"]), &baseline));
        assert!(!is_known(&cycle(&["a", "b", "d"]), &baseline));
    }

    #[test]
    fn finds_the_cycle() {
        let comps = tarjan(&[vec![1], vec![2], vec![0], vec![0]]);
        assert!(comps.iter().any(|c| c.len() == 3));
        assert!(comps.iter().any(|c| c == &vec![3]));
    }
}
