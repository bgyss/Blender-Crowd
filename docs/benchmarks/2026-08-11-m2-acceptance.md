# M2 acceptance status — 2026-08-11

## Decision

M2 is **not accepted yet**. The complete engineering subgate now passes, but
the contract still requires an independent, non-developer Blender crowd-TD
reproduction. This report records the boundary rather than converting a
developer-run automation result into independent evidence.

## Fresh technical evidence

`scripts/m2-full-acceptance.sh --out <host-evidence-directory>` ran on Blender
5.2 LTS with host Metal access. The clean-install runner:

- baked the authored 1,000-agent reference through ticks 0–9,999 in 54.319 s;
- wrote a complete cache and a transition-compacted behavior sidecar with
  1,724,156 decision/queue/group records;
- attached and replayed only the completed cache;
- inspected a selected cached agent and exposed graph/node evidence to the
  debug overlay;
- authored a cache-only hero pin and verified that every base-cache artifact
  remained byte-identical;
- produced both Eevee and Cycles cache-only renders.

The machine-readable result is
[2026-08-11-m2-full-acceptance.json](2026-08-11-m2-full-acceptance.json).
The source-controlled runner is `scripts/m2-full-acceptance.sh`.

## Exit-gate ledger

| Gate | Evidence | Status |
| --- | --- | --- |
| Full authorable Blender bake | 1K / 10K-tick complete cache, authorable sidecar | Passed |
| Cache-only playback and render | Attached cache plus Eevee and Cycles PNGs | Passed |
| Semantic/debug evidence | Durable decision, queue, group events and selected-agent overlay | Passed |
| Authorable social constraints | Saved Blender group contract with stable IDs and leader-first policy | Passed |
| Sparse correction isolation | Hero pin plus unchanged base-cache artifact hashes | Passed |
| Independent crowd-TD reproduction | [handoff procedure](../user/m2-reference-reproduction.md) | Open |

## Remaining acceptance action

Have a person who did not implement M2 run the handoff procedure on their
host, preserve their output directory, and append their dated environment and
result to this report. Only then may `m2_milestone_accepted` switch to `true`.
