# Legible 100K Recording Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make `scripts/make-m5-100k-recording.sh` produce a legible 4K clip by cropping to a 250 m window that tracks one lane block, instead of framing the whole 2401.6 x 1140.5 m scene at 1280x720.

**Architecture:** All framing maths moves into a new dependency-free module `scripts/crowd_framing.py`, so it can be unit-tested in plain CPython without launching Blender. `scripts/render_playback.py` keeps all `bpy` contact and calls into that module. Crop mode is opt-in by environment variable, so every existing recording keeps its current framing.

**Tech Stack:** Python 3 (stdlib only in `crowd_framing.py`), `unittest`, Blender 5.2 LTS with Workbench, ffmpeg.

## Global Constraints

- `scripts/crowd_framing.py` must import **only** the standard library. No `bpy`, no `numpy`. This is what lets `tests/test_crowd_framing.py` run outside Blender.
- Four spaces for Python indentation, `snake_case` for functions and modules.
- With `CROWD_CROP_WIDTH`, `CROWD_TRACK_STREAM`, and `CROWD_GROUND_GRID` all unset, `render_playback.py` behaviour must be unchanged from today.
- The renderer is Workbench with `use_nodes = False` and colour from `diffuse_color`. Node-based textures do not render. Any visible surface detail must be geometry.
- Blender path: `/Applications/Blender.app/Contents/MacOS/Blender`, overridable by `BLENDER`. Blender runners need real host Metal access; a restricted sandbox crashes Blender before Python starts.
- The clip shows 8,300-12,300 agents of 100,000, under 1% of scene area. It must never be captioned as an image of the population.
- Measured scene facts, for asserting against: scene 2401.6 x 1140.5 m; south band y in [62.7, 570.2] with 50,030 agents; north band y in [694.4, 1202.3] with 49,970; stream split y = 632.4555320336759.
- Tracking fit for the existing trace: `x(tick) = 1.924605 * tick + 279.4882`, worst residual 10.55 m, rms 4.56 m.

---

## Corrections after execution

This plan is kept as the record of what was executed, so its task text is left
as it was written. Two numbers in it are wrong and were corrected in the code
and the design spec by commit `e3b38ca`. Do not quote them from here:

- **The whole-scene camera distance is ~3046 units, not ~2076.** The 2076
  figure came from a single-tick debugging probe where the occupied extent was
  1637 m; the renderer uses the full-scan extent of 2401.6 m, which gives
  `hypot(2401.6*0.8*1.23, 2401.6*0.8) = 3045.6`. It appears wrong in the Global
  Constraints, in Task 1's test code and module docstring, and in Task 1's
  commit message. The conclusion is unaffected -- Blender's default `clip_end`
  of 1000 clips the scene at either distance.
- **The clip shows 8.3%-12.3% of the population and ~2% of the scene's ground
  area, not "under 1%".** The "under 1%" phrasing dates from an early estimate
  that the 250 m window would hold only a few hundred agents; the agent count
  was corrected to 8,300-12,300 but the percentage was not. It appears wrong in
  the Global Constraints and in Task 6's script text and commit message.

Both were found by the whole-branch review, not by any single task's review --
each task's text was internally consistent, which is exactly why a
task-scoped gate could not catch either one.

---

### Task 1: Extract clip-plane derivation and cover it with a test

The grey-frame defect is already fixed inline in `build_camera`, but untested. Move it into the new module so it has a test, and establish the module + test file the later tasks build on.

**Files:**
- Create: `scripts/crowd_framing.py`
- Create: `tests/test_crowd_framing.py`
- Modify: `scripts/render_playback.py` (imports near line 25-33; `build_camera` near line 208)

**Interfaces:**
- Consumes: nothing.
- Produces: `crowd_framing.clip_planes(distance) -> (near, far)` as floats.

- [ ] **Step 1: Write the failing test**

Create `tests/test_crowd_framing.py`:

```python
"""Framing maths for the recording renderer, tested without launching Blender."""

import importlib.util
import unittest
from pathlib import Path


MODULE = Path(__file__).parents[1] / "scripts" / "crowd_framing.py"
SPEC = importlib.util.spec_from_file_location("crowd_framing", MODULE)
crowd_framing = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(crowd_framing)


class ClipPlaneTest(unittest.TestCase):
    def test_far_plane_clears_the_distance_it_was_derived_from(self):
        # The 100K scene stands the camera ~2076 units back. Blender's default
        # far plane of 1000 sits inside that and clips the whole scene away.
        near, far = crowd_framing.clip_planes(2076.0052083268)
        self.assertGreater(far, 2076.0052083268)
        self.assertLess(near, 2076.0052083268)

    def test_near_plane_never_collapses_to_zero_on_a_close_camera(self):
        near, _far = crowd_framing.clip_planes(0.5)
        self.assertEqual(near, 0.1)

    def test_planes_scale_with_distance(self):
        near, far = crowd_framing.clip_planes(1000.0)
        self.assertEqual(near, 10.0)
        self.assertEqual(far, 4000.0)


if __name__ == "__main__":
    unittest.main()
```

- [ ] **Step 2: Run test to verify it fails**

Run: `python3 -m unittest -q tests/test_crowd_framing.py`
Expected: FAIL — `FileNotFoundError` for `scripts/crowd_framing.py`, because the module does not exist yet.

- [ ] **Step 3: Write minimal implementation**

