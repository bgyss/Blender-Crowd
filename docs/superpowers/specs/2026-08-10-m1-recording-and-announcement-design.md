# M1 concourse recording and project announcement

Date: 2026-08-10

Scope: one new visualisation of the accepted M1 vertical slice, and the
project's first public announcement posts.

## Problem

Both clips checked into `docs/media/` are the `crossing` scene, which jams: the
two streams intersect, lanes collapse, and only 24% of agents reach their
destination. That was an honest picture of the M0 solver, and it stays.

It is no longer the whole picture. The accepted
[M1 vertical slice](../../benchmarks/2026-08-10-m1-vertical-slice.md) added a
reference concourse that reaches 96% completion with zero boundary escapes,
survives a timed portal closure and reopen, and — the part no existing clip
shows — plays back and renders from a *completed cache in a process with no
live simulation session*. Nothing in `docs/media/` shows the concourse at all.

Separately, the project has never been announced. There is no prior audience
context to build on, so the first posts must introduce the project and land M1
at the same time.

## Non-goals

- Retiring the `crossing` clips. They document the open flow-quality problem,
  which M1 did not close.
- Any new simulation, solver, or presentation behaviour. The recorder composes
  shipped operators only.
- Any performance claim derived from the recording. See "Not a measurement".

## Component 1: `scripts/record_m1_playback.py`

A Blender-side recorder, structured like `tests/blender/test_m1_render.py`.

1. Enable `bl_ext.user_default.blender_crowd`.
2. `bpy.ops.crowd.attach_cache(filepath=$CROWD_M1_CACHE_PATH)`.
3. Assert the recording's own claims, failing non-zero rather than producing a
   clip that does not prove them:
   - `active_cache_playback()` has no `session` attribute;
   - `playback.agent_count == 1000`.
4. Call `render_workflow.configure_reference_scene(scene)` for the camera,
   lights, ground, and world, then override its hardcoded 320x180 with
   `CROWD_RES_X` / `CROWD_RES_Y` (default 960x540). Resolution is the only
   deviation from the accepted reference framing.
5. For `tick` in `tick_start .. tick_end` stepping by `CROWD_TICK_STEP`:
   `playback.sync_to_tick(tick)`, then render Eevee to `frame-%05d.png`.
6. Write `m1-recording.json` beside the frames: cache manifest hash, agent
   count, tick range, step, frame count, resolution, Blender version,
   `"cache_only": true`, and `"measurement": false`.

The sidecar exists so the clip is traceable to one specific cache rather than
being an undated video of someone's desktop.

## Component 2: `scripts/make-m1-recording.sh`

Mirrors `scripts/make-blender-recording.sh`.

1. Bake a strict cache with `crowd-bench m1 bake --cache "$CACHE"`, unless
   `CROWD_M1_CACHE_PATH` names an existing one. The cache is roughly 560 MB, so
   it is written to a temp directory and never to the repository.
2. Run the recorder through `scripts/blender-install-test.sh --python`, so the
   clip comes from a clean-installed extension, exactly as `m1-render-test.sh`
   does.
3. Encode with ffmpeg: H.264 `.mp4`, and a two-pass palette `.gif` with
   `dither=none`, for the reasons `make-gif.sh` already documents — a
   single-pass palette would be picked from a nearly empty opening frame, and
   dithering scatters noise across a flat background that LZW cannot compress.
4. Write `docs/media/m1-concourse-1000.{mp4,gif}`.

Defaults: `CROWD_TICK_STEP=20`, 30 fps, GIF at 10 fps and 440 px wide. Over
10,000 ticks that is 500 frames, a 16.7-second clip running at roughly 20x
simulation time.

## Not a measurement

Frames are rendered one at a time with a cache sync between them. Neither the
clip's length nor its frame rate says anything about playback or simulation
speed, and the clip's own sidecar records `"measurement": false`. The measured
costs stay where they are: separated simulation, cache write, cache read, point
upload, armature evaluation, Eevee, and Cycles CPU figures in the M1 acceptance
report.

## Component 3: README

A new hero clip above the existing crossing clips, stating what it shows — 1,000
agents, 96% completion, the `east_gate` closure at tick 600 and reopen at tick
900, rendered from a completed cache with no simulation process alive — and
carrying the same visualisation-not-measurement disclaimer as its neighbours.
The crossing clips and their open-problem framing are retained verbatim.

## Component 4: `docs/announcements/2026-08-10-m1-launch.md`

Three drafts — LinkedIn, X/Twitter, Reddit — checked in so they are reviewable
and revisable rather than pasted once into a chat.

Framing decisions:

- **Evidence-first.** Lead with what is proven and measured, not with vision.
- **Single combined announcement per platform.** There is no prior audience, so
  each post introduces the project and lands M1 together.
- **Every post carries limits.** The acceptance report's "known limitations and
  unsupported claims" list is not an appendix to be dropped for reach: one
  machine, one Blender version, procedural proxies rather than production rigs,
  one fixed behaviour program, no 10,000-agent claim, and 40 agents that never
  arrive.

## Verification

- `scripts/make-m1-recording.sh` exits zero and writes both media files.
- The recorder's assertions cover the cache-only and 1,000-agent claims; it
  exits non-zero if either fails.
- `m1-recording.json` reports 1,000 agents, ticks 0-9,999, and the expected
  frame count.
- `git diff --check` is clean.
