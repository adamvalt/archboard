//! archboard's core: scanning a repository into packages, folders, files and
//! their imports ([`model`], [`lang`]), checking architecture rules
//! ([`rules`]), and working out a plan drawn on the board ([`plan`]).
//!
//! The `archboard` binary serves the board and runs Claude Code around this
//! core; built for `wasm32`, the [`wasm`] module exposes plan checking to the
//! static demo page.

pub mod config;
pub mod glob;
pub mod lang;
pub mod model;
pub mod plan;
pub mod rules;
#[cfg(target_arch = "wasm32")]
pub mod wasm;
