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
    ~3046 units out, so at the default every object -- agents and ground
    alike -- falls beyond the far plane and the render is nothing but world
    background. Deriving both planes from the actual camera-to-target
    distance means framing and clipping can never disagree.
    """
    return max(0.1, distance * 0.01), distance * 4.0


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


def _median(values):
    """Median of a non-empty sequence of numbers, as a plain float."""
    ordered = sorted(values)
    count = len(ordered)
    middle = count // 2
    if count % 2:
        return float(ordered[middle])
    return float((ordered[middle - 1] + ordered[middle]) / 2.0)


def tracked_band_centre_y(flags, stream, ys, track_stream):
    """Median y of the live agents carrying `track_stream`'s label, in one tick.

    A slot that never spawned carries `flags == 0` yet, after `scan_trace`'s
    `stream[stream < 0] = 0` fallback, can carry stream label 0 while sitting
    at the trace format's padding position (the origin) rather than a real y.
    Filtering on `flags != 0` -- matching `scan_trace`'s own `live = flags !=
    0` -- keeps such a slot from dragging the centre.

    Returns `None` when no tracked agent is live this tick, so a gap tick can
    be skipped by the caller rather than crashing on an empty reduction.
    """
    live_ys = [
        y for flag, label, y in zip(flags, stream, ys)
        if flag != 0 and label == track_stream
    ]
    if not live_ys:
        return None
    return _median(live_ys)


def band_centre(per_tick_values):
    """Median of per-tick band centres over a whole clip.

    A median over many ticks, rather than a min/max computed from a single
    tick's positions, so neither a single straggler nor a single unusual
    tick can drag the crop off the band.
    """
    if not per_tick_values:
        raise ValueError("cannot compute a band centre from no samples")
    return _median(per_tick_values)


def crop_camera_placement(view_width, tilt_ratio):
    """`(standoff along -y, height along +z)` for a crop-mode camera.

    Mirrors the whole-scene camera's geometry but measured from the width
    of the crop window rather than the extent of the scene, so the shot
    reads the same at any zoom.
    """
    height = view_width * 0.8
    return height * tilt_ratio, height
