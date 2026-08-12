# M2 acceptance — 2026-08-12

## Decision

M2 is **accepted**. The complete engineering subgate passed on Blender 5.2 LTS,
and a project operator who did not implement M2 subsequently completed all six
steps of the Blender UI spot check in the
[reference reproduction procedure](../user/m2-reference-reproduction.md).

This acceptance closes the M2 functional milestone. It does not claim that the
current UI is the final production experience. The operator found the workflow
difficult and triggered a separate comprehensive UI/UX audit. All remaining
design and Figma work is recorded in the [UI/UX roadmap](../ui-ux-roadmap.md)
and intentionally continues in M3 and later milestones without reopening M2.

The machine-readable decision is
[2026-08-12-m2-acceptance.json](2026-08-12-m2-acceptance.json).

## Engineering evidence

The 2026-08-11 host acceptance run:

- baked the authored 1,000-agent reference through ticks 0–9,999;
- wrote a complete cache and 1,724,156 compacted behavior, queue, and group
  evidence records;
- attached and replayed the completed cache without a live simulation session;
- inspected a selected cached agent and exposed graph/node evidence;
- authored a sparse hero pin while preserving every base-cache artifact;
- rendered the cache-only result through Eevee and Cycles.

See the [engineering subgate report](2026-08-11-m2-acceptance.md) and
[machine-readable runner result](2026-08-11-m2-full-acceptance.json).

## Operator reproduction

The project operator completed the documented Blender UI workflow through step
6: reference creation, social-group inspection, M2 validation, fresh cache bake
and attachment, selected-agent evidence inspection, overlays, and sparse pinned
override creation.

The reproduction exposed several workflow problems rather than functional gate
failures:

- an empty pre-created cache directory was initially rejected;
- bake completion was too easy to miss in Blender's transient status line;
- the selected-agent ID entry and source of valid IDs were not discoverable;
- path and velocity overlays and graph/node evidence were hard to locate;
- the panel exposed internal IDs, raw data, and operation order too directly.

The cache-directory behavior, completion notification, decimal agent-ID field,
selected-agent evidence, nearest cached-event lookup, and selected marker were
addressed during the M2 completion pass. The broader workflow redesign remains
deferred and is not represented as finished.

## Exit-gate ledger

| Gate | Result | Status |
| --- | --- | --- |
| Full shot authored without code edits | Reference project, typed graph, populations, assets, environment, layouts, and groups validated in Blender | Passed |
| Actionable invalid-state handling | Graph, ID, destination, retarget, cache, and operator failures have checked validation paths | Passed |
| Groups and queues | Deterministic queue lifecycle and group split/cohesion evidence persisted | Passed |
| Selected-agent explanation | Cached observations, graph/node evidence, path, and velocities inspected | Passed |
| Custom character and variation | Stable retarget, clip, contact, and weighted variation contracts validated | Passed |
| Locomotion and terrain presentation | Presentation-only terrain and contact fixtures preserve simulation truth | Passed |
| Sparse correction isolation | Hero pin and override fixtures leave base-cache hashes unchanged | Passed |
| Non-implementer reproduction | Project operator completed UI spot-check steps 1–6 | Passed |

## Deferred UI/UX work

M2 closes with known UI debt. The deferred backlog includes the annotated Figma
audit, staged Crowd workflow, structured collection editors, viewport-first
selection, readable diagnostic history, automatic cache-state restoration,
accessibility validation, specialized correction/scale/debug views, and later
semantic proposal review. The authoritative checklist is in
[the UI/UX roadmap](../ui-ux-roadmap.md#deferred-uiux-todo).

## Evidence limitations

- The operator reproduction is recorded as an operator attestation in this
  repository; no separate externally authored evidence bundle was checked in.
- The current UI remains difficult to learn and does not satisfy the later M3
  production-polish, accessibility, and support-matrix goals.
- Figma composition is incomplete because the Starter plan exhausted its MCP
  tool-call quota; the file, foundations, and continuation tasks are preserved.
