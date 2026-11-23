//! Plan checking in the browser, for the static demo: the page loads a
//! snapshot written by `archboard export` and asks this module what a plan
//! does, as `archboard serve` answers `/api/check` and `/api/prompt`.

use std::cell::RefCell;
use std::path::Path;

use serde::Deserialize;
use wasm_bindgen::prelude::*;

use crate::config::Config;
use crate::model::Model;
use crate::plan::{self, Plan};

/// The parts of a snapshot plan checking needs.
#[derive(Deserialize)]
struct Snapshot {
    model: Model,
    config: Config,
}

thread_local! {
    static LOADED: RefCell<Option<Snapshot>> = const { RefCell::new(None) };
}

fn with<T>(f: impl FnOnce(&Snapshot) -> Result<T, JsError>) -> Result<T, JsError> {
    LOADED.with_borrow(|s| s.as_ref().map_or_else(|| Err(JsError::new("no snapshot loaded")), f))
}

/// Loads a snapshot (`archboard export`'s JSON) for the calls below.
///
/// # Errors
///
/// When the JSON is not a snapshot.
#[wasm_bindgen]
pub fn load(snapshot: &str) -> Result<(), JsError> {
    let parsed: Snapshot = serde_json::from_str(snapshot)?;
    LOADED.set(Some(parsed));
    Ok(())
}

/// What a plan (the board's JSON) changes and does to the rules, as JSON.
///
/// # Errors
///
/// When the plan is malformed or cannot be carried out (a move into itself,
/// two nodes on one path).
#[wasm_bindgen]
pub fn check(plan_json: &str) -> Result<String, JsError> {
    let p: Plan = serde_json::from_str(plan_json)?;
    with(|s| {
        let resolved = plan::resolve(&s.model, &s.config, &p).map_err(|e| JsError::new(&format!("{e:#}")))?;
        Ok(serde_json::to_string(&resolved)?)
    })
}

/// The prompt Claude Code would be given for a plan.
///
/// # Errors
///
/// As [`check`].
#[wasm_bindgen]
pub fn prompt(plan_json: &str) -> Result<String, JsError> {
    let p: Plan = serde_json::from_str(plan_json)?;
    with(|s| {
        let resolved = plan::resolve(&s.model, &s.config, &p).map_err(|e| JsError::new(&format!("{e:#}")))?;
        Ok(plan::prompt(Path::new("."), &s.model, &s.config, &p, &resolved))
    })
}
