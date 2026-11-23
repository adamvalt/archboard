//! Rust modules and paths.
//!
//! Modules live under each crate's `src/`, and every `use` and path is
//! resolved with `syn` through `crate::`, `self::`, `super::`, workspace crate
//! names and child modules, following `pub use` re-exports (globs too) so a
//! path lands on the module that defines the item.

use std::collections::HashMap;
use std::path::Path;

use anyhow::{Context, Result};
use syn::visit::Visit;

/// The crate's package name and the name Rust code calls it by, if `dir` has
/// a `Cargo.toml` with a `[package]`.
///
/// # Errors
///
/// When the `Cargo.toml` is not valid TOML.
pub fn read_manifest(dir: &Path) -> Result<Option<(String, String)>> {
    let Ok(text) = std::fs::read_to_string(dir.join("Cargo.toml")) else { return Ok(None) };
    let manifest: toml::Value = toml::from_str(&text).with_context(|| format!("parsing {}/Cargo.toml", dir.display()))?;
    let Some(package) = manifest.get("package").and_then(|p| p.get("name")).and_then(toml::Value::as_str) else {
        return Ok(None);
    };
    let lib = manifest
        .get("lib")
        .and_then(|l| l.get("name"))
        .and_then(toml::Value::as_str)
        .map_or_else(|| package.replace('-', "_"), String::from);
    Ok(Some((package.to_string(), lib)))
}

/// A `//!` or `#![doc]` before any item.
pub fn has_doc(text: &str) -> bool {
    text.lines()
        .map(str::trim_start)
        .find(|l| !l.is_empty() && !l.starts_with("//") || l.starts_with("//!"))
        .is_some_and(|l| l.starts_with("//!") || l.starts_with("#![doc"))
}

/// Collects every Rust file's paths, then resolves them once all `pub use`
/// re-exports are known.
#[derive(Debug)]
pub struct Scan<'a> {
    index: &'a HashMap<AbsPath, String>,
    lib_names: HashMap<String, String>,
    reexports: Reexports,
    pending: Vec<(String, AbsPath, String)>,
}

impl<'a> Scan<'a> {
    /// `index` maps (crate id, module segments) to node ids; `lib_names` maps
    /// the names code uses for workspace crates to their ids.
    #[must_use]
    pub fn new(index: &'a HashMap<AbsPath, String>, lib_names: HashMap<String, String>) -> Self {
        Self { index, lib_names, reexports: HashMap::new(), pending: Vec::new() }
    }

    /// Reads one file of crate `krate`, module `module`, owned by node `owner`.
    ///
    /// # Errors
    ///
    /// The parse error, when the file is not valid Rust.
    pub fn file(&mut self, krate: &str, owner: &str, module: &[String], file: &str, text: &str) -> Result<(), syn::Error> {
        let parsed = syn::parse_file(text)?;
        let mut visitor = RefVisitor {
            krate,
            module: module.to_vec(),
            index: self.index,
            lib_names: &self.lib_names,
            refs: Vec::new(),
            reexports: &mut self.reexports,
        };
        visitor.visit_file(&parsed);
        self.pending.extend(visitor.refs.into_iter().map(|(path, line)| (owner.to_string(), path, format!("{file}:{line}"))));
        Ok(())
    }

    /// Every (from node, to node, `file:line`) reference.
    #[must_use]
    pub fn finish(self) -> Vec<(String, String, String)> {
        let resolver = Resolver { index: self.index, reexports: &self.reexports };
        self.pending.into_iter().filter_map(|(owner, path, at)| resolver.resolve(&path, 0).map(|t| (owner, t, at))).collect()
    }
}

/// A path made absolute: the crate id and the segments from its root.
pub type AbsPath = (String, Vec<String>);
/// `pub use` items: (module, exported name or `*`) → what they name.
type Reexports = HashMap<(AbsPath, String), Vec<AbsPath>>;

struct RefVisitor<'a> {
    krate: &'a str,
    module: Vec<String>,
    index: &'a HashMap<AbsPath, String>,
    lib_names: &'a HashMap<String, String>,
    refs: Vec<(AbsPath, usize)>,
    reexports: &'a mut Reexports,
}