Create `scripts/crowd_framing.py`:

```python
"""Pure framing maths for the recording renderer.

Imported by `scripts/render_playback.py`, which runs inside Blender, and by
`tests/test_crowd_framing.py`, which runs in plain CPython. Nothing here may
import `bpy` or `numpy`: staying dependency-free is what lets the framing be
tested without launching Blender.
"""

import math


def clip_planes(distance):
    """Near and far clip planes for a camera `distance` from its target.

    A new Blender camera clips at 1000, but this renderer stands the camera
    back proportionally to the scene, and scene extent grows with the square
    root of the population. m5_city_flow at 100,000 agents puts the camera
    ~2076 units out, so at the default every object -- agents and ground
    alike -- falls beyond the far plane and the render is nothing but world
    background. Deriving both planes from the actual camera-to-target
    distance means framing and clipping can never disagree.
    """
    return max(0.1, distance * 0.01), distance * 4.0
```

- [ ] **Step 4: Run test to verify it passes**

Run: `python3 -m unittest -q tests/test_crowd_framing.py`
Expected: PASS, 3 tests.

- [ ] **Step 5: Wire `render_playback.py` to the module**

In `scripts/render_playback.py`, the import block currently reads:

```python
import os
import sys
import time

import numpy as np

import addon_utils
import bpy
import mathutils
```

Replace it with:

```python
import os
import sys
import time

import numpy as np

import addon_utils
import bpy
import mathutils

# Blender runs this file by path, so the directory holding it is not on
# sys.path the way it would be for an imported module.
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

import crowd_framing
```

Then in `build_camera`, replace the inline clip derivation:

```python
    distance = (camera.location - target).length
    camera_data.clip_start = max(0.1, distance * 0.01)
    camera_data.clip_end = distance * 4.0
```

with:

```python
    distance = (camera.location - target).length
    camera_data.clip_start, camera_data.clip_end = crowd_framing.clip_planes(distance)
```

Leave the explanatory comment above it in place.

- [ ] **Step 6: Verify Blender still renders a frame**

Run:

```bash
cargo run --release -p crowd-bench -- run --scene crossing --agents 1000 --trace --out /tmp/crowd-plan-check
CROWD_TRACE_PATH=/tmp/crowd-plan-check/crossing-1000.crowdtrace \
CROWD_FRAME_DIR=/tmp/crowd-plan-check/frames \
CROWD_TICK_STEP=2000 CROWD_RES_X=640 \
/Applications/Blender.app/Contents/MacOS/Blender -b --factory-startup \
  --python scripts/render_playback.py
```

Expected: ends with `PASS: rendered 3 frames to /tmp/crowd-plan-check/frames`. Open one PNG and confirm agents are visible against the ground, not a flat background.

- [ ] **Step 7: Commit**

```bash
git add scripts/crowd_framing.py tests/test_crowd_framing.py scripts/render_playback.py
git commit -m "Derive camera clip planes from camera distance, with a test

A new Blender camera clips at 1000. The recording renderer stands the
camera back proportionally to scene extent, which grows with the square
root of the population, so m5_city_flow at 100,000 agents put the camera
2076 units out and every object fell beyond the far plane. Every frame
of the 100K recording was world background and nothing else.

The derivation moves into a new dependency-free scripts/crowd_framing.py
so it can be tested in plain CPython instead of only inside Blender."
```

---

### Task 2: Fit a straight-line camera track

**Files:**
- Modify: `scripts/crowd_framing.py`
- Modify: `tests/test_crowd_framing.py`

**Interfaces:**
- Consumes: `crowd_framing` module from Task 1.
- Produces:
  - `crowd_framing.fit_linear_track(samples) -> (slope, intercept)` where `samples` is a list of `(index, value)` float pairs.
  - `crowd_framing.track_residuals(samples, fit) -> (worst_abs, rms)`.

- [ ] **Step 1: Write the failing test**

Append to `tests/test_crowd_framing.py`, above the `if __name__` block:

```python
class LinearTrackTest(unittest.TestCase):
    def test_fits_an_exact_line_exactly(self):
        samples = [(i, 2.0 * i + 10.0) for i in range(20)]
        slope, intercept = crowd_framing.fit_linear_track(samples)
        self.assertAlmostEqual(slope, 2.0)
        self.assertAlmostEqual(intercept, 10.0)

    def test_tolerates_gaps_in_the_sampled_ticks(self):
        # Ticks with no tracked agent are dropped rather than interpolated,
        # so the fit has to take explicit indices instead of assuming
        # consecutive ones.
        samples = [(0, 10.0), (5, 20.0), (17, 44.0)]
        slope, intercept = crowd_framing.fit_linear_track(samples)
        self.assertAlmostEqual(slope, 2.0)
        self.assertAlmostEqual(intercept, 10.0)

    def test_single_sample_yields_a_stationary_track(self):
        slope, intercept = crowd_framing.fit_linear_track([(7, 42.0)])
        self.assertEqual(slope, 0.0)
        self.assertEqual(intercept, 42.0)

    def test_no_samples_is_an_error_not_a_silent_zero(self):
        with self.assertRaisesRegex(ValueError, "no samples"):
            crowd_framing.fit_linear_track([])

    def test_residuals_of_an_exact_line_are_zero(self):
        samples = [(i, 3.0 * i - 4.0) for i in range(10)]
        worst, rms = crowd_framing.track_residuals(
            samples, crowd_framing.fit_linear_track(samples)
        )
        self.assertAlmostEqual(worst, 0.0)
        self.assertAlmostEqual(rms, 0.0)

    def test_residuals_report_the_worst_deviation(self):
        samples = [(0, 0.0), (1, 1.0), (2, 2.0), (3, 3.0)]
        worst, rms = crowd_framing.track_residuals(samples, (1.0, 0.5))
        self.assertAlmostEqual(worst, 0.5)
        self.assertAlmostEqual(rms, 0.5)
```

