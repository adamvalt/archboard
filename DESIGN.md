---
name: archboard
description: A repository drawn as a live architecture diagram in the notations engineers already read, edited by dragging.
colors:
  chrome: "#eef0f3"
  sheet: "#ffffff"
  panel: "#ffffff"
  ink: "#17202b"
  ink-2: "#3d4a5a"
  muted: "#5f6b7a"
  rule: "#d5dbe3"
  rule-2: "#e6eaef"
  band-a: "#f7f9fb"
  band-b: "#eff3f7"
  gutter: "#e3e9f0"
  container: "#2c6bb3"
  container-ink: "#ffffff"
  container-ink-2: "#d9e7f7"
  component: "#d4e6f8"
  component-line: "#9cc0e6"
  component-ink: "#0e2a47"
  component-ink-2: "#3b5878"
  code: "#ffffff"
  code-line: "#b7cbe1"
  select: "#17202b"
  focus: "#2c6bb3"
  edge: "#7d8899"
  plan: "#1d7a4f"
  plan-soft: "#e3f3ea"
  good: "#1d7a4f"
  good-soft: "#e3f3ea"
  bad: "#c42b1c"
  bad-soft: "#fbe7e4"
  cycle: "#b26a00"
  cycle-soft: "#fbefd9"
  uses: "#6447c9"
  usedby: "#0d8577"
typography:
  headline:
    fontFamily: "system-ui, -apple-system, Segoe UI, Roboto, Helvetica Neue, sans-serif"
    fontSize: "15px"
    fontWeight: 700
    lineHeight: 1.45
    letterSpacing: "-0.01em"
  title:
    fontFamily: "system-ui, -apple-system, Segoe UI, Roboto, Helvetica Neue, sans-serif"
    fontSize: "13px"
    fontWeight: 650
    lineHeight: 1.45
  body:
    fontFamily: "system-ui, -apple-system, Segoe UI, Roboto, Helvetica Neue, sans-serif"
    fontSize: "13px"
    fontWeight: 400
    lineHeight: 1.45
    fontFeature: "tnum"
  body-small:
    fontFamily: "system-ui, -apple-system, Segoe UI, Roboto, Helvetica Neue, sans-serif"
    fontSize: "12px"
    fontWeight: 400
    lineHeight: 1.45
    fontFeature: "tnum"
  caption:
    fontFamily: "system-ui, -apple-system, Segoe UI, Roboto, Helvetica Neue, sans-serif"
    fontSize: "11.5px"
    fontWeight: 400
    lineHeight: 1.4
  type-line:
    fontFamily: "system-ui, -apple-system, Segoe UI, Roboto, Helvetica Neue, sans-serif"
    fontSize: "11px"
    fontWeight: 400
    lineHeight: 1.45
  label:
    fontFamily: "system-ui, -apple-system, Segoe UI, Roboto, Helvetica Neue, sans-serif"
    fontSize: "10.5px"
    fontWeight: 700
    letterSpacing: "0.03em"
  stereotype:
    fontFamily: "system-ui, -apple-system, Segoe UI, Roboto, Helvetica Neue, sans-serif"
    fontSize: "11px"
    fontWeight: 400
  mono:
    fontFamily: "ui-monospace, SF Mono, Cascadia Mono, Menlo, Consolas, monospace"
    fontSize: "12px"
    fontWeight: 400
rounded:
  hairline: "2px"
  chip: "3px"
  box: "4px"
  dialog: "6px"
  pill: "9px"
spacing:
  xxs: "4px"
  xs: "6px"
  sm: "8px"
  md: "10px"
  lg: "12px"
  xl: "14px"
  xxl: "16px"
  band-pad: "22px"
  card-gap-y: "22px"
  card-gap-x: "40px"
