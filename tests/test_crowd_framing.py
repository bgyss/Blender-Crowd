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
        # The 100K scene stands the camera ~3045.6 units back (full-scan
        # occupied extent 2401.6 m -> height 1921.28 -> standoff 2363.17 ->
        # hypot(standoff, height) = 3045.6). Blender's default far plane of
        # 1000 sits well inside that and clips the whole scene away.
        distance = math.hypot(2401.6 * 0.8 * 1.23, 2401.6 * 0.8)
        near, far = crowd_framing.clip_planes(distance)
        self.assertGreater(far, distance)
        self.assertLess(near, distance)

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


class TrackedBandCentreYTest(unittest.TestCase):
    def test_medians_the_live_tracked_agents(self):
        flags = [1, 1, 1]
        stream = [0, 0, 0]
        ys = [100.0, 200.0, 900.0]
        self.assertEqual(
            crowd_framing.tracked_band_centre_y(flags, stream, ys, 0), 200.0
        )

    def test_ignores_agents_in_the_other_stream(self):
        flags = [1, 1, 1, 1]
        stream = [0, 0, 1, 1]
        ys = [100.0, 200.0, 5000.0, 6000.0]
        self.assertEqual(
            crowd_framing.tracked_band_centre_y(flags, stream, ys, 0), 150.0
        )

    def test_never_spawned_slot_does_not_drag_the_centre(self):
        # A slot that never held an agent carries flags == 0 and, after
        # scan_trace's `stream[stream < 0] = 0` fallback, can be relabelled
        # into stream 0 while sitting at the trace format's padding position
        # -- the origin -- rather than a real y. Without the flags != 0
        # filter this y = 0.0 would pull the median toward the origin.
        flags = [1, 1, 0]
        stream = [0, 0, 0]
        ys = [300.0, 320.0, 0.0]
        self.assertEqual(
            crowd_framing.tracked_band_centre_y(flags, stream, ys, 0), 310.0
        )

    def test_returns_none_when_nothing_is_live_this_tick(self):
        flags = [0, 0]
        stream = [0, 1]
        ys = [10.0, 20.0]
        self.assertIsNone(crowd_framing.tracked_band_centre_y(flags, stream, ys, 0))

    def test_returns_none_when_tracked_stream_has_no_live_agents(self):
        flags = [1, 1]
        stream = [1, 1]
        ys = [10.0, 20.0]
        self.assertIsNone(crowd_framing.tracked_band_centre_y(flags, stream, ys, 0))


class BandCentreTest(unittest.TestCase):
    def test_medians_the_per_tick_values(self):
        self.assertEqual(crowd_framing.band_centre([300.0, 320.0, 900.0]), 320.0)

    def test_a_single_outlier_tick_cannot_drag_the_centre_far(self):
        values = [316.0] * 20 + [900.0]
        # A min/max centre would be pulled to (316 + 900) / 2 = 608; a median
        # over many ticks is not.
        self.assertEqual(crowd_framing.band_centre(values), 316.0)

    def test_no_samples_is_an_error_not_a_silent_zero(self):
        with self.assertRaisesRegex(ValueError, "no samples"):
            crowd_framing.band_centre([])


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


class CameraYawTest(unittest.TestCase):
    """Why the recording camera is yawed off the lane axis.

    m5_city_flow runs parallel one-way lanes along +x at a 2.663 m pitch
    (16 * scale / lanes_per_direction, both at 100K). Looking straight down
    that axis maps every lane onto image *rows*, so the lane periodicity sums
    coherently across all 3840 columns and reads as horizontal banding. The
    banding is real scene structure, not an encoding artefact -- a row-occupancy
    FFT of the 100K clip peaks at 25.7 px, and 25.7 px * 0.1032 m/px = 2.65 m.

    Yaw does not remove the lanes. It stops them from lining up with the pixel
    grid, which is what made them read as bands.
    """

    def test_zero_yaw_reproduces_the_straight_on_camera(self):
        # The whole point of the default: every existing recording that does
        # not opt into yaw must place its camera exactly where it did before.
        standoff, _height = crowd_framing.crop_camera_placement(250.0, 1.23)
        offset_x, offset_y = crowd_framing.yaw_offsets(standoff, 0.0)
        self.assertAlmostEqual(offset_x, 0.0)
        self.assertAlmostEqual(offset_y, -standoff)

    def test_yaw_preserves_the_standoff_distance(self):
        # Yaw swings the camera around the target; it must not dolly in or
        # out, or the shot's scale and clip planes would drift with it.
        standoff, _height = crowd_framing.crop_camera_placement(250.0, 1.23)
        for yaw in (0.0, 12.5, 30.0, 45.0, -30.0):
            offset_x, offset_y = crowd_framing.yaw_offsets(standoff, yaw)
            self.assertAlmostEqual(math.hypot(offset_x, offset_y), standoff, places=5)

    def test_yaw_swings_the_camera_the_signed_way_round(self):
        offset_x, offset_y = crowd_framing.yaw_offsets(100.0, 90.0)
        self.assertAlmostEqual(offset_x, 100.0, places=5)
        self.assertAlmostEqual(offset_y, 0.0, places=5)

    def test_lanes_are_exactly_horizontal_without_yaw(self):
        # This is the defect, stated as a test: at yaw 0 a lane never leaves
        # the row it starts in, whatever the tilt.
        self.assertAlmostEqual(crowd_framing.lane_screen_slope(1.23, 0.0), 0.0)

    def test_lane_slope_grows_with_yaw(self):
        shallow = crowd_framing.lane_screen_slope(1.23, 10.0)
        steep = crowd_framing.lane_screen_slope(1.23, 30.0)
        self.assertGreater(steep, shallow)
        self.assertGreater(shallow, 0.0)

    def test_lane_slope_is_foreshortened_by_the_tilt(self):
        # A world line along +x rises on screen by tan(yaw), foreshortened by
        # sin(elevation). tilt_ratio is standoff/height, so tan(elevation) is
        # its reciprocal.
        elevation = math.atan2(1.0, 1.23)
        expected = math.tan(math.radians(30.0)) * math.sin(elevation)
        self.assertAlmostEqual(
            crowd_framing.lane_screen_slope(1.23, 30.0), expected
        )

    def test_thirty_degrees_destroys_row_coherence_at_4k(self):
        # The fix, stated as a measurement rather than left to the eye: at the
        # 100K recording's tilt, a lane crosses out of any given image row
        # within a handful of columns, so its 2.66 m periodicity can no longer
        # accumulate across a 3840-wide frame the way it does at yaw 0.
        columns = crowd_framing.lane_row_coherence_columns(1.23, 30.0)
        self.assertLess(columns, 10.0)

    def test_row_coherence_is_unbounded_without_yaw(self):
        self.assertEqual(
            crowd_framing.lane_row_coherence_columns(1.23, 0.0), math.inf
        )


if __name__ == "__main__":
    unittest.main()
