//! `architecture.toml`, archboard's configuration.
//!
//! It says which packages make up the codebase, the architecture rules they
//! must keep, and how Claude Code is run and its work verified. Every section
//! is optional: without the file, archboard finds the packages
//! itself, checks no rules, and verifies with the languages' usual checks.

use std::path::Path;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

/// The file archboard reads from the repository root.
pub const CONFIG_FILE: &str = "architecture.toml";
/// Known violations `check` tolerates, one key per line (`check --write-baseline`).
pub const BASELINE_FILE: &str = "architecture.baseline";

/// The whole `architecture.toml`.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct Config {
    /// Which packages are scanned.
    pub scan: Scan,
    /// Layers from the top (may depend on everything below) to the bottom.
    #[serde(rename = "layer")]
    pub layers: Vec<Layer>,
    /// Dependencies that are never allowed, whatever the layers say.
    pub forbid: Vec<Forbid>,
    /// Structural rules.
    pub rules: Rules,
    /// How Claude Code is invoked.
    pub claude: Claude,
    /// Commands that must pass after Claude Code has applied a plan.
    pub verify: Verify,
}

/// `[scan]`.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct Scan {
    /// Package directories, relative to the repository; a trailing `/*` takes
    /// every directory in it with a manifest. Unset, every package in the
    /// repository is found (honouring `.gitignore`).
    #[serde(alias = "crates", skip_serializing_if = "Option::is_none")]
    pub packages: Option<Vec<String>>,
    /// Paths to leave out (vendored or generated code), as globs matched
    /// against packages, folders and files.
    pub exclude: Vec<String>,
}

/// One `[[layer]]`.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Layer {
    /// Shown on the board.
    pub name: String,
    /// What belongs here and why.
    #[serde(default)]
    pub description: String,
    /// Node paths (globs) in this layer. The most specific pattern wins.
    pub paths: Vec<String>,
}

/// One `[[forbid]]`.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Forbid {
    /// Glob of depending nodes.
    pub from: String,
    /// Glob of nodes they must not use.
    pub to: String,
    /// Globs of nodes under `to` that stay allowed.
    #[serde(default)]
    pub except: Vec<String>,
    /// Shown with each violation.
    #[serde(default)]
    pub reason: String,
}

/// `[rules]`.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct Rules {
    /// Globs whose matches must not depend on each other in a cycle, e.g.
    /// `src/plugins/*`: each matching node, with everything inside it, is one
    /// vertex of the graph that must stay acyclic.
    pub acyclic: Vec<String>,
    /// Globs of packages and folders that must say what they are for: a doc
    /// at the top of their root file (`//!`, a docstring, a leading comment,
    /// `// Package`) or a README.
    pub module_doc: Vec<String>,
    /// Longest a single source file may be.
    pub max_file_lines: Option<usize>,
}

/// `[claude]`.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct Claude {
    /// The Claude Code binary.
    pub binary: String,
    /// Extra arguments, e.g. the permission mode and allowed tools. Unset,
    /// edits are accepted and the tools allowed are reading, editing, moving
    /// files with git, and the build and test commands of the languages in
    /// the repository (`job::claude_args`).
    pub args: Option<Vec<String>>,
    /// `--model`, if set.
    pub model: Option<String>,
    /// How many times Claude Code is sent the verification failures to fix.
    pub max_fix_rounds: u32,
}

impl Default for Claude {
    fn default() -> Self {
        Self { binary: "claude".into(), args: None, model: None, max_fix_rounds: 2 }
    }
}

/// `[verify]`.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct Verify {
    /// Shell commands run from the repository root; each must exit 0. Unset,
    /// the languages' usual checks run (`lang::default_verify`); set
    /// it to `[]` to run none.
    pub commands: Option<Vec<String>>,
}

impl Config {
    /// Reads `architecture.toml` from `repo`, or the defaults when it has none.
    ///
    /// # Errors
    ///
    /// When the file exists but cannot be read or parsed.
    pub fn load(repo: &Path) -> Result<Self> {
        let path = repo.join(CONFIG_FILE);
        if !path.exists() {
            return Ok(Self::default());
        }
        let text = std::fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
        toml::from_str(&text).with_context(|| format!("parsing {}", path.display()))
    }
}

/// Reads the baseline's keys; a missing file is an empty baseline.
pub fn load_baseline(repo: &Path) -> Vec<String> {
    std::fs::read_to_string(repo.join(BASELINE_FILE))
        .unwrap_or_default()
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .map(String::from)
        .collect()
}