components:
  button:
    backgroundColor: "{colors.panel}"
    textColor: "{colors.ink}"
    rounded: "{rounded.box}"
    padding: "0 10px"
    height: "28px"
  button-primary:
    backgroundColor: "{colors.container}"
    textColor: "{colors.container-ink}"
    rounded: "{rounded.box}"
    padding: "0 10px"
    height: "28px"
  button-icon:
    backgroundColor: "{colors.panel}"
    textColor: "{colors.ink}"
    rounded: "{rounded.box}"
    width: "28px"
    height: "28px"
  segmented-pressed:
    backgroundColor: "{colors.ink}"
    textColor: "{colors.sheet}"
    height: "26px"
  input:
    backgroundColor: "{colors.sheet}"
    textColor: "{colors.ink}"
    rounded: "{rounded.box}"
    padding: "7px 9px"
  tab:
    textColor: "{colors.muted}"
    padding: "0 12px"
    height: "42px"
  tab-active:
    textColor: "{colors.ink}"
  element-package-head:
    backgroundColor: "{colors.container}"
    textColor: "{colors.container-ink}"
    padding: "8px 10px 7px"
  element-folder-head:
    backgroundColor: "{colors.component}"
    textColor: "{colors.component-ink}"
    padding: "8px 10px 7px"
  element-file-head:
    backgroundColor: "{colors.code}"
    textColor: "{colors.ink}"
    padding: "8px 10px 7px"
  element-box:
    backgroundColor: "{colors.sheet}"
    rounded: "{rounded.box}"
    width: "240px"
  chip:
    backgroundColor: "{colors.code}"
    textColor: "{colors.ink-2}"
    rounded: "{rounded.chip}"
    padding: "0 7px"
    height: "24px"
  chip-package:
    backgroundColor: "{colors.container}"
    textColor: "{colors.container-ink}"
    rounded: "{rounded.chip}"
    height: "24px"
  overlay:
    backgroundColor: "{colors.panel}"
    rounded: "{rounded.box}"
  status-good:
    backgroundColor: "{colors.good-soft}"
    textColor: "{colors.good}"
    rounded: "{rounded.box}"
    padding: "9px 11px"
  status-bad:
    backgroundColor: "{colors.bad-soft}"
    textColor: "{colors.bad}"
    rounded: "{rounded.box}"
    padding: "9px 11px"
  count-badge:
    backgroundColor: "{colors.chrome}"
    textColor: "{colors.ink-2}"
    rounded: "{rounded.pill}"
    height: "18px"
---

# Design System: archboard

## Overview

**Creative North Star: "The Living Architecture Diagram"**

The board is a software-architecture diagram that happens to be editable. Every visual device comes from a notation engineers already read: layered-architecture bands, C4 element boxes, UML dependency arrows, stereotypes and notes, and a dependency structure matrix. Nothing is a borrowed metaphor; if a device has no precedent in that canon, it does not belong on the board.

The world is a white diagram sheet laid on cool grey chrome. The sheet carries flat, full-width layer bands with a label gutter at the left; C4 boxes sit in the bands, coloured by kind; dashed open-headed arrows run between them, thickening with the number of imports they stand for. The chrome around the sheet (header, side panel, floating overlays) is quiet, square-cornered and hairline-ruled, so the diagram carries all the colour. Density is high but calm: a 13px system sans with tabular figures, 4px corners, and depth only where something is lifted.

Plan edits are drawn in the same notation as the diagram: a dashed green outline and a UML stereotype (`«moved»`, `«new»`, `«renamed»`). Rule breaks are red, cycles amber. Every diagram view has a title (top left) and a key (bottom), as a published diagram would.

**Key Characteristics:**
- White sheet on cool grey chrome; colour lives in the diagram, not the chrome.
- C4 kind colouring: package solid container blue, folder light component blue, file white with a blue rule.
- UML dashed dependency lines with open arrowheads; weight follows import count.
- Red means a broken rule, amber a cycle, green a planned change. Never decoration.
- One system sans with tabular figures; mono only for paths and code.
- Fully token-driven light and dark schemes from the same custom properties.

## Colors

A cool, near-neutral sheet-and-chrome palette carrying one structural blue family (the C4 kinds) and a small set of semantic signal colours.

### Primary
- **Container Blue** (`container`): the C4 container colour. Fills package box heads and package chips, the primary button, the key's package swatch, and the DSM's density shading (mixed 8–88% into transparent by import count). Doubles as `focus` for focus rings and field borders.
- **Component Blue** (`component`, line `component-line`, inks `component-ink` / `component-ink-2`): the C4 component colour for folders. Folder heads, folder chips (mixed 55% into the sheet), the key's folder swatch.
- **Code Rule Blue** (`code-line`): the border that marks a file box; the file head itself is the white `code` fill.

