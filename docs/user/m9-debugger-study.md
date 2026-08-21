# M9 debugger and motion-authoring study protocol

This protocol is the human-evidence lane for M9. It must be run with a clean
artifact by a participant who did not implement the tested M6 debugger,
motion-layer features, M9 backend, or neural-control UI. Automated tests and
developer walkthroughs do not count as this gate.

## Purpose

Measure whether an independent artist or technical director can author, inspect,
and repair representative agency and motion failures using the Blender
interface and node reference, while understanding what lower-fidelity tiers do
not expose.

## Prerequisites

- A clean Blender file containing the checked M6 reference project and no
  developer-specific setup instructions.
- The M6 extension installed through the supported package path.
- The checked interaction, formation, terrain, recovery, and mixed-tier
  fixtures available as project-local reference inputs.
- For the conditional neural-control task, a backend that passed its Track A
  technical gate plus frozen worker/model/checkpoint/configuration/data-license
  hashes and a deterministic fallback fixture. Omit that task until then.
- Screen capture or timestamped observer notes, with participant consent and
  no private source assets copied into the repository.

## Tasks

Run each task from a fresh saved copy. The observer may clarify the task wording
but must not identify the expected node, layer, or correction.

1. **State-machine repair:** find the agent whose interrupt did not fire,
   navigate from its trace to the graph node, identify the missing typed
   observation, repair it, and revalidate.
2. **Utility repair:** find a utility decision that chose the wrong action,
   compare the displayed scores and blackboard values, adjust the authored
   scorer, and verify the next decision.
3. **Behavior-tree repair:** locate a sequence/fallback failure, follow the
   visited-node path, correct the failing action or fallback, and rerun the
   bounded shot.
4. **Motion/contact repair:** locate a rejected or fallback interaction layer,
   inspect root deviation, contact ownership, foot/terrain diagnostics, and
   solver ownership, then correct or mute the sparse layer without rebaking
   unrelated agents.
5. **Degraded evidence:** inspect the same event at a distant/background tier
   and state which observations, scores, contacts, and group diagnostics are
   unavailable. The participant must not infer unavailable evidence.
6. **Large-graph navigation:** search for a named action and navigate through
   its highlighted parent/child path to the decisive node.
7. **Neural proposal control, when claimed:** inspect model/checkpoint/data and
   constraint provenance, compare the proposal with its deterministic baseline,
   reject one invalid variation, correct a valid-but-misdirected sparse layer
   and accept it only after correction, force the fallback path, disable the
   worker/model, reload the accepted cache, and render from cache without
   changing the base cache or unrelated agents.

## Record for every task

| Field | Required value |
|---|---|
| Participant ID | Pseudonymous study ID only |
| Task start/end | Timestamps in one declared timezone |
| First successful discovery | Timestamp and path taken |
| Repair result | Pass, fallback, rejection, or unresolved |
| Wrong turns | Count and short description |
| Developer intervention | None, clarification, or intervention with reason |
| Evidence state understood | Full, reduced, or misunderstood |
| Screenshot/artifact ID | Relative path or redacted capture ID |
| UI states captured | Applicable normal, empty, loading, success, warning, error, stale, canceled, and recovered states |
| Keyboard/focus review | Result and any blocked task |
| Visual accessibility review | Labels, target size, truncation, contrast, theme, and scaling findings |
| Exposed advanced controls | Control, why it remains exposed, and whether the participant used it |
| Unresolved usability risks | Repository issue/follow-up ID and severity |

## Gate criteria

The study report must show, per task:

- trace-to-node agreement;
- discovery time and repair time;
- whether the participant preserved stable IDs, unrelated agents, and the base
  cache;
- whether rejected motion was corrected, muted, or deliberately kept as a
  deterministic fallback;
- whether reduced-evidence states were understood accurately;
- whether large-graph search reached the intended action without copied IDs;
- for a claimed neural backend, whether provenance, rejection, correction,
  fallback, model-absent replay, and render-from-cache were completed; and
- the applicable UI-state captures, keyboard/focus and visual-accessibility
  review, exposed-control accounting, and follow-ups for unresolved risks.

The M9 operator gate remains open until an independent participant completes the
representative state-machine, utility, behavior-tree, motion/contact, and
interaction tasks with the agreed study report. When neural controls are
claimed, the conditional neural-control task is also mandatory. A successful
automated smoke is evidence that the interface loads, not evidence of
independent authorability.

## Stop conditions

Stop the study and record the failure if the participant must edit raw JSON or
source code, if a worker/model is required to replay an accepted result, if a
correction mutates unrelated agents or the base cache, or if the UI presents a
diagnostic without naming its owning layer and recovery action.
