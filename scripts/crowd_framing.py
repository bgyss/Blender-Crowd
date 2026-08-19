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