### Secondary
- **Uses Violet** (`uses`) and **Used-by Teal** (`usedby`): the hover pair. While an element is hovered, what it uses tints violet and what uses it tints teal (16% chip fills, 2px box rings), and the side panel's dependency headings carry the same swatches.

### Tertiary (signal colours)
- **Rule Red** (`bad`, soft `bad-soft`): a dependency that breaks an architecture rule (arrow, arrowhead, edge label, DSM cell ring), violation badges, a new violation row in the ledger, failed run lines and the failed drop target.
- **Cycle Amber** (`cycle`, soft `cycle-soft`): dependency cycles (dotted arrow, DSM cell ring), result lines in the run log, and the tint of UML notes.
- **Plan Green** (`plan`, soft `plan-soft`) and **Good Green** (`good`, `good-soft`): one value in two roles. `plan` marks what the plan changes (dashed outlines, stereotypes, the change ledger's action column); `good` marks success (valid drop target, resolved violations, a passing status or run).

### Neutral
- **Chrome Grey** (`chrome`): the page and canvas ground around the sheet; also the hover fill of crumbs, menu items and ledger rows, and the fill of teaching and info blocks.
- **Sheet White** (`sheet`) and **Panel White** (`panel`): the diagram sheet and the header, side panel, overlays, menus and dialogs.
- **Band Light / Band Shade** (`band-a`, `band-b`): alternating layer bands on the sheet, also the zebra of the layer legend.
- **Gutter Grey** (`gutter`): the layer label gutter (at 55%) and the DSM's vertical layer labels.
- **Ink** (`ink`), **Ink 2** (`ink-2`), **Muted** (`muted`): primary text; secondary text and labels; tertiary text, counts, placeholders and idle states.
- **Rule** (`rule`) and **Rule 2** (`rule-2`): structural hairlines (panel edges, controls, overlays) and inner dividers (band seams, ledger rows, DSM cells).
- **Edge Grey** (`edge`): ordinary dependency arrows, at 0.35 opacity for one or two imports and 0.75 above.
- **Select Ink** (`select`): the 2px selection outline, equal to `ink`.

### Dark scheme
Under `prefers-color-scheme: dark` every token is remapped on `:root`: chrome `#13171c`, sheet and panel `#1a1f26`, ink `#e6ebf1`, ink-2 `#c3ccd7`, muted `#8e9aa9`, rule `#2e3742`, rule-2 `#262e38`, bands `#1c222a` / `#20272f`, gutter `#232a33`, container `#2f67a6`, component `#233a54` (line `#3d5d82`, ink `#dce9f8`, ink-2 `#a3bbd6`), code `#1f262f` (line `#3a4a5d`), plan and good `#4fbf86` (soft `#1b3428`), bad `#f07a6b` (soft `#3a211e`), cycle `#e3a54a` (soft `#3a2d18`), uses `#a690f5`, usedby `#4cc3b5`, edge `#6f7c8d`, focus `#7fb0ea`. Components reference tokens only, so they follow without their own dark rules.

### Named Rules
**The Notation Colour Rule.** Every hue on the board means something a diagram key can name: element kind, rule break, cycle, plan, or hover direction. A colour that would not earn an entry in the key does not ship.

**The Quiet Chrome Rule.** Header, panel and overlays stay on `panel`, `chrome` and the rules. The only saturated fill in the chrome is the primary action and live counts.

## Typography

**Display Font:** none; the board has no display tier.
**Body Font:** the system sans stack (`system-ui, -apple-system, "Segoe UI", Roboto, "Helvetica Neue", sans-serif`)
**Label/Mono Font:** the system mono stack (`ui-monospace, "SF Mono", "Cascadia Mono", Menlo, Consolas, monospace`), for paths and code only

**Character:** A tool face, not a voice. One sans at small, dense sizes with tabular figures everywhere (`font-variant-numeric: tabular-nums` on the body), so line counts, import counts and matrix cells align in columns. Weight 650 is the working emphasis between regular and bold.

### Hierarchy
- **Headline** (700, 15px, -0.01em): the `archboard` wordmark and dialog headings. The largest text on the board.
- **Title** (650, 13px): element box names, layer names in the gutter, the diagram title. Details pane names step up to 14px.
- **Body** (400, 13px, line-height 1.45): the base size of the whole page, controls included.
- **Body small** (400 or 650, 12px): chips, ledgers, the run log, pane section headings (650, `ink-2`), status blocks (12.5px), the key heading.
- **Caption** (400, 11.5px): sublines (diagram subtitle, layer descriptions, details path), key entries, edge labels (650).
- **Type line** (400, 11px): the C4 `[Package: Rust]` line and size under each box name; counts and line totals on chips.
- **Stereotype** (400 italic, 11px): UML `«moved»`, `«new»`, `«renamed»`, always in guillemets.
- **Label** (700, 10.5px, 0.03em, uppercase): column keys in fixed-column ledgers (the violation and change ledgers) and the demo tag. Used only as data keys inside a grid, never above a heading.
- **Mono** (400, 11–12px): file paths, dependency lists, code, run-log timestamps and tool lines, the repository path in the diagram subtitle.

### Named Rules
**The Zoom-Invariant Label Rule.** Text that names a layer (gutter name, description, rule line) and edge labels are counter-scaled by the zoom (`1px / max(zoom, .45)`), so they stay readable at any zoom. Element boxes scale with the diagram.

**The Tabular Rule.** Figures are tabular everywhere. Counts are data and must line up.

**The Mono-For-Paths Rule.** Monospace marks something the person could paste into a terminal: a path, a branch, code. Never use it for decoration or tone.

## Layout

The page is a 48px header strip over a full-height `main` split into the canvas (flex) and a 384px side panel (320px under 980px wide). Under 720px the panel drops below the canvas at 45% height, the key, brand and metadata are hidden, and breadcrumbs collapse to the last crumb.

The canvas is a pannable, zoomable world (transform-origin top left) holding one diagram sheet. On the sheet, layers are stacked full-width bands, top layer first, each with a label gutter (180–340px, widened as the fitted zoom shrinks so its counter-scaled labels fit) and at least 132px of height at the fitted zoom. Element boxes are 240px wide on a grid with 40px column gaps, 22px row gaps and 22px band padding; the column count (2–9) is chosen to show the whole board largest.

Floating overlays sit 14px from the canvas edges: the diagram title top left, the new-folder drop zone and the key bottom left, zoom controls bottom right. The matrix view replaces the world with a scrolling DSM table padded 64px from the top so the title clears it.

Spacing steps are small and tight: 4, 6, 8, 10, 12, 14 and 16px, with 22px reserved for the diagram's band padding and row gaps.

## Elevation & Depth

Mostly flat, with two ambient shadows. The sheet, element boxes, overlays and the DSM rest on a barely-there `shadow`; anything lifted off the board (a dragged box, the drag ghost, the context menu, dialogs) takes `shadow-lift`. Bands and gutters are separated by tone and hairlines, never by shadow. State on the board is drawn as outlines and rings, not elevation.

### Shadow Vocabulary
- **Resting** (`box-shadow: 0 1px 2px rgba(23, 32, 43, .06), 0 2px 6px rgba(23, 32, 43, .05)`): the sheet, element boxes, overlays, the DSM table.
- **Lifted** (`box-shadow: 0 2px 4px rgba(23, 32, 43, .10), 0 6px 12px rgba(23, 32, 43, .12)`): dragging, the drag ghost, the context menu, dialogs.
- **Hover and dependency rings**: a 1px ring of `ink` at 40% on the hovered box; 2px rings of `uses` or `usedby` on related boxes, added to the resting shadow.
- **Field focus** (`box-shadow: 0 0 0 3px` `focus` at 22%): text fields only, with the border turning `focus`.

### Named Rules
**The Flat Band Rule.** Layer bands divide the sheet by tone alone: alternating `band-a` / `band-b` with a `rule-2` seam. No boxed group borders, no band shadows.

**The Lift Means Moving Rule.** `shadow-lift` is only for things in motion or floating above the board. A box at rest never takes it.

## Shapes

Square-shouldered and technical. Nearly everything takes a 4px corner (controls, element boxes, overlays, status blocks, ledgers); chips and menu items take 3px; swatches and inline code 2px; dialogs 6px; counts and violation badges are pills. Borders are 1px hairlines; plan state is a 1.5px dashed outline offset 2px; selection a 2px solid `select` outline offset 2px (1px on chips); drop targets a 2px `good` or `bad` outline.

Two UML forms recur. Dependency lines are dashed (6 4) cubic curves with open chevron arrowheads; cycles are dotted (2 4). Notes are UML dog-eared notes: a rectangle with the top-right 12px corner clipped and a folded triangle in its place. Icons are 14px, 1.5px-stroke line drawings in a 16-unit box (package, folder, file and a few actions), inline as an SVG sprite.

## Components

### Buttons
Plain and compact, like a desktop tool.
- **Shape:** gently squared (4px), 28px tall, 10px horizontal padding, icon and label 6px apart.
- **Default:** `panel` fill, `ink` text, 1px `rule` border. Hover darkens the border to `muted` and mixes 4% `ink` into the fill; active 8%.
- **Primary:** `container` fill and border, `container-ink` text, weight 600; hover mixes 12% black in. One per pane: Apply with Claude Code at the foot of Plan.
- **Icon:** 28px square, centred icon.
- **Disabled:** 45% opacity, no hover change.
- **Focus:** a 2px `focus` outline offset 2px (global `:focus-visible`).

### Segmented switch
The Diagram/Matrix switch: buttons joined in one 4px `rule`-bordered group, 26px tall, split by `rule` hairlines. The pressed segment inverts to `ink` fill with `sheet` text.

### Tabs
The side panel's Plan, Rules, Run: 42px transparent tabs in `muted`, the active tab `ink`, weight 600, with a 2px `ink` underline. Each may carry a pill count (18px, `chrome` fill); a count turns `bad` for violations and `container` while a run is live.

### Inputs / Fields
- **Style:** `sheet` fill, 1px `rule` border, 4px corners, 7px 9px padding, inherit the 13px body font.
- **Focus:** border turns `focus` with a 3px `focus` halo at 22%.
- **Checkboxes:** native, 14px, accent `container`, in 26px-tall label rows.

### Chips
The rows inside an element box: one child each.
- **Style:** 24px tall, 3px corners, 12px text, name left, count and size right in 11px `muted`. Files on `code` with a `rule-2` border; folders on `component` mixed 55% into the sheet; packages solid `container`.
- **State:** hover darkens the border; selection is a 2px `select` outline; planned chips take the dashed `plan` outline and a green stereotype; hover direction tints violet (`uses`) or teal (`usedby`).

### Navigation
Breadcrumbs in the header after a `rule` divider: 3px 7px crumbs in `ink-2`, the last crumb `ink` weight 600, `rule`-coloured separators, `chrome` hover fill. Crumbs are drop targets. A level opens by double-click on a box.

### C4 element box (signature)
The board's unit: a 240px box with a 4px corner, a head and a body of chips.
- **Head:** 8px 10px 7px padding; a kind icon, the name (650, 13px, ellipsised), an optional note mark and red violation badge; below it the C4 type line `[Package: Rust]` with the size (`109.7k lines`) right-aligned.
- **Package:** head filled `container` with `container-ink` text and `container-ink-2` type line; border `container`.
- **Folder:** head `component` with `component-ink`, a `component-line` divider and border.
- **File:** white `code` head with a `rule-2` divider and `code-line` border.
- **Body:** 6px padding, up to 3 chips on the workspace level (largest first) or 10 inside a level, then a muted "N more; double-click to open".
- **States:** resting shadow; hover ring; 2px selection outline; dashed plan outline with stereotype; violet or teal dependency ring; lifted while dragging.

### Layer band and gutter (signature)
Full-width flat bands alternating `band-a` / `band-b`. The gutter on the left holds the layer name (650, 13px), up to three lines of description (`ink-2`, 11.5px), and at the foot a down-arrow line "may use the layers below" for every layer but the last. Elements matched by no layer sit in a muted "Unlayered" band; with no `architecture.toml` the single band reads "All elements".

### Dependency arrow (signature)
A UML dependency: a dashed cubic curve with an open chevron head, ends spread along the side of the box they leave from. Stroke width runs from 0.9 to about 5.2 screen pixels on a log ramp of import count (non-scaling), with a count label on heavy flows. `edge` grey for ordinary use, `bad` red (6 4 dash) for a rule break, `cycle` amber (2 4 dots) for a cycle; red and amber draw on top. While a box is hovered, unrelated arrows fade to 8%.

### Dependency structure matrix (signature)
A sheet-white table of 30px by 26px cells with `rule-2` hairlines, sticky row and column headers, layer groups split by 2px `rule` lines and named by vertical `gutter` labels. The diagonal is filled `rule-2`. A dependency cell shows its count on `container` blue mixed 8–88% by log count (text flips to `container-ink` above 46%); a rule break or cycle is instead a bold count inside a 1.5px inset ring of `bad` or `cycle`.

### Diagram title and key
Every view has a title overlay top left (650, 13px; subtitle 11.5px `muted`: element and dependency counts, "showing the plan" when one exists) and, in the diagram, a collapsible key bottom left listing package, folder, file, planned, uses, breaks a rule, cycle, it uses and uses it, with the same swatches and line samples the board draws.

### UML note
Notes left on an element: a dog-eared rectangle (12px fold), `cycle` mixed 10% into the sheet, a `cycle` border at 45%, 12px text. A small note icon marks the element box.

### Status blocks and ledgers
- **Status:** a 4px bordered block with an icon, 9px 11px padding, 12.5px text; `good` on `good-soft`, `bad` on `bad-soft`, or `info` in `ink-2` on `chrome`.
- **Ledgers:** fixed two-column grids (64px key column, uppercase label keys) in a 1px `rule` frame with `rule-2` row lines. Rows restyle by state without breaking the grid: a new violation on `bad-soft`, a resolved one on `good-soft`, a known one in `ink-2`. The change ledger keys actions in `plan` green beside mono paths. The run log uses the same grid with mono timestamps, step rows on `chrome`, and line colour by kind.

### Context menu and dialogs
The right-click menu is a `panel` card, 4px corners, 4px padding, `shadow-lift`, with 28px items that fill `chrome` on hover or focus and `rule-2` separators. Dialogs are `panel` with a 6px corner, 18px padding, `shadow-lift`, up to 760px wide (440px small), over a backdrop of `rgba(15, 20, 28, .38)`.

### Motion
Motion is minimal and functional: 0.15s colour, border, shadow and opacity transitions; the key chevron rotates 90° when it closes; the scanning bar pulses opacity over 1.6s. `prefers-reduced-motion: reduce` turns every transition and animation off.

## Do's and Don'ts

### Do:
- **Do** take every visual device from established architecture notation: layered bands, C4 boxes and type lines, UML dashed arrows, stereotypes in guillemets and dog-eared notes, the DSM.
- **Do** colour elements by C4 kind only: package `container` with `container-ink`, folder `component`, file `code` with a `code-line` border.
- **Do** reserve `bad` for rule breaks, `cycle` for cycles and `plan` for planned changes, and show each in the key wherever it appears on the board.
- **Do** mark a planned change with a 1.5px dashed `plan` outline offset 2px and its stereotype (`«moved»`, `«new»`, `«renamed»`).
- **Do** give every diagram view a title and a key.
- **Do** keep layer labels and edge labels at their on-screen size at any zoom.
- **Do** use tabular figures for every count and size, and mono only for paths, branches and code.
- **Do** define every colour as a `:root` token and remap it in the dark scheme; components reference tokens, never literals.
- **Do** keep corners at 4px for boxes and controls (3px chips, 2px swatches, 6px dialogs).

### Don't:
- **Don't** turn the board into a generic whiteboard canvas of rounded pastel cards and floating toolbars.
- **Don't** make it a dark neon "hacker" tool or a wall of tiny labels; the light sheet is the default and both schemes come from the same tokens.
- **Don't** use borrowed metaphors (maps, transit lines, star charts) for structure the notation already names.
- **Don't** separate layers with boxed borders or shadows; bands divide by tone and a hairline.
- **Don't** use `shadow-lift` on anything at rest.
- **Don't** add a hue the key cannot explain, or use red, amber or green decoratively.
- **Don't** fetch fonts, scripts or images; the board is one self-contained file using system font stacks.