- [ ] **Step 2: Run test to verify it fails**

Run: `python3 -m unittest -q tests/test_crowd_framing.py`
Expected: FAIL with `AttributeError: module 'crowd_framing' has no attribute 'fit_linear_track'`.

- [ ] **Step 3: Write minimal implementation**

Append to `scripts/crowd_framing.py`:

```python
def fit_linear_track(samples):
    """Least-squares line through `(index, value)` pairs.

    Returns `(slope, intercept)` such that `slope * index + intercept`
    approximates the value at that index.

    The tracked lane block's median x is near-perfectly linear in tick -- on
    the 100K trace the worst deviation is 10.55 m, 4.2% of a 250 m window --
    so this fit is the camera's entire smoothing strategy. A straight line
    cannot jitter, which is why no filter is applied on top of it.
    """
    count = len(samples)
    if count == 0:
        raise ValueError("cannot fit a track through no samples")
    if count == 1:
        return 0.0, float(samples[0][1])

    mean_index = sum(index for index, _value in samples) / count
    mean_value = sum(value for _index, value in samples) / count
    numerator = 0.0
    denominator = 0.0
    for index, value in samples:
        offset = index - mean_index
        numerator += offset * (value - mean_value)
        denominator += offset * offset
    if denominator == 0.0:
        # Every sample sits at the same index; nothing determines a slope.
        return 0.0, mean_value
    slope = numerator / denominator
    return slope, mean_value - slope * mean_index


def track_residuals(samples, fit):
    """Worst absolute and rms deviation of `samples` from `fit`.

    Reported so a run says how far the crowd wanders from frame centre
    rather than leaving it to be judged by eye.
    """
    slope, intercept = fit
    deviations = [value - (slope * index + intercept) for index, value in samples]
    worst = max(abs(deviation) for deviation in deviations)
    rms = math.sqrt(sum(d * d for d in deviations) / len(deviations))
    return worst, rms
```

- [ ] **Step 4: Run test to verify it passes**

Run: `python3 -m unittest -q tests/test_crowd_framing.py`
Expected: PASS, 9 tests.

- [ ] **Step 5: Commit**

```bash
git add scripts/crowd_framing.py tests/test_crowd_framing.py
git commit -m "Fit the recording camera's pan to a straight line

The tracked lane block's median x is near-perfectly linear in tick, so a
least-squares line is both the track and the smoothing: worst deviation
on the 100K trace is 10.55 m, 4.2% of a 250 m window. A straight line
cannot jitter, so no filter is needed on top.

Samples carry explicit indices so ticks holding no tracked agent can be
dropped rather than interpolated."
```

---

### Task 3: Generate the ground grid as geometry

**Files:**
- Modify: `scripts/crowd_framing.py`
- Modify: `tests/test_crowd_framing.py`

**Interfaces:**
- Consumes: `crowd_framing` module from Task 2.
- Produces:
  - `crowd_framing.grid_line_positions(low, high, spacing) -> [float]`
  - `crowd_framing.grid_mesh(minimum, maximum, spacing, line_width, z) -> (vertices, faces)` where `minimum`/`maximum` are `(x, y)` pairs, `vertices` is a list of `(x, y, z)` triples and `faces` a list of 4-tuples of vertex indices, ready for `bpy.types.Mesh.from_pydata`.

- [ ] **Step 1: Write the failing test**

Append to `tests/test_crowd_framing.py`, above the `if __name__` block:

```python
class GroundGridTest(unittest.TestCase):
    def test_lines_land_on_multiples_of_the_spacing(self):
        self.assertEqual(crowd_framing.grid_line_positions(0.0, 100.0, 50.0),
                         [0.0, 50.0, 100.0])

    def test_lines_outside_the_range_are_excluded(self):
        self.assertEqual(crowd_framing.grid_line_positions(10.0, 90.0, 50.0), [50.0])

    def test_spacing_must_be_positive(self):
        with self.assertRaisesRegex(ValueError, "positive"):
            crowd_framing.grid_line_positions(0.0, 100.0, 0.0)

    def test_mesh_has_one_quad_per_line_in_both_axes(self):
        vertices, faces = crowd_framing.grid_mesh(
            (0.0, 0.0), (100.0, 100.0), 50.0, 1.0, -0.83
        )
        # 3 lines along x plus 3 along y.
        self.assertEqual(len(faces), 6)
        self.assertEqual(len(vertices), 24)
        self.assertTrue(all(len(face) == 4 for face in faces))

    def test_every_vertex_sits_at_the_requested_height(self):
        vertices, _faces = crowd_framing.grid_mesh(
            (0.0, 0.0), (100.0, 100.0), 50.0, 1.0, -0.83
        )
        self.assertTrue(all(vertex[2] == -0.83 for vertex in vertices))

    def test_a_line_spans_the_full_perpendicular_extent(self):
        vertices, faces = crowd_framing.grid_mesh(
            (0.0, 0.0), (100.0, 100.0), 50.0, 1.0, 0.0
        )
        first = [vertices[index] for index in faces[0]]
        xs = sorted({round(vertex[0], 6) for vertex in first})
        ys = sorted({round(vertex[1], 6) for vertex in first})
        # A bar 1.0 wide standing on x = 0, running the whole y range.
        self.assertEqual(xs, [-0.5, 0.5])
        self.assertEqual(ys, [0.0, 100.0])

    def test_face_indices_stay_inside_the_vertex_list(self):
        vertices, faces = crowd_framing.grid_mesh(
            (63.0, 62.0), (2466.0, 1202.0), 50.0, 0.25, -0.83
        )
        for face in faces:
            for index in face:
                self.assertLess(index, len(vertices))
```

