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
