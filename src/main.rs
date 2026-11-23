//! archboard: an architecture board for any repository (Rust, TypeScript and
//! JavaScript, Python, Go).
//!
//! `archboard serve` opens a page with the repository's packages, folders and
//! files as cards. Drag them into a new layout; the plan is checked live against the
//! rules in `architecture.toml`, and Apply has Claude Code carry it out and
//! verifies the result (see [`job`]). `archboard check` runs the same rules
//! from the command line, for CI.

mod job;

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use anyhow::{Context, Result};
use axum::extract::{Query, State};
use axum::http::{StatusCode, header};
use axum::response::{Html, IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use clap::{Parser, Subcommand};
use serde::{Deserialize, Serialize};

use archboard::config::{self, Config};
use archboard::{model, plan, rules};

#[derive(Parser)]
#[command(version, about)]
struct Cli {
    /// The repository root (where `architecture.toml` is, if it has one).
    #[arg(long, global = true, default_value = ".")]
    repo: PathBuf,
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Serve the board.
    Serve {
        /// Port on 127.0.0.1.
        #[arg(long, default_value_t = 7420)]
        port: u16,
    },
    /// Check the architecture rules; exits 1 on violations not in the baseline.
    Check {
        /// Print JSON instead of text.
        #[arg(long)]
        json: bool,
        /// Write every current violation to `architecture.baseline`.
        #[arg(long)]
        write_baseline: bool,
    },
    /// Print the scanned model as JSON.
    Graph,
    /// Write a starter `architecture.toml` for the repository.
    Init,
    /// Write a snapshot of the board (model, rules, violations) as JSON, for
    /// the static demo page.
    Export {
        /// Where to write it.
        #[arg(long)]
        out: PathBuf,
        /// Rules to use instead of the repository's `architecture.toml`.
        #[arg(long)]
        config: Option<PathBuf>,
        /// The name shown for the repository; its directory name by default.
        #[arg(long)]
        name: Option<String>,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let repo = cli.repo.canonicalize().context("--repo")?;
    match cli.command {
        Command::Serve { port } => serve(repo, port).await,
        Command::Check { json, write_baseline } => check(&repo, json, write_baseline),
        Command::Init => init(&repo),
        Command::Export { out, config, name } => export(&repo, &out, config.as_deref(), name),
        Command::Graph => {
            let config = Config::load(&repo)?;
            println!("{}", serde_json::to_string_pretty(&model::scan(&repo, &config)?)?);
            Ok(())
        }
    }
}

/// Writes `architecture.toml` with the packages found, cycle checks between
/// them, the verify commands for their languages, and a commented layer example.
fn init(repo: &Path) -> Result<()> {
    let path = repo.join(config::CONFIG_FILE);
    anyhow::ensure!(!path.exists(), "{} already exists", path.display());
    let model = model::scan(repo, &Config::default())?;
    let packages = model.packages();
    let quoted = |items: &[String]| items.iter().map(|i| format!("{i:?}")).collect::<Vec<_>>().join(", ");
    let ids: Vec<String> = packages.iter().map(|(id, _)| id.clone()).collect();
    // Packages side by side in one folder are checked for cycles as a group (`packages/*`).
    let mut by_dir: std::collections::BTreeMap<&str, usize> = std::collections::BTreeMap::new();
    for id in &ids {
        if let Some((dir, _)) = id.rsplit_once('/') {
            *by_dir.entry(dir).or_default() += 1;
        }
    }
    let groups: Vec<String> = by_dir.into_iter().filter(|(_, n)| *n > 1).map(|(dir, _)| format!("{dir}/*")).collect();
    let top_dirs: Vec<String> = model
        .nodes
        .values()
        .filter(|n| n.parent.as_deref().is_some_and(|p| ids.contains(&p.to_string())) && n.kind == model::NodeKind::Dir)
        .map(|n| n.id.clone())
        .take(3)
        .collect();
    let verify = model::verify_commands(&Config::default(), repo, &model);
    let text = format!(
        r#"# Architecture rules for this repository, checked by `archboard check` and on
# archboard's board (`archboard serve`). Node paths are repository-relative,
# without file extensions; a package is its directory (`.` for the root).

[scan]
# Found automatically (honouring .gitignore); list them to pin the set:
# packages = [{packages}]
# Vendored or generated code to leave out, as globs:
exclude = []

# Layers, top first: a node may use its own layer and every layer below it,
# never one above. The most specific matching pattern decides a node's layer,
# and a node no pattern matches takes its parent's. For example:
#
# [[layer]]
# name = "app"
# description = "Entry points and wiring"
# paths = [{example_top}]
#
# [[layer]]
# name = "core"
# description = "What everything else may use"
# paths = [{example_core}]

# A dependency that is never allowed, whatever the layers say:
#
# [[forbid]]
# from = "src/ui/**"
# to = "src/db/**"
# reason = "the UI goes through the API"

[rules]
# Globs whose matches must not depend on each other in a cycle.
acyclic = [{acyclic}]
# Globs of packages and folders that must say what they are for: a doc at the
# top of their root file or a README.
module_doc = []
# max_file_lines = 1000

[claude]
binary = "claude"
max_fix_rounds = 2
# model = "sonnet"

[verify]
# Run from the repository root after Claude Code applies a plan; each must pass.
commands = [{verify}]
"#,
        packages = quoted(&ids),
        example_top = top_dirs.first().map_or_else(String::new, |d| format!("{d:?}")),
        example_core = top_dirs.get(1).map_or_else(String::new, |d| format!("{d:?}")),
        acyclic = quoted(&groups),
        verify = quoted(&verify),
    );
    std::fs::write(&path, text)?;
    println!("wrote {} ({} package(s)); `archboard check --write-baseline` records today's violations", config::CONFIG_FILE, ids.len());
    Ok(())
}

/// Writes what `/api/model` serves, plus the rules it was checked with.
fn export(repo: &Path, out: &Path, config_path: Option<&Path>, name: Option<String>) -> Result<()> {
    let config = match config_path {
        Some(p) => toml::from_str(&std::fs::read_to_string(p).with_context(|| format!("reading {}", p.display()))?)?,
        None => Config::load(repo)?,
    };
    let mut model = model::scan(repo, &config)?;
    if let Some(name) = name
        && let Some(ws) = model.nodes.get_mut(model::WORKSPACE)
    {
        ws.name = name;
    }
    let violations = rules::check(&model, &config);
    let baseline = if config_path.is_some() { Vec::new() } else { config::load_baseline(repo) };
    let commit = std::process::Command::new("git").args(["rev-parse", "--short", "HEAD"]).current_dir(repo).output().ok().filter(|o| o.status.success()).map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string());
    let snapshot = serde_json::json!({
        "branch": commit.map_or_else(|| "snapshot".to_string(), |c| format!("snapshot at {c}")),
        "known": violations.iter().filter(|v| rules::is_known(v, &baseline)).map(|v| v.key.clone()).collect::<Vec<_>>(),
        "violations": violations,
        "layers": config.layers,
        "model": model,
        "config": config,
    });
    std::fs::write(out, serde_json::to_string(&snapshot)?)?;
    println!("wrote {} ({} nodes, {} edges, {} violations)", out.display(), model.nodes.len(), model.edges.len(), snapshot["violations"].as_array().map_or(0, Vec::len));
    Ok(())
}

fn check(repo: &Path, json: bool, write_baseline: bool) -> Result<()> {
    let config = Config::load(repo)?;
    let model = model::scan(repo, &config)?;
    let violations = rules::check(&model, &config);
    if write_baseline {
        let mut text = String::from("# Known architecture violations `archboard check` tolerates; fix them and delete their lines.\n");
        for v in &violations {
            text.push_str(&v.key);
            text.push('\n');
        }
        std::fs::write(repo.join(config::BASELINE_FILE), text)?;
        println!("wrote {} violation(s) to {}", violations.len(), config::BASELINE_FILE);
        return Ok(());
    }
    let baseline = config::load_baseline(repo);
    let new: Vec<&rules::Violation> = violations.iter().filter(|v| !rules::is_known(v, &baseline)).collect();
    let fixed = rules::stale(&violations, &baseline);
    if json {
        println!("{}", serde_json::to_string_pretty(&serde_json::json!({ "new": new, "known": violations.len() - new.len(), "fixed": fixed }))?);
    } else {
        for w in &model.warnings {
            eprintln!("warning: {w}");
        }
        for v in &new {
            println!("error[{}]: {}", v.rule, v.message);
            for s in &v.samples {
                println!("    at {s}");
            }
        }
        for k in &fixed {
            println!("note: `{k}` no longer matches; update {} (`check --write-baseline`)", config::BASELINE_FILE);
        }
        println!(
            "{} node(s), {} edge(s): {} new violation(s), {} known",
            model.nodes.len(),
            model.edges.len(),
            new.len(),
            violations.len() - new.len()
        );
    }
    if !new.is_empty() {
        std::process::exit(1);
    }
    Ok(())
}

/// A scan and the configuration it was made with.
type Scanned = Arc<(model::Model, Config)>;

#[derive(Clone)]
struct AppState {
    repo: PathBuf,
    job: job::Shared,
    /// The last scan, so checking a plan on every edit doesn't reparse the
    /// repository; `/api/model` (Rescan) refreshes it.
    scanned: Arc<Mutex<Option<Scanned>>>,
}

impl AppState {
    fn rescan(&self) -> Result<Scanned> {
        let config = Config::load(&self.repo)?;
        let fresh = Arc::new((model::scan(&self.repo, &config)?, config));
        *self.scanned.lock().expect("scan lock") = Some(fresh.clone());
        Ok(fresh)
    }

    fn scanned(&self) -> Result<Scanned> {
        let cached = self.scanned.lock().expect("scan lock").clone();
        cached.map_or_else(|| self.rescan(), Ok)
    }
}

async fn serve(repo: PathBuf, port: u16) -> Result<()> {
    let state = AppState { repo, job: Arc::new(Mutex::new(job::Job::default())), scanned: Arc::default() };
    let app = Router::new()
        .route("/", get(index))
        .route("/api/model", get(get_model))
        .route("/api/check", post(post_check))
        .route("/api/prompt", post(post_prompt))
        .route("/api/apply", post(post_apply))
        .route("/api/job", get(get_job))
        .route("/api/job/cancel", post(post_cancel))
        .with_state(state);
    let listener = tokio::net::TcpListener::bind(("127.0.0.1", port)).await?;
    println!("archboard: http://127.0.0.1:{port}");
    axum::serve(listener, app).await?;
    Ok(())
}

async fn index(State(s): State<AppState>) -> Html<String> {
    let repo = serde_json::to_string(&s.repo.to_string_lossy()).unwrap_or_default().replace("</", "<\\/");
    Html(include_str!("../web/index.html").replacen("/*REPO*/\"\"", &repo, 1))
}

struct AppError(anyhow::Error);

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        (StatusCode::UNPROCESSABLE_ENTITY, [(header::CONTENT_TYPE, "text/plain")], format!("{:#}", self.0)).into_response()
    }
}

