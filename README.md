# archboard

An architecture board for any repository. It shows the repository's packages,
folders and files as cards you drag into a new layout, checks the layout
against your architecture rules while you draw, then has
[Claude Code](https://claude.com/claude-code) carry it out and verifies the
result.

It reads **Rust**, **TypeScript/JavaScript**, **Python** and **Go**, including
repositories that mix them. It needs no setup: point it at a repository and
it finds the packages itself.

```bash
cargo install --git https://github.com/adamvalt/archboard

cd path/to/any/repo
archboard serve          # the board, at http://127.0.0.1:7420
archboard init           # write a starter architecture.toml
archboard check          # check the rules; exits 1 on new violations (for CI)
archboard graph          # the scanned model as JSON
```

`--repo <path>` points any command at another repository. Apply needs the
`claude` binary on your `PATH` and the repository to be a git checkout.

## What it reads

A **package** is a directory with a manifest: `Cargo.toml` (with a
`[package]`), `package.json`, `go.mod`, `pyproject.toml`, `setup.py` or
`setup.cfg`. One directory with several manifests is one package in several
languages. Code outside every package goes in a package for the repository
root. Directories in `.gitignore`, and the usual dependency and build folders
(`node_modules`, `target`, `dist`, `.venv`, …), are skipped.

Inside a package, every folder and file is a node. The file that stands for a
folder (`mod.rs`, `index.ts`, `__init__.py`) is that folder's root file, not a
node of its own, and a Rust crate's `src/` level is skipped so its modules sit
right under the crate.

Dependencies come from each language's imports:

| Language | Read from | Resolved through |
|---|---|---|
| Rust | every `use` and path, parsed with `syn` | `crate::`, `self::`, `super::`, workspace crate names, and `pub use` re-exports (globs too) to the module that defines the item |
| TypeScript/JavaScript | `import`, `export … from`, `require()`, `import()` | relative paths, `tsconfig.json`/`jsconfig.json` `paths` and `baseUrl`, and workspace packages by their `package.json` name |
| Python | `import a.b`, `from a.b import c`, relative imports | each Python package's directory and its `src/`, then the repository root |
| Go | `import` paths | `go.mod` module paths in the repository |

Imports of anything outside the repository are ignored. Rust paths inside
macro calls aren't seen, and neither are TypeScript aliases set up outside
`tsconfig.json` (a bundler's own alias config, for example).

## The board

- **Cards** are the children of the level you have open: packages at the top,
  then folders and files. They sit in rows by architecture layer, and arrows
  show who uses whom. A red arrow breaks a layer or `forbid` rule, and a
  dashed amber arrow is part of a dependency cycle. Hover a card to tint what
  it uses (blue) and what uses it (amber).
- **Drag** a folder or file onto a card or chip to move it inside. Drop it on a
  breadcrumb to move it up, or on "Drop here to put it in a new folder" for a
  new folder. Dropped on the workspace, it becomes a package of its own. Drag
  a card's header to rearrange the board (positions are remembered per level).
- **Double-click** opens a level. **Right-click** to rename, add a folder, move
  up a level, extract a package, or leave a note for Claude Code. `Ctrl+Z`
  undoes the last edit.
- The **Plan** tab lists the edits and the paths they come to. It says live
  whether the plan adds or removes rule violations, and Preview prompt shows
  exactly what Claude Code will be asked.

The plan and card positions are saved in the browser, per repository.

## Apply

Apply runs four steps and streams them to the Run tab (`src/job.rs`):

1. **Preflight**: no uncommitted changes to tracked files, and the plan adds no
   violation unless you ticked "Accept new rule violations".
2. **Branch**: `git switch -c archboard/<time>` (on by default).
3. **Apply**: `claude -p <prompt> --output-format stream-json` in the
   repository. The prompt lists the moves and tells Claude Code what each
   language involved needs updating (imports, manifests, re-exports, docs).
4. **Verify**: rescan and check that every change landed at its drawn path, no
   rule broke that the plan or the baseline didn't accept, and the verify
   commands pass. Failures go back to the same Claude Code session
   (`--resume`) up to `max_fix_rounds` times.

Nothing is committed. Review the result with `git diff`.

Without a `[verify]` section, the verify commands are the languages' usual
checks: `cargo check`, `tsc --noEmit` (when TypeScript is installed in the
repository), `go build ./...` and `python3 -m compileall`. Claude Code may
edit files, move them with git, and run those languages' build and test
commands, unless `[claude] args` says otherwise.

## Rules (`architecture.toml`)

Every section is optional; `archboard init` writes a commented starting point.
Node paths are repository-relative, without file extensions (`src/api/client`,
`packages/web/src/app`). A package is its directory (`.` for the root). In
globs, `*` matches within one segment and `**` matches any depth.

| Section | Checks |
|---|---|
| `[scan] packages` | Package directories to scan (`dir/*` takes every directory in `dir` with a manifest). Unset, every package is found. |
| `[scan] exclude` | Globs of packages, folders and files to leave out (vendored or generated code). |
| `[[layer]]` | Listed top first. A node may use its own layer and the layers below it. The most specific matching pattern decides a node's layer, and a node without one takes its parent's. |
| `[[forbid]]` | `from` must not use `to` (each matching a node or any of its ancestors), apart from `except`. |
| `[rules] acyclic` | The nodes matching each glob (each standing for everything inside it) must not depend on each other in a cycle. |
| `[rules] module_doc` | Packages and folders matching these globs say what they are for: a doc at the top of their root file (`//!`, a docstring, a leading comment, `// Package`) or a README. |
| `[rules] max_file_lines` | No single source file is longer. |
| `[claude]` | `binary`, `model`, `args` (replacing the default permission mode and allowed tools) and `max_fix_rounds`. |
| `[verify] commands` | Shell commands run from the repository root after Apply; each must pass. `[]` runs none. |

```toml
[[layer]]
name = "app"
paths = ["src/cli", "src/server"]

[[layer]]
name = "domain"
paths = ["src/orders", "src/billing"]

[[layer]]
name = "core"
paths = ["src/db", "src/util"]

[[forbid]]
from = "src/billing"
to = "src/orders/internal"
reason = "billing reads orders through their public API"

[rules]
acyclic = ["src/*"]
module_doc = ["src/*"]
```

`architecture.baseline` lists known violations, one key per line.
`check --write-baseline` rewrites it, and `check` fails only on violations it
doesn't list, so you can adopt rules in a codebase that breaks them today. A
listed cycle also covers any smaller cycle among its members, so breaking a
cycle up never fails the check, but a module joining one does.

## In CI

```yaml
- uses: dtolnay/rust-toolchain@stable
- run: cargo install --locked --git https://github.com/adamvalt/archboard
- run: archboard check
```

## Developing

```bash
cargo test
cargo clippy --all-targets
cargo run -- --repo ../some-repo serve
```

The board is one HTML file (`web/index.html`) with no dependencies, built into
the binary.
