//! Carrying out a plan with Claude Code, in four steps:
//!
//! 1. **Preflight**: the working tree has no uncommitted changes to tracked
//!    files, and the plan adds no architecture violation (unless the person
//!    accepted them on the board).
//! 2. **Branch**: optionally `git switch -c archboard/<time>`.
//! 3. **Apply**: `claude -p <prompt> --output-format stream-json`, its events
//!    streamed to the board's log.
//! 4. **Verify**: rescan and check every change landed where it was drawn,
//!    that no unaccepted rule broke, and that the `[verify]` commands pass.
//!    Failures go back to the same Claude Code session (`--resume`) up to
//!    `max_fix_rounds` times.
//!
//! Nothing is committed: the result is left in the working tree for review.

use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result, bail};
use serde::Serialize;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::sync::Notify;

use archboard::config::{self, Config};
use archboard::lang::Lang;
use archboard::model;
use archboard::plan::{self, Plan, Resolved};

/// Where a job is.
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    /// Still going.
    Running,
    /// Applied and verified.
    Done,
    /// Stopped by a failed step.
    Failed,
    /// Stopped from the board.
    Cancelled,
}

/// One line of the job's log.
#[derive(Debug, Clone, Serialize)]
pub struct LogLine {
    /// `step`, `text` (Claude's words), `tool`, `result`, `ok`, `error` or `info`.
    pub kind: &'static str,
    /// The line.
    pub text: String,
}

/// A running or finished job, as the board polls it.
#[derive(Debug, Default)]
pub struct Job {
    /// Where it is; `None` before the first job.
    pub status: Option<Status>,
    /// Everything it logged.
    pub log: Vec<LogLine>,
    /// Woken to cancel the running step.
    pub cancel: Arc<Notify>,
}

/// The job slot shared with the server.
pub type Shared = Arc<Mutex<Job>>;

/// Options from the board's Apply button.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct ApplyOptions {
    /// Switch to a new branch first.
    #[serde(default = "yes")]
    pub new_branch: bool,
    /// Go ahead although the plan adds violations.
    #[serde(default)]
    pub accept_violations: bool,
    /// Go ahead with uncommitted changes in the working tree.
    #[serde(default)]
    pub allow_dirty: bool,
}

const fn yes() -> bool {
    true
}

fn log(job: &Shared, kind: &'static str, text: impl Into<String>) {
    let text = text.into();
    eprintln!("[{kind}] {text}");
    job.lock().expect("job lock").log.push(LogLine { kind, text });
}

/// Runs the whole job; the result is reported through `job`.
#[expect(clippy::significant_drop_tightening, reason = "the lock is held only to record the outcome")]
pub async fn run(job: Shared, repo: PathBuf, plan: Plan, options: ApplyOptions) {
    let outcome = steps(&job, &repo, &plan, &options).await;
    let mut j = job.lock().expect("job lock");
    if j.status == Some(Status::Cancelled) {
        return;
    }
    match outcome {
        Ok(()) => {
            j.status = Some(Status::Done);
            j.log.push(LogLine { kind: "ok", text: "Done. Review the changes with `git diff`; nothing was committed.".into() });
        }
        Err(e) => {
            j.status = Some(Status::Failed);
            j.log.push(LogLine { kind: "error", text: format!("{e:#}") });
        }
    }
}

