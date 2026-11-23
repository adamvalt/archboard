---
version: 1
slug: "web-index-html"
primary_target: "web/index.html"
related_targets: []
---

# Board (web/index.html)

Scope: the whole board page. Mode: Operate.

Audience and task: a developer meeting a repository's architecture, often for the first time, then reshaping it by dragging. First need: the shape of the system (layers, major packages, heavy dependency flows); problems second. Must stay readable at hundreds of nodes; must not look like a dark neon "hacker" tool or a wall of tiny labels. Every existing interaction stays (drag to move, double-click to open, right-click menu, pan/zoom, hover, Ctrl+Z); layout, panel and visuals may change.

Constraints: one self-contained HTML file, no fonts or scripts fetched.

## Direction contract

THESIS: The board is a live architecture diagram in the notations engineers already read: layered-architecture bands, C4 element boxes, UML dependency arrows and stereotypes, and a dependency structure matrix. It refuses the generic whiteboard canvas of rounded pastel cards and floating toolbars.

OWN-WORLD: White diagram sheet on cool grey chrome. Layers are full-width flat bands with a label gutter carrying the layer's name, its description and the allowed direction. Elements are C4 boxes coloured by kind (package as container blue with white text, folder as component light blue, file as white with a blue rule), each with a [Type: language] line and size. Dependencies are UML dashed lines with open arrowheads; red breaks a rule, amber is a cycle. Plan edits wear UML stereotypes («moved», «new», «renamed») and dashed outlines; notes are UML dog-eared notes. Every diagram has a title and a key. One system sans, tabular figures, mono only for paths.

STORY: The visitor sees the system's layers and biggest parts at once, follows arrows to see who depends on whom, switches to the matrix to see density and cycles, drags elements into a new shape, sees the rule consequences in the plan, and applies it.

FIRST VIEWPORT: Header strip (archboard, repo and branch, breadcrumbs, Diagram/Matrix switch, actions). Canvas below with the diagram title top-left ("Packages in <repo>, by layer"), the layer bands stacked full width with the gutter at left, C4 boxes in each band, dashed arrows between them, the key bottom-left beside the new-folder drop zone, zoom bottom-right. Side panel at right: Plan, Rules, Run; Apply is the primary action at the foot of Plan.

FORM: Category standard by user steer: established software-architecture notation (layered + C4 + UML + DSM), canon choice from the direction round; seed key ac07b812.

FINISH: unreviewed and undocumented is unfinished; this build ends with the finish review, the verdict, DESIGN.md, and every shipping raster carrying its provenance