impl<E: Into<anyhow::Error>> From<E> for AppError {
    fn from(e: E) -> Self {
        Self(e.into())
    }
}

#[derive(Serialize)]
struct ModelResponse {
    branch: String,
    model: model::Model,
    layers: Vec<config::Layer>,
    violations: Vec<rules::Violation>,
    /// Keys of the current violations `architecture.baseline` covers.
    known: Vec<String>,
}

async fn get_model(State(s): State<AppState>) -> Result<Json<ModelResponse>, AppError> {
    let scanned = s.rescan()?;
    let (model, config) = &*scanned;
    let violations = rules::check(model, config);
    let baseline = config::load_baseline(&s.repo);
    Ok(Json(ModelResponse {
        branch: job::branch(&s.repo).await,
        known: violations.iter().filter(|v| rules::is_known(v, &baseline)).map(|v| v.key.clone()).collect(),
        violations,
        layers: config.layers.clone(),
        model: model.clone(),
    }))
}

async fn post_check(State(s): State<AppState>, Json(p): Json<plan::Plan>) -> Result<Json<plan::Resolved>, AppError> {
    let scanned = s.scanned()?;
    Ok(Json(plan::resolve(&scanned.0, &scanned.1, &p)?))
}

async fn post_prompt(State(s): State<AppState>, Json(p): Json<plan::Plan>) -> Result<String, AppError> {
    let scanned = s.scanned()?;
    let (model, config) = &*scanned;
    let resolved = plan::resolve(model, config, &p)?;
    Ok(plan::prompt(&s.repo, model, config, &p, &resolved))
}

