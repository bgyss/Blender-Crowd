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


if __name__ == "__main__":
    unittest.main()