async fn steps(job: &Shared, repo: &Path, plan: &Plan, options: &ApplyOptions) -> Result<()> {
    let config = Config::load(repo)?;
    let baseline = config::load_baseline(repo);

    log(job, "step", "1/4 Preflight");
    let dirty = git(repo, &["status", "--porcelain", "--untracked-files=no", "--ignore-submodules=all"]).await?;
    if !dirty.trim().is_empty() && !options.allow_dirty {
        bail!("the working tree has uncommitted changes; commit or stash them first:\n{dirty}");
    }
    let model = model::scan(repo, &config)?;
    let resolved = plan::resolve(&model, &config, plan)?;
    if resolved.changes.is_empty() && plan.instructions.trim().is_empty() && !plan.ops.iter().any(|o| matches!(o, plan::Op::Note { .. })) {
        bail!("the plan changes nothing");
    }
    log(job, "info", format!("{} change(s); the plan removes {} and adds {} violation(s)", resolved.changes.len(), resolved.removed.len(), resolved.added.len()));
    if !resolved.added.is_empty() {
        for v in &resolved.added {
            log(job, "info", format!("adds: {}", v.message));
        }
        if !options.accept_violations {
            bail!("the plan breaks the architecture rules above; change it, or tick \"accept new violations\"");
        }
    }

    if options.new_branch {
        let secs = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs());
        let branch = format!("archboard/{secs}");
        log(job, "step", format!("2/4 Branch {branch}"));
        git(repo, &["switch", "-c", &branch]).await?;
    } else {
        log(job, "step", "2/4 Branch: staying on the current branch");
    }

    log(job, "step", "3/4 Claude Code applies the plan");
    let prompt = plan::prompt(repo, &model, &config, plan, &resolved);
    let args = claude_args(&config, &model.langs());
    let mut session = claude(job, repo, &config, &args, &prompt, None).await?;

    let mut round = 0;
    loop {
        log(job, "step", if round == 0 { "4/4 Verify".to_string() } else { format!("4/4 Verify (after fix round {round})") });
        let problems = verify(job, repo, &config, &resolved, &baseline).await?;
        if problems.is_empty() {
            log(job, "ok", "Every change landed where it was drawn, the architecture rules hold and the verify commands pass.");
            break;
        }
        for p in &problems {
            log(job, "error", p.lines().next().unwrap_or_default().to_string());
        }
        if round >= config.claude.max_fix_rounds {
            bail!("verification still fails after {round} fix round(s)");
        }
        round += 1;
        log(job, "step", format!("Sending the failures back to Claude Code (round {round} of {})", config.claude.max_fix_rounds));
        let message = format!(
            "Verification of the restructuring failed. Fix these, keeping to the plan, then reply with a short summary:\n\n{}",
            problems.iter().map(|p| format!("- {p}")).collect::<Vec<_>>().join("\n")
        );
        session = claude(job, repo, &config, &args, &message, session.as_deref()).await?;
    }

    let stat = git(repo, &["diff", "HEAD", "--stat", "-M"]).await.unwrap_or_default();
    let untracked = git(repo, &["status", "--short", "--untracked-files=all"]).await.unwrap_or_default();
    log(job, "info", format!("git diff --stat:\n{stat}"));
    if untracked.lines().any(|l| l.starts_with("??")) {
        log(job, "info", "New files are untracked until you `git add` them.");
    }
    Ok(())
}

async fn verify(job: &Shared, repo: &Path, config: &Config, resolved: &Resolved, baseline: &[String]) -> Result<Vec<String>> {
    let after = model::scan(repo, config)?;
    let mut problems = plan::verify(&after, config, resolved, baseline);
    if !problems.is_empty() {
        return Ok(problems);
    }
    for cmd in &model::verify_commands(config, repo, &after) {
        log(job, "info", format!("$ {cmd}"));
        let cancel = job.lock().expect("job lock").cancel.clone();
        let child = Command::new("sh").arg("-c").arg(cmd).current_dir(repo).kill_on_drop(true).output();
        let output = tokio::select! {
            out = child => out?,
            () = cancel.notified() => bail!("cancelled"),
        };
        if !output.status.success() {
            let text = String::from_utf8_lossy(&output.stderr);
            // A compiler's errors are at the end of its report; drop cargo's progress lines before them.
            let tail: Vec<&str> = text.lines().filter(|l| !l.trim_start().starts_with("Compiling") && !l.trim_start().starts_with("Checking")).collect();
            let tail = tail[tail.len().saturating_sub(80)..].join("\n");
            problems.push(format!("`{cmd}` failed:\n{tail}"));
            break;
        }
    }
    Ok(problems)
}

/// `[claude] args`, or edits accepted with reading, editing, moving files with
/// git, and the build and test commands of `langs` allowed.
fn claude_args(config: &Config, langs: &[Lang]) -> Vec<String> {
    if let Some(args) = &config.claude.args {
        return args.clone();
    }
    let mut tools: Vec<&str> = vec![
        "Read", "Edit", "Write", "Glob", "Grep", "Bash(git mv:*)", "Bash(git rm:*)", "Bash(git status:*)", "Bash(git diff:*)",
        "Bash(mkdir:*)", "Bash(mv:*)", "Bash(ls:*)", "Bash(rmdir:*)", "Bash(grep:*)", "Bash(rg:*)", "Bash(find:*)",
    ];
    for l in langs {
        tools.extend(l.tools());
    }
    vec!["--permission-mode".into(), "acceptEdits".into(), "--allowedTools".into(), tools.join(",")]
}

