"""Framing maths for the recording renderer, tested without launching Blender."""

import importlib.util
import math
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


if __name__ == "__main__":
    unittest.main()
