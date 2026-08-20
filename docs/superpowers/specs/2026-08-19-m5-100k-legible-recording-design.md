# Legible 100K recording: cropped, tracked, 4K

Date: 2026-08-19
Status: implemented
Touches: `scripts/render_playback.py`, `scripts/make-m5-100k-recording.sh`

## Problem

`scripts/make-m5-100k-recording.sh` renders the whole `m5_city_flow` scene at
100,000 agents into a 1280x720 frame. The scene is 2401.6 x 1140.5 m, so the
camera's ortho window is 2762 m across 1280 px: 2.16 m per pixel. An agent is
instanced as a cone of radius 0.25 (0.5 m across), which lands at 0.23 px. The
clip is therefore a pale wash rather than a picture of a crowd.

This is separate from the grey-frame defect fixed alongside it, where the
camera's default `clip_end` of 1000 sat inside the ~3046 m camera-to-centre
distance and clipped the entire scene away. That fix is a prerequisite: without
it nothing renders at all.

Raising resolution alone does not solve it. At 3840 px the same window gives
0.70 px per agent, still under a pixel. Legibility has to come from the crop.

## Scene facts the design depends on

Measured from `m5_city_flow-100000.crowdtrace` (875 ticks, 100,000 agents,
`--trace-interval 48`, `--max-ticks 42000`):

- Two one-way lane blocks, stacked in y and never mixing: south band
  y in [62.7, 570.2] holding 50,030 agents, north band y in [694.4, 1202.3]
  holding 49,970. The 124 m gap between them is empty ground.
- The blocks travel in opposite x directions and pass each other from about
  tick 470 onward.
- All 100,000 agents are live at every tick in the trace, positions finite.

## Decisions

### Framing: a 250 m window, band-centred

A 250 m-wide orthographic window centred at y = 316, inside the south band.

At 3840 px that is 15.36 px/m: an agent cone is 0.5 m across, so 7.7 px, and
1.7 m tall, so about 26 px before the camera tilt foreshortens it. Agents read
as individual shapes with visible facing, and the fixture's 2.66 m lane pitch
is resolvable.

The window's visible ground depth is roughly 220 m rather than the 140 m a flat
16:9 crop implies, because the camera is tilted. `CAMERA_TILT_RATIO` of 1.23
puts the view direction 39.1 degrees above the horizon, so ground depth is
frame height divided by sin(39.1) = 0.63.

Rejected: a window straddling the y gap to show both directions at street
level. Measured, it holds 1,900-4,600 agents against the band-centred window's
8,300-12,300, and both directions are simultaneously in frame for only about
5 s of the 29 s clip, because the north block crosses the tracked x briefly and
leaves.

### Camera: constant-velocity pan

The camera tracks the south block along x only. y stays fixed, because the
block never leaves its band, and holding y still removes a whole axis of
potential wobble.

The pan is a least-squares straight-line fit of the block's median x against
tick, over all 875 ticks:

    x(tick) = 1.924605 * tick + 279.4882

Travel is 279.5 m to 1961.6 m. Residual against the true median is 10.55 m
worst case and 4.56 m rms, i.e. the crowd stays within 4.2% of frame width of
centre for the whole clip. This is why no smoothing filter is needed: the fit
is the smoothing, and a straight line cannot jitter.

Median rather than mean: the block disperses as it travels and a mean is pulled
by stragglers.

### Ground: a 50 m grid, as geometry

A tracking camera over a featureless plane looks like a static camera over
milling agents. The grid supplies the motion cue and doubles as a scale
reference.

It must be geometry, not a texture: the renderer is Workbench with
`configure_render` setting `use_nodes = False` and colour coming from
`diffuse_color`, so a node-based texture would not appear. Build the grid as
thin quads at 50 m spacing, ~0.25 m wide (about 3.8 px at 15.36 px/m), sitting
just above the ground plane, carrying their own darker material.

Grid extent covers the occupied bounds `scan_trace` returns, not just the
traversed corridor. It is a
few hundred quads either way and generating the whole thing avoids a second
piece of positioning logic that could disagree with the camera's.

### Scope: crop mode is opt-in

New environment variables on `render_playback.py`:

- `CROWD_CROP_WIDTH` - ortho window width in metres. Unset means no crop.
- `CROWD_TRACK_STREAM` - which stream label to track (0 = south, 1 = north).
- `CROWD_GROUND_GRID` - grid spacing in metres. Unset means no grid.

With none of them set, behaviour is pixel-identical to today (Blender writes
`Date` and `RenderTime` into each PNG's `tEXt` chunks, so renders are never
byte-identical). This keeps
`make-blender-recording.sh`, `make-m1-recording.sh`, and the crossing
recordings untouched.

`build_camera` gains a view-width parameter distinct from scene extent. In crop
mode `ortho_scale` is exactly `CROWD_CROP_WIDTH`, with no `FRAMING_MARGIN`
applied, so "250 m window" means 250 m. Camera height and standoff derive from
the view width rather than the scene extent, and the clip planes keep deriving
from the resulting camera-to-target distance.

`scan_trace` already walks every tick for bounds and stream labels; it gains
the per-tick median x of the tracked stream in the same pass, so the fit costs
no extra I/O.

`make-m5-100k-recording.sh` sets the new variables and `RES_X=3840`.

## Costs

- Render stage: 4K is ~9x the pixels of 1280x720. Expect roughly 15-20 min
  against the current 137 s.
- Output: 161 MB (156,193,321 bytes) against the current 7.1 MB.
- The trace is reused; the multi-hour bake does not re-run.

## Claims this clip does and does not support

The clip shows 8,300-12,300 agents of 100,000, under 1% of the scene area. It
must not be captioned as an image of the population; that claim belongs to
`docs/media/m5-100k-hero.png`, which is a measured positional plot asserted at
above 95% occupancy. The script header should say so, matching the existing
qualifiers in `make-m5-100k-recording.sh` about the run being truncated and the
frame rate carrying no performance meaning.

The clip is a visualisation, not a measurement. Nothing about it is gate
evidence.

## Verification

- One frame in crop mode, checked by eye for agent legibility and grid.
- `crossing` at 1,000 agents rendered with no crop variables set, confirming
  the default path is unchanged.
- Full 875-frame run, then a frame extracted back out of the encoded mp4.
- `git diff --check`.
