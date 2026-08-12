# Blender Crowd UI/UX roadmap

This document turns the M2 UI/UX audit into a product contract that applies to
the remaining milestones. The editable design artifact is the
[Blender Crowd UI/UX Roadmap M2-M8 Figma file](https://www.figma.com/design/snabkLqO8N7uHJUm6UnrTE).

The governing product rule is that Blender Crowd should expose an artist
workflow, not its internal schema. Stable IDs, raw cache data, JSON, individual
validators, and implementation diagnostics remain available for advanced use,
but they are not the default authoring path.

## Current-state diagnosis

The M2 implementation has broad capability, but the current Scene Properties
panel presents most of it as one long form. The principal UX failures are:

- no persistent indication of the current workflow stage or project health;
- raw JSON and manually entered logical or agent IDs in primary workflows;
- weak selection context, including `Remove Last` and unlabeled removal actions;
- independent Bake, Cancel, and Attach controls without an explicit cache state;
- cache-reader state that is not visibly restored with the saved Blender file;
- numeric debug results that require implementation knowledge to interpret;
- a behavior graph editor that is disconnected from population and trace tasks;
- transient status text serving as validation report, progress indicator, and
  error history at the same time.

## Product principles

1. **Artist default, advanced fallback.** Common shots require no JSON, copied
   IDs, source-code knowledge, or manual cache lifecycle management.
2. **State is visible.** Project health, validation, bake, cache, selection,
   override, and render states are persistent and distinguishable.
3. **Viewport first.** Spatial concepts and agents are selected or authored in
   the viewport, with numeric fields as precise secondary controls.
4. **One primary action.** Every stage has one visually dominant next action;
   destructive, cancellation, and expert actions appear only when applicable.
5. **Evidence explains itself.** Debugging uses readable states, reasons,
   observations, paths, velocities, and graph-node links rather than raw codes.
6. **Blender native.** The workflow respects Blender workspaces, editors,
   panels, lists, undo, selection, keymaps, themes, accessibility, and save/load.
7. **Progressive disclosure.** Complexity is organized into focused sections;
   advanced data remains inspectable without overwhelming first use.

## Target artist workflow

1. **Setup** — create or open a crowd shot, select a template, and see project
   health and the next required action.
2. **Author** — edit populations, behavior, environment, assets, and groups
   through selected-item lists, pickers, presets, and viewport tools.
3. **Validate** — run one shot validation and navigate grouped, actionable
   findings directly to the affected entity or graph node.
4. **Bake** — use one stateful bake card with progress, cancellation only while
   active, automatic attachment, and explicit stale or invalid states.
5. **Review and correct** — click an agent, inspect a readable trace timeline,
   show path and velocity evidence, and create sparse corrections in context.
6. **Render and publish evidence** — render previews or acceptance evidence and
   open reports without interpreting cache-directory contents manually.

## Cross-milestone UI gates

| Milestone | UI outcome | Minimum evidence |
| --- | --- | --- |
| M2 | Independent artist can author, validate, bake, inspect, correct, and render the reference shot | Unassisted workflow run, screenshots, task timings, and failure-recovery notes |
| M3 | The workflow is installable, recoverable, accessible, and trustworthy across the support matrix | Clean-install studies, save/reload drills, keyboard and contrast audit, actionable-error fixtures |
| M4 | Layered corrections and interchange are visible, reversible, and conflict-aware | Seven-agent correction study, layer/conflict screenshots, before/after and base-cache proof |
| M5 | Scale and resource costs remain understandable at 10K and 100K agents | Profiling dashboard captures, responsive-cancel tests, tier/culling explanations |
| M6 | Advanced agency and motion remain traceable despite graph and motion complexity | Graph-debugger studies, trace-to-node proof, motion/contact diagnostic fixtures |
| M7 | Blender integration feels native and survives host conventions and lifecycle boundaries | Host-version UX matrix, theme/keymap/accessibility checks, extension workflow study |
| M8 | Generated semantic proposals and datasets remain reviewable, attributable, and safely approved | Proposal/diff usability study, provenance views, rejection paths, dataset review/export evidence |

## Evidence standard

Every milestone with UI scope must include:

- a dated task-based evaluation using a clean artifact rather than a developer
  checkout alone;
- at least one participant who did not implement the tested feature;
- success rate, time on task, errors, recoveries, and assistance required;
- screenshots or recordings of the normal, empty, loading, success, warning,
  error, stale, canceled, and recovered states that apply;
- keyboard/focus, label clarity, target size, truncation, and contrast review;
- a list of raw or advanced controls that remain exposed and why they are needed;
- repository issues or milestone follow-ups for every unresolved usability risk.

An automated operator pass proves that controls execute; it does not prove that
an artist can discover, understand, or recover the workflow.