#[derive(Deserialize)]
struct ApplyRequest {
    plan: plan::Plan,
    options: job::ApplyOptions,
}

async fn post_apply(State(s): State<AppState>, Json(req): Json<ApplyRequest>) -> Result<StatusCode, AppError> {
    {
        let mut j = s.job.lock().expect("job lock");
        if j.status == Some(job::Status::Running) {
            return Err(anyhow::anyhow!("a job is already running").into());
        }
        *j = job::Job { status: Some(job::Status::Running), ..job::Job::default() };
    }
    tokio::spawn(job::run(s.job.clone(), s.repo, req.plan, req.options));
    Ok(StatusCode::ACCEPTED)
}

#[derive(Deserialize)]
struct Since {
    #[serde(default)]
    since: usize,
}

async fn get_job(State(s): State<AppState>, Query(q): Query<Since>) -> Json<serde_json::Value> {
    let j = s.job.lock().expect("job lock");
    Json(serde_json::json!({
        "status": j.status,
        "total": j.log.len(),
        "log": j.log.get(q.since.min(j.log.len())..).unwrap_or_default(),
    }))
}

async fn post_cancel(State(s): State<AppState>) -> StatusCode {
    let mut j = s.job.lock().expect("job lock");
    if j.status == Some(job::Status::Running) {
        j.status = Some(job::Status::Cancelled);
        j.cancel.notify_one();
        j.log.push(job::LogLine { kind: "error", text: "Cancelled.".into() });
    }
    StatusCode::OK
}