impl RefVisitor<'_> {
    /// Makes `segs` absolute, or `None` for paths into other crates and
    /// local items.
    fn absolute(&self, segs: &[String]) -> Option<AbsPath> {
        let first = segs.first()?;
        let (krate, mut base, rest): (String, Vec<String>, &[String]) = match first.as_str() {
            "crate" => (self.krate.to_string(), Vec::new(), &segs[1..]),
            "self" => (self.krate.to_string(), self.module.clone(), &segs[1..]),
            "super" => {
                let ups = segs.iter().take_while(|s| *s == "super").count();
                let mut m = self.module.clone();
                m.truncate(m.len().saturating_sub(ups));
                (self.krate.to_string(), m, &segs[ups..])
            }
            name if self.lib_names.contains_key(name) => (self.lib_names[name].clone(), Vec::new(), &segs[1..]),
            name => {
                let mut child = self.module.clone();
                child.push(name.to_string());
                if !self.index.contains_key(&(self.krate.to_string(), child)) {
                    return None;
                }
                (self.krate.to_string(), self.module.clone(), segs)
            }
        };
        base.extend(rest.iter().cloned());
        Some((krate, base))
    }
}

struct Resolver<'a> {
    index: &'a HashMap<AbsPath, String>,
    reexports: &'a Reexports,
}

impl Resolver<'_> {
    /// The deepest module `path` names, following `pub use` re-exports so a
    /// path lands on the module that defines the item.
    fn resolve(&self, (krate, segs): &AbsPath, depth: usize) -> Option<String> {
        let mut module: Vec<String> = Vec::new();
        let mut target = self.index.get(&(krate.clone(), module.clone())).cloned();
        for (i, seg) in segs.iter().enumerate() {
            let mut next = module.clone();
            next.push(seg.clone());
            if let Some(id) = self.index.get(&(krate.clone(), next.clone())) {
                target = Some(id.clone());
                module = next;
                continue;
            }
            if depth < 8 {
                let key = (krate.clone(), module);
                let named = self.reexports.get(&(key.clone(), seg.clone())).into_iter().flatten().map(|via| (via, i + 1));
                let globbed = self.reexports.get(&(key, "*".into())).into_iter().flatten().map(|via| (via, i));
                for (via, from) in named.chain(globbed) {
                    let mut redirected = via.clone();
                    redirected.1.extend(segs[from..].iter().cloned());
                    if let Some(t) = self.resolve(&redirected, depth + 1).filter(|t| Some(t) != target.as_ref()) {
                        return Some(t);
                    }
                }
            }
            break;
        }
        target
    }
}

/// Flattens a `use` tree into (path, name it binds, line); a glob binds `*`.
fn flatten_use(tree: &syn::UseTree, prefix: &mut Vec<String>, out: &mut Vec<(Vec<String>, String, usize)>) {
    match tree {
        syn::UseTree::Path(p) => {
            prefix.push(p.ident.to_string());
            flatten_use(&p.tree, prefix, out);
            prefix.pop();
        }
        syn::UseTree::Name(n) => {
            let mut v = prefix.clone();
            let name = if n.ident == "self" { prefix.last().cloned().unwrap_or_default() } else { n.ident.to_string() };
            if n.ident != "self" {
                v.push(n.ident.to_string());
            }
            out.push((v, name, n.ident.span().start().line));
        }
        syn::UseTree::Rename(r) => {
            let mut v = prefix.clone();
            v.push(r.ident.to_string());
            out.push((v, r.rename.to_string(), r.ident.span().start().line));
        }
        syn::UseTree::Glob(g) => out.push((prefix.clone(), "*".into(), g.star_token.spans[0].start().line)),
        syn::UseTree::Group(g) => g.items.iter().for_each(|t| flatten_use(t, prefix, out)),
    }
}

impl<'ast> Visit<'ast> for RefVisitor<'_> {
    fn visit_item_mod(&mut self, m: &'ast syn::ItemMod) {
        if m.content.is_some() {
            self.module.push(m.ident.to_string());
            syn::visit::visit_item_mod(self, m);
            self.module.pop();
        }
    }

    fn visit_item_use(&mut self, u: &'ast syn::ItemUse) {
        let mut out = Vec::new();
        flatten_use(&u.tree, &mut Vec::new(), &mut out);
        let exported = !matches!(u.vis, syn::Visibility::Inherited);
        for (segs, alias, line) in out {
            let Some(abs) = self.absolute(&segs) else { continue };
            if exported {
                let here = (self.krate.to_string(), self.module.clone());
                self.reexports.entry((here, alias)).or_default().push(abs.clone());
            }
            self.refs.push((abs, line));
        }
    }

    fn visit_path(&mut self, p: &'ast syn::Path) {
        if p.segments.len() >= 2 {
            let segs: Vec<String> = p.segments.iter().map(|s| s.ident.to_string()).collect();
            let line = p.segments[0].ident.span().start().line;
            if let Some(abs) = self.absolute(&segs) {
                self.refs.push((abs, line));
            }
        }
        syn::visit::visit_path(self, p);
    }
}
