# Blender Crowd UI/UX roadmap

This document turns the M2 UI/UX audit into a product contract that applies to
the remaining milestones. The editable design artifact is the
[Blender Crowd UI/UX Roadmap M2-M8 Figma file](https://www.figma.com/design/snabkLqO8N7uHJUm6UnrTE).

The governing product rule is that Blender Crowd should expose an artist
workflow, not its internal schema. Stable IDs, raw cache data, JSON, individual
validators, and implementation diagnostics remain available for advanced use,
but they are not the default authoring path.

## Status

- **M2:** accepted on 2026-08-12. Its functional authoring, bake, cache,
  selected-agent debug, sparse correction, and render gates are closed.
- **UI/UX redesign:** deferred and incomplete. It is scheduled work for M3 and
  the specialized later milestones; it does not reopen M2.
- **Figma:** the editable file exists with Blender-dark foundations, typography,
  a three-page structure, and screen placeholders. Screenshot upload, annotation,
  component construction, screen composition, and final review remain open
  because the Figma Starter-plan MCP quota was exhausted.

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

## Deferred UI/UX TODO

This is the authoritative backlog for the work deferred when M2 was accepted.
Unchecked items are intentionally incomplete.

### Figma audit and design artifact

- [ ] Resume automated Figma work when the MCP quota resets or the team is
  upgraded to a paid plan with a Full or Dev seat.
- [ ] Upload the eight captured Blender audit screenshots to the `00 — Audit &
  Roadmap` page.
- [ ] Add numbered annotations covering entry/setup, asset authoring, layouts,
  environment/groups, bake/debug, detached-cache failure, successful debug, and
  the disconnected behavior graph editor.
- [ ] Complete the simplified Setup → Author → Validate → Bake → Review/Correct
  → Render workflow map.
- [ ] Finish reusable Blender-style Button, Status Badge, and Workflow Step
  components using the existing variables and Inter styles.
- [ ] Compose the Shot Dashboard, Authoring Workspace, Bake & Cache, and Review
  & Correct screens on the `01 — Proposed UI` page.
- [ ] Add the M2–M8 roadmap board with milestone ownership, UI gates, evidence,
  and dependency notes.
- [ ] Validate every major frame at readable and full-board scales for clipped
  text, overlap, contrast, hierarchy, font family, and incomplete placeholders.
- [ ] Remove all Figma placeholder shimmer states and capture final screenshots.

Figma file:
[Blender Crowd UI/UX Roadmap M2-M8](https://www.figma.com/design/snabkLqO8N7uHJUm6UnrTE)

### M3 — production workflow and recovery

- [ ] Replace the monolithic project form with a dedicated Crowd workspace or
  focused native Blender panels organized by workflow stage.
- [ ] Add persistent project health, current stage, selection context, and one
  primary next action.
- [ ] Replace required raw JSON and copied-ID tasks with `UIList` collections,
  selected-item editors, searchable pickers, presets, and viewport selection.
- [ ] Consolidate normal validation into one actionable report and move granular
  validators to Advanced.
- [ ] Make bake/cache lifecycle stateful: measurable progress, contextual
  cancellation, automatic attachment, save/reload restoration, stale-state
  explanation, and direct artifact/report actions.
- [ ] Add diagnostic history and readable states/reasons rather than relying on
  one transient status string or numeric codes.
- [ ] Complete keyboard, focus, target-size, contrast, truncation, theme,
  scaling, and assistive-technology checks across the support matrix.

### M4 — layered correction and interchange

- [ ] Build the layer editor with order, source, priority, mute/solo, affected
  IDs/ranges, provenance, validity, and base-cache relationship.
- [ ] Add viewport-first per-agent, region, and curve correction tools.
- [ ] Add conflict, invalidation, local-resimulation, before/after, layer
  isolation, reversible flatten/export, and failed-operation recovery views.

### M5 — scale and profiling

- [ ] Expose simulation/render tiers, promotion, culling, proxies, quality
  limits, preflight estimates, measured results, and backend fallbacks.
- [ ] Add responsive progress, throughput, cancellation/resume, aggregation,
  drill-down, and population/tier/camera/cache bottleneck attribution.

### M6 — advanced graph and motion debugging

- [x] Build the deterministic trace summary, synchronized visited-node timeline,
  and brain debugger surface for graph state,
  decisive nodes, observations, scores, blackboard changes, and interrupts.
- [x] Expose selected-agent trace summaries for cached events, graph nodes,
  contacts, solver/layer ownership, failures, recoveries, and corrections.
- [ ] Link viewport agents, cached events, graph nodes, actions, clips, contacts,
  solver/layer ownership, failures, recoveries, and corrections bidirectionally.
- [x] Add deterministic large-graph search/highlight paths, typed-port diagnostics,
  and explicit reduced-evidence states by tier.
- [ ] Add reusable subgraphs/actions and presets.

### M7 — native Blender integration

- [ ] Audit workspaces, editors, panels, operators, selection, undo, keymaps,
  themes, translation, help, preferences, assets, installation, and save/load
  against stock Blender conventions and supported versions.
- [ ] Separate Crowd-product UI from evidence-backed Blender-general host gaps
  and document fallbacks for every version-specific difference.

### M8 — reviewed semantic and dataset workflows

- [ ] Build bounded semantic proposal/diff review with provenance, validation,
  grouped accept/reject, manual edit, revalidation, undo, and a non-model path.
- [ ] Build domain-pack discovery, compatibility, licensing, limitations,
  examples, install/update/removal, and claim-boundary UI.
- [ ] Build synthetic-data sensor, annotation, taxonomy, split, license, bias,
  preview, progress, resume, manifest, rejected-sample, and consumer-validation
  workflows.