/// Runs Claude Code once and returns its session id.
async fn claude(job: &Shared, repo: &Path, config: &Config, args: &[String], prompt: &str, resume: Option<&str>) -> Result<Option<String>> {
    let mut cmd = Command::new(&config.claude.binary);
    cmd.arg("-p").arg(prompt).args(["--output-format", "stream-json", "--verbose"]).args(args);
    if let Some(model) = &config.claude.model {
        cmd.args(["--model", model]);
    }
    if let Some(id) = resume {
        cmd.args(["--resume", id]);
    }
    cmd.current_dir(repo).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped()).kill_on_drop(true);
    let mut child = cmd.spawn().with_context(|| format!("starting {}", config.claude.binary))?;
    let mut lines = BufReader::new(child.stdout.take().context("no stdout")?).lines();
    let stderr = child.stderr.take().context("no stderr")?;
    let err_job = job.clone();
    tokio::spawn(async move {
        let mut lines = BufReader::new(stderr).lines();
        while let Ok(Some(l)) = lines.next_line().await {
            log(&err_job, "info", l);
        }
    });

    let cancel = job.lock().expect("job lock").cancel.clone();
    let mut session = None;
    let mut failed = None;
    loop {
        let line = tokio::select! {
            l = lines.next_line() => l?,
            () = cancel.notified() => {
                let _ = child.kill().await;
                bail!("cancelled");
            }
        };
        let Some(line) = line else { break };
        let Ok(event) = serde_json::from_str::<serde_json::Value>(&line) else {
            log(job, "info", line);
            continue;
        };
        if let Some(id) = event.get("session_id").and_then(|v| v.as_str()) {
            session = Some(id.to_string());
        }
        match event["type"].as_str() {
            Some("assistant") => {
                for block in event["message"]["content"].as_array().into_iter().flatten() {
                    match block["type"].as_str() {
                        Some("text") => log(job, "text", block["text"].as_str().unwrap_or_default()),
                        Some("tool_use") => log(job, "tool", describe_tool(block)),
                        _ => {}
                    }
                }
            }
            Some("user") => {
                for block in event["message"]["content"].as_array().into_iter().flatten() {
                    if block["type"] == "tool_result" && block["is_error"] == true {
                        let text = match &block["content"] {
                            serde_json::Value::String(s) => s.clone(),
                            other => other.to_string(),
                        };
                        log(job, "result", truncate(&text, 300));
                    }
                }
            }
            Some("result") => {
                let cost = event["total_cost_usd"].as_f64().map_or_default(|c| format!(", ${c:.2}"));
                let turns = event["num_turns"].as_u64().unwrap_or(0);
                if event["is_error"] == true || event["subtype"] != "success" {
                    failed = Some(event["result"].as_str().unwrap_or("Claude Code stopped with an error").to_string());
                }
                log(job, "info", format!("Claude Code finished: {turns} turns{cost}"));
            }
            _ => {}
        }
    }
    let status = child.wait().await?;
    if let Some(reason) = failed {
        bail!("Claude Code failed: {reason}");
    }
    if !status.success() {
        bail!("Claude Code exited with {status}");
    }
    Ok(session)
}

fn describe_tool(block: &serde_json::Value) -> String {
    let name = block["name"].as_str().unwrap_or("tool");
    let input = &block["input"];
    let detail = ["command", "file_path", "pattern", "path", "description"]
        .iter()
        .find_map(|k| input[*k].as_str())
        .map_or_else(|| truncate(&input.to_string(), 160), |s| truncate(s, 200));
    format!("{name}: {detail}")
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max { s.to_string() } else { format!("{}…", s.chars().take(max).collect::<String>()) }
}

async fn git(repo: &Path, args: &[&str]) -> Result<String> {
    let out = Command::new("git").args(args).current_dir(repo).output().await?;
    if !out.status.success() {
        bail!("git {} failed: {}", args.join(" "), String::from_utf8_lossy(&out.stderr));
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

/// The current branch, for the board's header.
pub async fn branch(repo: &Path) -> String {
    git(repo, &["rev-parse", "--abbrev-ref", "HEAD"]).await.map_or_default(|s| s.trim().to_string())
}