- [ ] **Step 2: Run test to verify it fails**

Run: `python3 -m unittest -q tests/test_crowd_framing.py`
Expected: FAIL with `AttributeError: module 'crowd_framing' has no attribute 'grid_line_positions'`.

- [ ] **Step 3: Write minimal implementation**

Append to `scripts/crowd_framing.py`:

```python
def grid_line_positions(low, high, spacing):
    """Multiples of `spacing` lying within `[low, high]`.

    Anchored on world zero rather than on `low`, so the lines stay put as
    the tracking camera moves and read as a fixed surface rather than as
    something following the camera.
    """
    if spacing <= 0:
        raise ValueError("grid spacing must be positive")
    first = math.ceil(low / spacing)
    last = math.floor(high / spacing)
    return [index * spacing for index in range(first, last + 1)]


def grid_mesh(minimum, maximum, spacing, line_width, z):
    """Vertices and quad faces for a lattice of thin bars on the ground.

    Returned as plain lists so the caller can hand them straight to
    `Mesh.from_pydata`. The grid has to be geometry rather than a texture:
    the renderer is Workbench with `use_nodes = False`, which reads
    `diffuse_color` only and would ignore a node-based texture entirely.

    Without it a tracking camera crossing a featureless plane looks like a
    static camera watching agents mill in place.
    """
    half = line_width / 2.0
    low_x, low_y = minimum
    high_x, high_y = maximum
    vertices = []
    faces = []

    def add_quad(x0, y0, x1, y1):
        base = len(vertices)
        vertices.extend(
            [(x0, y0, z), (x1, y0, z), (x1, y1, z), (x0, y1, z)]
        )
        faces.append((base, base + 1, base + 2, base + 3))

    for x in grid_line_positions(low_x, high_x, spacing):
        add_quad(x - half, low_y, x + half, high_y)
    for y in grid_line_positions(low_y, high_y, spacing):
        add_quad(low_x, y - half, high_x, y + half)
    return vertices, faces
```

- [ ] **Step 4: Run test to verify it passes**

Run: `python3 -m unittest -q tests/test_crowd_framing.py`
Expected: PASS, 16 tests.

- [ ] **Step 5: Commit**

```bash
git add scripts/crowd_framing.py tests/test_crowd_framing.py
git commit -m "Generate a ground grid as geometry for the recording

A tracking camera crossing a featureless plane looks like a static
camera watching agents mill in place. A grid supplies the motion cue and
doubles as a scale reference.

It has to be geometry, not a texture: the renderer is Workbench with
use_nodes = False, which reads diffuse_color only. Lines are anchored on
world zero so they stay put as the camera moves."
```

---

### Task 4: Place a crop-mode camera

**Files:**
- Modify: `scripts/crowd_framing.py`
- Modify: `tests/test_crowd_framing.py`
- Modify: `scripts/render_playback.py` (`build_camera`, near line 208)

**Interfaces:**
- Consumes: `crowd_framing.clip_planes` from Task 1.
- Produces:
  - `crowd_framing.crop_camera_placement(view_width, tilt_ratio) -> (standoff, height)`.
  - `build_camera(centre, extent, view_width=None)` in `render_playback.py`. When `view_width` is `None` the whole-scene behaviour is unchanged; when set, `ortho_scale` is exactly `view_width` and the camera geometry derives from it.

- [ ] **Step 1: Write the failing test**

Append to `tests/test_crowd_framing.py`, above the `if __name__` block:

```python
class CropCameraTest(unittest.TestCase):
    def test_placement_derives_from_the_window_not_the_scene(self):
        standoff, height = crowd_framing.crop_camera_placement(250.0, 1.23)
        self.assertAlmostEqual(height, 200.0)
        self.assertAlmostEqual(standoff, 246.0)

    def test_tilt_is_scale_invariant(self):
        # A 250 m window and a 2500 m window must read as the same shot,
        # so standoff and height stay in proportion.
        near_standoff, near_height = crowd_framing.crop_camera_placement(250.0, 1.23)
        far_standoff, far_height = crowd_framing.crop_camera_placement(2500.0, 1.23)
        self.assertAlmostEqual(far_height / near_height, 10.0)
        self.assertAlmostEqual(far_standoff / near_standoff, 10.0)

    def test_crop_camera_stays_inside_its_own_clip_planes(self):
        standoff, height = crowd_framing.crop_camera_placement(250.0, 1.23)
        distance = math.hypot(standoff, height)
        near, far = crowd_framing.clip_planes(distance)
        self.assertLess(near, distance)
        self.assertGreater(far, distance)
```

