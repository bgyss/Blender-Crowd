# M7 — Blender ecosystem and mainline readiness

## Objective

Turn extension experience into a maintainable Blender ecosystem integration
strategy and, where evidence and maintainer agreement support it, narrowly
scoped proposals for generally useful Blender mainline capabilities.

## Sources of truth

- [Long-term Blender integration destination](../blender-crowd-1.0.md#19-long-term-industrial-and-blender-integration-destination)
- [Blender ecosystem and mainline strategy](../industrial-crowd-capability-roadmap.md#blender-ecosystem-and-mainline-strategy)
- [M3 production 1.0](M3-production-1.0.md)
- [UI/UX roadmap](../ui-ux-roadmap.md)

## Prerequisites and authorization

M3 passes and real users or production evaluators have exercised the extension.
M4-M6 evidence is included only for completed capabilities. Researching public
Blender processes and preparing local proposals is in scope; contacting
maintainers, publishing proposals, filing issues, opening pull requests, or
representing the project externally requires explicit authorization in that
task. Blender maintainer review and acceptance are external gates.

Before M7 acceptance, at least two evaluators who did not implement the feature
must independently install a release archive, create or open a project,
validate, bake, reload, recover one injected failure, and render the acceptance
shot using the documented interface. Record task success, time, errors,
recovery, accessibility findings, and remaining support limitations with the
[evaluator study template](../release/1.0-evaluator-study-template.md).

## In scope

1. Extension adoption and maintenance evidence: supported versions, install and
   crash data, real shot profiles, upgrade/migration history, contributor load,
   user pain points, and long-term ownership.
2. Audit of host boundaries: RNA/dependency graph, undo, save/reload, extension
   packaging, native modules, Geometry Nodes attributes, cache streaming,
   viewport drawing, render extraction, armatures, and USD.
3. Separation of Blender-general gaps from crowd-product policy. Potentially
   general capabilities include efficient point-instancer animation exchange,
   versioned cache hooks, viewport/proxy APIs, or reusable schema primitives;
   evidence decides whether each belongs upstream.
4. Compatibility and security review for public Rust/Python/cache/IR/GN APIs,
   any C ABI/C++ wrapper, native binary loading, untrusted files, paths, resource
   limits, and crash isolation.
5. Blender-style design documents with motivation, user stories, alternatives,
   API/data ownership, performance, migration, tests, maintenance, and staged
   rollout for each proposed general capability.
6. Small independently reviewable prototype patches only after proposal scope
   and authorization are clear.
7. An extension fallback and support plan for every feature not accepted into
   Blender or not appropriate for mainline.

## UI/UX goals and gate

- Audit the extension against Blender conventions for workspaces, editors,
  Properties and sidebar panels, operators, selection, undo, keymaps, themes,
  translations, help links, preferences, asset browsing, and save/load.
- Separate product-specific Crowd UI from Blender-general host improvements;
  never propose the entire product interface as a mainline feature.
- Preserve familiar interaction and terminology on every supported Blender
  version, with explicit fallbacks when a host API or presentation differs.
- Ensure extension installation, native-module diagnostics, upgrades,
  permissions, missing dependencies, and support collection are readable and
  privacy-preserving.
- Carry the M3 accessibility criteria through stock themes, display scaling,
  keyboard customization, and supported operating systems.

The M7 UI gate passes when the complete supported workflow is evaluated on stock
Blender builds, with no private fork assumptions. The host-version matrix must
record visual or interaction differences, fallbacks, task success, accessibility
results, and which gaps remain extension policy versus evidence-backed Blender
platform candidates.

## Explicit exclusions

- Assuming Blender Foundation endorsement, acquisition, bundling, or acceptance.
- Proposing the entire CrowdBrain/product UI as one mainline patch.
- Maintaining a permanent private Blender fork as the default product path.
- Using upstream discussion as a substitute for extension quality or support.

## Required artifacts

- Dated adoption/maintenance report with privacy-preserving evidence and limits.
- Host-boundary and upstream-candidate matrix with keep-in-extension rationale.
- Local Blender design proposal packet and prototype benchmarks/tests for each
  candidate; external links only after authorized publication.
- Compatibility, security, licensing, governance, maintainer, and fallback plan.

## Acceptance criteria

1. Production evidence identifies at least one Blender-general host limitation;
   the proposed solution is useful beyond Blender Crowd and smaller than the
   product itself.
2. Each candidate has reproducible performance/correctness evidence, ownership
   boundaries, alternatives, compatibility impact, tests, and a named long-term
   maintenance plan.
3. The extension runs on unmodified supported Blender releases before and after
   any prototype, and fallback behavior is tested.
4. Security review covers malformed schemas/caches, resource exhaustion, native
   module loading, path handling, unsafe external assets, and diagnostic privacy.
5. Licensing/provenance permits the proposed code and fixtures to enter Blender's
   contribution process.
6. If authorized and submitted, proposal/patch status is reported exactly as
   draft, discussed, under review, accepted, declined, or superseded; “ready”
   never means “accepted.”

## Validation and proof

Run the full extension release suite on stock Blender, candidate-specific unit/
integration/performance/security tests, compatibility comparisons, and fallback
tests. Any host patch must follow Blender's actual contribution and test process
current at submission time, verified from primary Blender documentation.

## Definition of done and stop conditions

M7 readiness is done when the local evidence/proposal/fallback packet satisfies
all criteria. Actual mainline integration is done only for patches accepted by
Blender maintainers and shipped through their process. Stop external action
without authorization, on missing maintenance ownership, or when evidence shows
the capability is crowd-specific and belongs in the extension.
