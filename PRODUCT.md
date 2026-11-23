# Product

<!-- impeccable:product-schema 1 -->

## Platform

web

## Users

Developers who install archboard from GitHub and point it at a repository,
often one whose architecture they are meeting for the first time or want to
change: open-source maintainers, engineers planning a restructure, tech leads
deciding where code should live. They work at a desktop, in a browser tab
beside their editor and terminal, on the machine that holds the checkout.

## Product Purpose

archboard turns a repository into a board of its packages, folders and files
with the real dependencies between them, lets the person drag pieces into a
new layout, checks that layout against architecture rules while they draw, and
hands the result to Claude Code to carry out, then verifies it landed. Success
is a restructure the person drew by hand that arrives as a reviewable diff
which builds, keeps the rules, and puts every module where they put it.

## Positioning

Other tools draw dependency graphs to look at. archboard's graph is the editor:
moving a card is the instruction, the architecture rules are checked live
against the planned layout before any code changes, and the same rules gate
the agent's work and CI afterwards.

## Operating Context

- Started from a terminal (`archboard serve`), opened at `127.0.0.1:7420`, one
  repository per server; plans and card positions are kept per repository in
  the browser.
- Reads Rust, TypeScript/JavaScript, Python and Go, mixed in one repository.
  Repositories range from a few dozen nodes to thousands (tRPC: about 1,550
  nodes and 2,600 edges).
- `architecture.toml` (layers, forbidden dependencies, acyclic groups, doc and
  size rules) and `architecture.baseline` (known violations) live in the
  repository; `archboard check` enforces them in CI.
- Apply runs four steps (preflight, branch, Claude Code, verify) with a
  streamed log, and leaves the result uncommitted for `git diff`.

## Capabilities and Constraints

- The board is one self-contained HTML file with inline CSS and JavaScript,
  built into the Rust binary: no framework, no CDN, no fetched fonts, works
  offline.
- Operations: open a level, drag a node into another, onto a breadcrumb or the
  workspace, rename, create a folder or package, leave a note, undo; preview
  the prompt; apply, watch, cancel.
- Terms: workspace, package, folder, file, layer, violation, baseline, plan,
  Apply.
- Must stay usable with no `architecture.toml` (no layers, no rules) and with
  many layers and hundreds of violations.

## Brand Commitments

The name is lowercase `archboard`. No logo or other brand assets exist.

The visual language follows established software-engineering and
software-architecture notation wherever one exists (the owner's standing
preference): layered-architecture diagrams, the C4 model, UML (dependency
arrows, stereotypes, notes) and dependency structure matrices, at the craft
level of Structurizr, Lattix and similar tools. No borrowed metaphors.

## Evidence on Hand

Public repositories it has been run on: Flask, cobra, fzf, zustand, tRPC
(dependency cycles found in tRPC and Flask). No users,
testimonials, benchmarks or press exist; do not invent them.

## Product Principles

1. The real codebase is the truth: every card and arrow comes from the source,
   and the board never shows a dependency it did not read.
2. Show the consequence before the change: a plan's effect on the rules is
   visible while it is drawn, not after the agent runs.
3. The person decides the shape; the agent only carries it out, and nothing is
   committed for them.
4. Readable at repository scale: hundreds of nodes and edges must stay legible
   and navigable.