Add `import math` to the top of `tests/test_crowd_framing.py`, below `import importlib.util`.

- [ ] **Step 2: Run test to verify it fails**

Run: `python3 -m unittest -q tests/test_crowd_framing.py`
Expected: FAIL with `AttributeError: module 'crowd_framing' has no attribute 'crop_camera_placement'`.

- [ ] **Step 3: Write minimal implementation**

Append to `scripts/crowd_framing.py`:

```python
def crop_camera_placement(view_width, tilt_ratio):
    """`(standoff along -y, height along +z)` for a crop-mode camera.

    Mirrors the whole-scene camera's geometry but measured from the width
    of the crop window rather than the extent of the scene, so the shot
    reads the same at any zoom.
    """
    height = view_width * 0.8
    return height * tilt_ratio, height
```

- [ ] **Step 4: Run test to verify it passes**

Run: `python3 -m unittest -q tests/test_crowd_framing.py`
Expected: PASS, 19 tests.

- [ ] **Step 5: Teach `build_camera` about a crop window**

In `scripts/render_playback.py`, change the signature and the two derived values. The function currently starts:

```python
def build_camera(centre, extent):
```

Change to:

```python
def build_camera(centre, extent, view_width=None):
```

Extend the docstring by appending this paragraph before the closing `"""`:

```
    `view_width` opts into crop mode: `ortho_scale` becomes exactly that
    width with no framing margin, so a 250 m window means 250 m, and the
    camera's standoff and height derive from the window rather than from
    the scene extent.
```

Then replace:

```python
    camera_data.ortho_scale = extent * FRAMING_MARGIN
```

with:

```python
    camera_data.ortho_scale = (
        view_width if view_width is not None else extent * FRAMING_MARGIN
    )
```

and replace:

```python
    height = extent * 0.8
    camera.location = (
        centre[0],
        centre[1] - height * CAMERA_TILT_RATIO,
        height,
    )
```

with:

```python
    if view_width is not None:
        standoff, height = crowd_framing.crop_camera_placement(
            view_width, CAMERA_TILT_RATIO
        )
    else:
        height = extent * 0.8
        standoff = height * CAMERA_TILT_RATIO
    camera.location = (centre[0], centre[1] - standoff, height)
```

- [ ] **Step 6: Verify the default path is byte-identical**

Run the same crossing render as Task 1 Step 6, into a fresh directory:

```bash
CROWD_TRACE_PATH=/tmp/crowd-plan-check/crossing-1000.crowdtrace \
CROWD_FRAME_DIR=/tmp/crowd-plan-check/frames-after \
CROWD_TICK_STEP=2000 CROWD_RES_X=640 \
/Applications/Blender.app/Contents/MacOS/Blender -b --factory-startup \
  --python scripts/render_playback.py
```

Then compare the frames pixel for pixel:

```bash
python3 - <<'EOF'
from PIL import Image
import numpy as np
import pathlib

before = sorted(pathlib.Path("/tmp/crowd-plan-check/frames").glob("*.png"))
after = sorted(pathlib.Path("/tmp/crowd-plan-check/frames-after").glob("*.png"))
assert before, "no baseline frames -- rerun the Task 1 Step 6 render first"
assert len(before) == len(after), (len(before), len(after))
for b, a in zip(before, after):
    x = np.array(Image.open(b).convert("RGBA"), dtype=np.int16)
    y = np.array(Image.open(a).convert("RGBA"), dtype=np.int16)
    assert x.shape == y.shape and not np.any(x - y), b.name
print("IDENTICAL: {} frames match pixel for pixel".format(len(before)))
EOF
```

Expected: `IDENTICAL: 3 frames match pixel for pixel`. If any frame differs, the
refactor changed the default path and must be corrected before moving on.

Compare pixels rather than bytes: Blender stamps `Date` and `RenderTime` into
each PNG's `tEXt` chunks, so two renders of an identical scene are never
byte-identical and `diff`/`cmp` would always report a difference. This was
measured before the plan was executed -- the pixels came back bit-identical
while the files did not.

- [ ] **Step 7: Commit**

```bash
git add scripts/crowd_framing.py tests/test_crowd_framing.py scripts/render_playback.py
git commit -m "Let build_camera take an explicit crop window

In crop mode ortho_scale is exactly the requested width with no framing
margin, so a 250 m window means 250 m, and standoff and height derive
from the window rather than the scene extent. Unset, every value is what
it was, verified by rendering the crossing fixture before and after and
diffing the frames."
```

---

### Task 5: Wire crop mode into the renderer

**Files:**
- Modify: `scripts/render_playback.py` (module docstring near line 18-23; `scan_trace` near line 75; `main` near line 275)

**Interfaces:**
- Consumes: `fit_linear_track`, `track_residuals`, `grid_mesh`, `crop_camera_placement`, `clip_planes`.
- Produces: `scan_trace(playback, data, track_stream=None)` now returns `(stream, minimum, maximum, track)`, where `track` is a list of `(tick, median_x)` pairs for the tracked stream, empty when `track_stream` is `None`. Also `build_grid(minimum, maximum, spacing)` returning the grid object.

- [ ] **Step 1: Add the environment variables and document them**

In the module docstring, the `Environment:` block currently ends:

```
    CROWD_TICK_STEP   render every Nth tick (default 20)
    CROWD_RES_X       horizontal resolution (default 960)
```

