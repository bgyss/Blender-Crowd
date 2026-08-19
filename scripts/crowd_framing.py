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