Replace those two lines with:

```
    CROWD_TICK_STEP   render every Nth tick (default 20)
    CROWD_RES_X       horizontal resolution (default 960)
    CROWD_CROP_WIDTH  ortho window width in metres. Unset frames the whole
                      scene, which is the historical behaviour.
    CROWD_TRACK_STREAM  stream label the crop follows (0 or 1). Only read
                      when CROWD_CROP_WIDTH is set.
    CROWD_GROUND_GRID  ground grid spacing in metres. Unset draws no grid.
```

Below the existing `STREAM_SPLIT` definition near line 72, add:

```python
# Crop mode. Unset, every one of these is inert and framing is unchanged.
CROP_WIDTH = float(os.environ.get("CROWD_CROP_WIDTH", "0")) or None
TRACK_STREAM = int(os.environ.get("CROWD_TRACK_STREAM", "0"))
GROUND_GRID = float(os.environ.get("CROWD_GROUND_GRID", "0")) or None

# Grid bars ~0.25 m wide land at ~4 px in a 250 m window rendered 3840 wide:
# visible as a line, too thin to compete with the agents.
GRID_LINE_WIDTH = 0.25
# Lifted clear of the ground plane so it never z-fights with it.
GRID_LIFT = 0.02
COLOUR_GRID = (0.06, 0.07, 0.09, 1.0)
```

- [ ] **Step 2: Collect the tracked median during the existing scan**

`scan_trace` already walks every tick, so the median costs no extra I/O. Change the signature:

```python
def scan_trace(playback, data):
```

to:

```python
def scan_trace(playback, data, track_stream=None):
```

Immediately after the existing `newly_spawned` block (the `if np.any(newly_spawned):` statement and its body), and still inside the `for tick in ...` loop, add:

```python
        if track_stream is not None:
            # Every live agent already carries a label: a slot is labelled
            # the first tick it holds an agent, so this is exact rather
            # than an approximation that improves as the scan proceeds.
            tracked = live & (stream == track_stream)
            if np.any(tracked):
                track.append((tick, float(np.median(xy[tracked][:, 0]))))
```

Add `track = []` beside the other accumulators at the top of the function, after `maximum = ...`:

```python
    track = []
```

Change the closing `return` from:

```python
    return stream, minimum, maximum
```

to:

```python
    return stream, minimum, maximum, track
```

And extend the docstring's `Returns` line from:

```
    Returns (stream, minimum, maximum) over the live agents.
```

to:

```
    Returns (stream, minimum, maximum, track) over the live agents, where
    `track` is the (tick, median x) of the `track_stream` agents and is
    empty when no stream is being tracked.
```

- [ ] **Step 3: Add the grid builder**

Insert this function directly after `build_ground` in `scripts/render_playback.py`:

```python
def build_grid(minimum, maximum, spacing):
    """Lay a grid of thin bars over the ground plane.

    Presentation only, like the ground itself: it never touches agent
    state. It exists so a tracking camera reads as moving rather than as a
    static shot of agents milling in place, and it doubles as a scale
    reference.
    """
    vertices, faces = crowd_framing.grid_mesh(
        minimum,
        maximum,
        spacing,
        GRID_LINE_WIDTH,
        -CONE_HALF_DEPTH + GRID_LIFT,
    )
    mesh = bpy.data.meshes.new("grid")
    mesh.from_pydata(vertices, [], faces)
    mesh.update()
    grid = bpy.data.objects.new("grid", mesh)
    bpy.context.scene.collection.objects.link(grid)
    grid.data.materials.append(make_material("grid", COLOUR_GRID))
    print("grid: {} bars every {:.0f} m".format(len(faces), spacing))
    return grid
```

- [ ] **Step 4: Wire it all into `main`**

In `main`, replace:

```python
    stream, minimum, maximum = scan_trace(playback, data)
```

with:

```python
    stream, minimum, maximum, track = scan_trace(
        playback, data, TRACK_STREAM if CROP_WIDTH is not None else None
    )
```

Replace:

```python
    build_ground(centre, extent)
    build_camera(centre, extent)
```

with:

```python
    build_ground(centre, extent)
    if GROUND_GRID is not None:
        build_grid(minimum, maximum, GROUND_GRID)

    if CROP_WIDTH is not None:
        if not track:
            raise SystemExit(
                "no agent ever carried stream label {}; nothing to track".format(
                    TRACK_STREAM
                )
            )
        fit = crowd_framing.fit_linear_track(track)
        worst, rms = crowd_framing.track_residuals(track, fit)
        print("track fit: x(tick) = {:.6f} * tick + {:.4f}".format(*fit))
        print(
            "track residual: worst {:.2f} m, rms {:.2f} m "
            "({:.2f}% of the {:.0f} m window)".format(
                worst, rms, worst / CROP_WIDTH * 100.0, CROP_WIDTH
            )
        )
        band_centre_y = tracked_band_centre_y(data, stream, TRACK_STREAM)
        print("tracking stream {} centred on y {:.1f}".format(TRACK_STREAM, band_centre_y))
        camera = build_camera(
            (fit[1], band_centre_y), extent, view_width=CROP_WIDTH
        )
    else:
        fit = None
        camera = build_camera(centre, extent)
```

Add the helper directly above `main`:

```python
def tracked_band_centre_y(data, stream, track_stream):
    """Mid-height of the band the tracked stream occupies.

    Read from the data rather than passed in: the two lane blocks sit in
    fixed, disjoint y bands, so the centre of the tracked one is where the
    crop should sit, and hardcoding it would break on any other scene.
    """
    count = len(stream)
    positions = np.empty(count * 3, dtype=np.float32)
    data.attributes["position"].data.foreach_get("vector", positions)
    ys = positions.reshape(count, 3)[:, 1][stream == track_stream]
    return float((ys.min() + ys.max()) / 2.0)
```

Note this reads whatever tick the point cloud currently holds, which is the last tick of the scan. That is a tick where the tracked block is fully spawned and dispersed, so its band is at full height.

Finally, in the render loop, move the camera before each render. Replace:

```python
    for index, tick in enumerate(ticks):
        playback.sync_to_tick(tick)
```

with:

```python
    for index, tick in enumerate(ticks):
        playback.sync_to_tick(tick)
        if fit is not None:
            # Only x changes, so the camera's rotation stays correct: the
            # target slides by exactly as much as the camera does.
            camera.location.x = fit[0] * tick + fit[1]
```

- [ ] **Step 5: Render one crop frame and check it by eye**

```bash
CROWD_TRACE_PATH=$HOME/blender-crowd-m5/recording/m5_city_flow-100000.crowdtrace \
CROWD_FRAME_DIR=/tmp/crowd-crop-check \
CROWD_TICK_STEP=400 CROWD_RES_X=3840 \
CROWD_STREAM_AXIS=y CROWD_STREAM_SPLIT=632.4555320336759 \
CROWD_CROP_WIDTH=250 CROWD_TRACK_STREAM=0 CROWD_GROUND_GRID=50 \
/Applications/Blender.app/Contents/MacOS/Blender -b --factory-startup \
  --python scripts/render_playback.py
```

Expected in the log:
- `track fit: x(tick) = 1.924605 * tick + 279.4882`
- `track residual: worst 10.55 m, rms 4.56 m (4.22% of the 250 m window)`
- `tracking stream 0 centred on y` around `316`
- `grid: 71 bars every 50 m` (48 lines along x, 23 along y)
- `PASS: rendered 3 frames`

Open the frames and confirm: agents are individually distinguishable cones with visible facing, grid lines are visible but not dominant, and the crowd sits near frame centre in all three.

- [ ] **Step 6: Verify the default path is still unchanged**

```bash
CROWD_TRACE_PATH=/tmp/crowd-plan-check/crossing-1000.crowdtrace \
CROWD_FRAME_DIR=/tmp/crowd-plan-check/frames-task5 \
CROWD_TICK_STEP=2000 CROWD_RES_X=640 \
/Applications/Blender.app/Contents/MacOS/Blender -b --factory-startup \
  --python scripts/render_playback.py
```

Then compare the frames pixel for pixel:

```bash
python3 - <<'EOF'
from PIL import Image
import numpy as np
import pathlib

before = sorted(pathlib.Path("/tmp/crowd-plan-check/frames").glob("*.png"))
after = sorted(pathlib.Path("/tmp/crowd-plan-check/frames-task5").glob("*.png"))
assert before, "no baseline frames -- rerun the Task 1 Step 6 render first"
assert len(before) == len(after), (len(before), len(after))
for b, a in zip(before, after):
    x = np.array(Image.open(b).convert("RGBA"), dtype=np.int16)
    y = np.array(Image.open(a).convert("RGBA"), dtype=np.int16)
    assert x.shape == y.shape and not np.any(x - y), b.name
print("IDENTICAL: {} frames match pixel for pixel".format(len(before)))
EOF
```

Expected: `IDENTICAL: 3 frames match pixel for pixel`. If any frame differs, the
refactor changed the default path and must be corrected before moving on.

Compare pixels rather than bytes: Blender stamps `Date` and `RenderTime` into
each PNG's `tEXt` chunks, so two renders of an identical scene are never
byte-identical and `diff`/`cmp` would always report a difference. This was
measured before the plan was executed -- the pixels came back bit-identical
while the files did not.

- [ ] **Step 7: Commit**

```bash
git add scripts/render_playback.py
git commit -m "Add an opt-in cropped, tracking camera to the recorder

CROWD_CROP_WIDTH frames a fixed-width window instead of the whole scene,
CROWD_TRACK_STREAM picks the lane block it follows, and
CROWD_GROUND_GRID lays down a scale reference so the pan reads as motion.
With none set, framing is exactly what it was.

The tracked median x is collected during the scan that already walks
every tick, so it costs no extra I/O, and the band centre is read from
the data rather than hardcoded."
```

---

### Task 6: Point the 100K recording at the new framing

**Files:**
- Modify: `scripts/make-m5-100k-recording.sh`

**Interfaces:**
- Consumes: the `CROWD_CROP_WIDTH`, `CROWD_TRACK_STREAM`, `CROWD_GROUND_GRID` variables from Task 5.
- Produces: the finished mp4.

- [ ] **Step 1: Raise the resolution default**

In `scripts/make-m5-100k-recording.sh`, change:

```sh
RES_X="${RES_X:-1280}"
```

to:

```sh
RES_X="${RES_X:-3840}"
```

and in the `Environment overrides:` comment block change:

```
#   RES_X           horizontal resolution (default 1280)
```

to:

```
#   RES_X           horizontal resolution (default 3840)
#   CROP_WIDTH      width of the tracked window in metres (default 250)
#   TRACK_STREAM    lane block the camera follows, 0 south or 1 north
#                   (default 0)
#   GROUND_GRID     ground grid spacing in metres (default 50)
```

- [ ] **Step 2: Add the crop defaults**

Directly below the `FPS="${FPS:-30}"` line, add:

```sh
CROP_WIDTH="${CROP_WIDTH:-250}"
TRACK_STREAM="${TRACK_STREAM:-0}"
GROUND_GRID="${GROUND_GRID:-50}"
```

- [ ] **Step 3: Pass them to the renderer**

Change the render invocation from:

```sh
CROWD_TRACE_PATH="$TRACE" \
CROWD_FRAME_DIR="$FRAME_DIR" \
CROWD_TICK_STEP=1 \
CROWD_RES_X="$RES_X" \
CROWD_STREAM_AXIS=y \
CROWD_STREAM_SPLIT="$STREAM_SPLIT" \
    "$BLENDER" -b --factory-startup --python "$REPO_ROOT/scripts/render_playback.py"
```

to:

```sh
CROWD_TRACE_PATH="$TRACE" \
CROWD_FRAME_DIR="$FRAME_DIR" \
CROWD_TICK_STEP=1 \
CROWD_RES_X="$RES_X" \
CROWD_STREAM_AXIS=y \
CROWD_STREAM_SPLIT="$STREAM_SPLIT" \
CROWD_CROP_WIDTH="$CROP_WIDTH" \
CROWD_TRACK_STREAM="$TRACK_STREAM" \
CROWD_GROUND_GRID="$GROUND_GRID" \
    "$BLENDER" -b --factory-startup --python "$REPO_ROOT/scripts/render_playback.py"
```

- [ ] **Step 4: State what the clip does and does not show**

In the header comment block, directly after the paragraph beginning `It is also a TRUNCATED run:`, insert:

```
# It is also a CROPPED view. The camera frames a 250 m window that tracks one
# of the two lane blocks, because the whole 2,402 x 1,140 m scene rendered to
# a single frame puts an agent at well under a pixel. So the clip shows on the
# order of 8,300-12,300 agents at a time, under 1% of the scene area, and must
# never be captioned as a picture of 100,000 agents. That claim belongs to
# docs/media/m5-100k-hero.png, which is a measured plot asserted above 95%
# occupancy.
```

Add to the `=== plan ===` block, after the `frames` line:

```sh
printf 'window           %s m, tracking stream %s\n' "$CROP_WIDTH" "$TRACK_STREAM"
```

And change the closing reminder from:

```sh
printf '\nReminder: a visualisation, not a measurement, and a truncated run.\n'
```

to:

```sh
printf '\nReminder: a visualisation, not a measurement; a truncated run; and a\n'
printf 'cropped view showing well under 1%% of the population at a time.\n'
```

- [ ] **Step 5: Run the whole thing**

The trace already exists, so only the render and encode stages run.

```bash
scripts/make-m5-100k-recording.sh 2>&1 | tee /tmp/m5-recording.log
```

Expected: exits 0, reports `rendering 875 frames`, the same track fit and residual lines as Task 5, and a final `wrote .../m5-city-flow-100000.mp4`. Render time roughly 15-20 min at 4K.

- [ ] **Step 6: Check a frame out of the encoded video, not just the PNGs**

```bash
ffmpeg -y -loglevel error -i "$HOME/blender-crowd-m5/recording/m5-city-flow-100000.mp4" \
  -vf "select='eq(n\,437)'" -vframes 1 /tmp/m5-encoded-frame.png
ffprobe -v error -show_entries stream=width,height,nb_frames -of default=nw=1 \
  "$HOME/blender-crowd-m5/recording/m5-city-flow-100000.mp4"
```

Expected: `width=3840`, `height=2160`, `nb_frames=875`. Open `/tmp/m5-encoded-frame.png` and confirm agents are individually legible and the grid is visible.

- [ ] **Step 7: Run the full check set and commit**

```bash
python3 -m unittest -q tests/test_crowd_framing.py
git diff --check
git status --short
```

Expected: tests pass, no whitespace errors.

```bash
git add scripts/make-m5-100k-recording.sh
git commit -m "Record the 100K clip cropped and tracked at 4K

The whole-scene framing put an agent at 0.23 px and the clip read as a
pale wash. A 250 m tracked window at 3840 px gives 15.36 px/m, so an
agent cone is 7.7 px across and its facing is visible.

The header now says the view is cropped and that the clip shows under 1%
of the population at a time, so it is never mistaken for a picture of
100,000 agents."
```

---

## Notes for the implementer

- Run the Blender steps with normal host access. A restricted automation sandbox returns no Metal device on macOS and crashes Blender before Python starts, which looks like a Python error but is not one.
- `scripts/crowd_framing.py` is deliberately dependency-free. If you find yourself wanting `numpy` in it, put the `numpy` part in `render_playback.py` and keep the pure maths in the module, or the test file stops working outside Blender.
- Blender PNGs are never byte-identical between runs: `Date` and `RenderTime`
  go into the file's `tEXt` chunks. Compare rendered frames by pixel, never
  with `diff` or `cmp`.
- The 3 GB trace at `~/blender-crowd-m5/recording/m5_city_flow-100000.crowdtrace` already exists. Do not re-bake it; that stage is multi-hour.
