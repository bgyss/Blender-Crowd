import hashlib
import importlib.util
import io
import json
import math
import sys
import tempfile
from pathlib import Path
import unittest


ROOT = Path(__file__).parents[1]
sys.path.insert(0, str(ROOT / "scripts"))
ASF = ROOT / "tests" / "fixtures" / "m6" / "cmu-mini.asf"
AMC = ROOT / "tests" / "fixtures" / "m6" / "cmu-mini.amc"


def load_script(name):
    path = ROOT / "scripts" / f"{name}.py"
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def fixture_manifest():
    return {
        "schema_version": 1,
        "dataset_id": "cmu-mini",
        "source_host": "mocap.cs.cmu.edu",
        "terms_url": "http://mocap.cs.cmu.edu/info.php?info=faq",
        "license_id": "CMU-Mocap-Free-All-Uses",
        "redistribution_allowed": False,
        "required_attribution": "Carnegie Mellon University Graphics Lab Motion Capture Database",
        "source_frame_rate_hz": 120,
        "target_frame_rate_hz": 30,
        "retarget_profile": {
            "profile_id": "cmu-acclaim-humanoid-v1",
            "root_bone": "root",
            "left_foot_bone": "lfoot",
            "right_foot_bone": "rfoot",
            "forward_axis": "+Z",
        },
        "files": [
            {
                "id": "mini_skeleton",
                "kind": "skeleton",
                "subject": 0,
                "description": "Hand-checked mini skeleton",
                "url": "http://mocap.cs.cmu.edu/subjects/0/cmu-mini.asf",
                "filename": "cmu-mini.asf",
                "sha256": hashlib.sha256(ASF.read_bytes()).hexdigest(),
            },
            {
                "id": "mini_walk",
                "kind": "motion",
                "subject": 0,
                "trial": 1,
                "clip_id": "mini_walk",
                "description": "Hand-checked mini motion",
                "skeleton_id": "mini_skeleton",
                "url": "http://mocap.cs.cmu.edu/subjects/0/cmu-mini.amc",
                "filename": "cmu-mini.amc",
                "sha256": hashlib.sha256(AMC.read_bytes()).hexdigest(),
            },
        ],
    }


class FakeResponse(io.BytesIO):
    def __init__(self, data, url):
        super().__init__(data)
        self._url = url

    def geturl(self):
        return self._url

    def __enter__(self):
        return self

    def __exit__(self, *_args):
        self.close()


class FakeOpener:
    def __init__(self, payloads):
        self.payloads = payloads
        self.requested = []

    def open(self, request, timeout):
        url = request.full_url
        self.requested.append((url, timeout))
        data, final_url = self.payloads[url]
        return FakeResponse(data, final_url)


class M6CmuMotionTest(unittest.TestCase):
    def test_forward_kinematics_preserves_xyz_values_under_permuted_orders(self):
        ingest = load_script("m6_cmu_motion_ingest")
        skeleton = {
            "length_scale_mm": 1.0,
            "root_order": ["TX", "TY", "TZ", "RZ", "RX", "RY"],
            "root_axis": "YZX",
            "root_position": (0.0, 0.0, 0.0),
            "root_orientation": (5.0, 10.0, 15.0),
            "bones": {
                "foot": {
                    "direction": (1.0, 0.0, 0.0),
                    "length_mm": 10.0,
                    "axis": (30.0, 20.0, 10.0),
                    "axis_order": "ZXY",
                    "dof": ["rz", "rx", "ry"],
                    "limits": [],
                }
            },
            "children": {"root": ["foot"]},
        }
        frame = {
            "source_frame": 1,
            "channels": {
                "root": [0.0, 0.0, 0.0, 25.0, 35.0, 15.0],
                "foot": [40.0, 50.0, 60.0],
            },
        }
        _, root_rotation, endpoints, _ = ingest._frame_world(skeleton, frame)
        expected_root = (
            (0.8027588832811212, -0.34991263847893106, 0.4828450276703281),
            (0.5855522024605384, 0.6156378662204942, -0.5273695439339379),
            (-0.11272441397878141, 0.7060815555643624, 0.6991008821228523),
        )
        expected_endpoint = (3.0687283404321852, 9.499289012003814, -0.5885699950319878)
        for actual_row, expected_row in zip(root_rotation, expected_root):
            for actual, expected in zip(actual_row, expected_row):
                self.assertAlmostEqual(actual, expected, places=12)
        for actual, expected in zip(endpoints["foot"], expected_endpoint):
            self.assertAlmostEqual(actual, expected, places=12)

    def test_metric_ceilings_have_no_epsilon_or_rounding_headroom(self):
        ingest = load_script("m6_cmu_motion_ingest")
        self.assertEqual(ingest._ceil_metric(1.0), 1)
        self.assertEqual(ingest._ceil_metric(1.0000000001), 2)
        self.assertEqual(ingest._turn_discontinuity_microradians(0.0, 0.00000125), 2)
        self.assertEqual(
            ingest._turn_discontinuity_microradians(math.pi - 0.000001, -math.pi + 0.0000010001),
            3,
        )

    def test_fetch_rejects_a_hash_mismatch_before_publish(self):
        fetch = load_script("m6_fetch_cmu_motion")
        with self.assertRaisesRegex(ValueError, "SHA-256"):
            fetch.verify_download(b"wrong", "0" * 64)

    def test_fetch_refuses_off_host_redirects_and_extra_manifest_files(self):
        fetch = load_script("m6_fetch_cmu_motion")
        manifest = json.loads((ROOT / "assets" / "reference" / "m6" / "cmu-motion-source-v1.json").read_text())
        opener = FakeOpener(
            {
                entry["url"]: (b"not published", "http://example.com/stolen")
                for entry in manifest["files"]
            }
        )
        with tempfile.TemporaryDirectory() as directory:
            with self.assertRaisesRegex(ValueError, "redirect"):
                fetch.fetch_manifest(manifest, directory, opener=opener)
            self.assertEqual(list(Path(directory).iterdir()), [])
        manifest["files"].append(dict(manifest["files"][-1], id="extra", filename="extra.amc"))
        with self.assertRaisesRegex(ValueError, "extra"):
            fetch.validate_manifest(manifest)

    def test_ingest_uses_declared_units_world_space_feet_and_fixed_downsampling(self):
        ingest = load_script("m6_cmu_motion_ingest")
        database = ingest.ingest_parser_fixture(ASF, [AMC], fixture_manifest())
        clip = database["clips"][0]
        self.assertEqual(clip["samples"][1]["velocity_millimeters_per_second"], [1000, 0])
        self.assertEqual(clip["left_foot_contacts"], [[0, 1]])
        self.assertEqual(clip["right_foot_contacts"], [[0, 1]])
        self.assertEqual([sample["source_frame"] for sample in clip["samples"]], [1, 5, 9])
        self.assertAlmostEqual(clip["samples"][0]["left_foot_position_millimeters"][0], 16.667, places=3)
        self.assertAlmostEqual(clip["samples"][1]["left_foot_position_millimeters"][0], 16.667, places=3)
        self.assertEqual(clip["metrics"]["max_foot_slide_millimeters"], 0)
        self.assertEqual(clip["metrics"]["max_trajectory_deviation_millimeters"], 1)
        self.assertEqual(clip["metrics"]["rejected_frame_rate_ppm"], 0)

    def test_ingest_records_malformed_frames_and_requires_two_retained_samples(self):
        ingest = load_script("m6_cmu_motion_ingest")
        malformed = AMC.read_text().replace("rfoot 0 90 0\n4", "4")
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "cmu-mini.amc"
            path.write_text(malformed)
            manifest = fixture_manifest()
            manifest["files"][1]["sha256"] = hashlib.sha256(path.read_bytes()).hexdigest()
            database = ingest.ingest_parser_fixture(ASF, [path], manifest)
            clip = database["clips"][0]
            self.assertEqual(clip["rejected_frames"], [{"source_frame": 3, "reason": "missing bone rfoot"}])
            self.assertEqual(clip["metrics"]["rejected_frame_rate_ppm"], 111112)

            too_short = "\n".join(AMC.read_text().splitlines()[:8]) + "\n"
            path.write_text(too_short)
            manifest["files"][1]["sha256"] = hashlib.sha256(path.read_bytes()).hexdigest()
            with self.assertRaisesRegex(ValueError, "two retained"):
                ingest.ingest_parser_fixture(ASF, [path], manifest)

    def test_ingest_rejects_weakened_license_boundaries_and_malformed_channels(self):
        ingest = load_script("m6_cmu_motion_ingest")
        manifest = fixture_manifest()
        manifest["redistribution_allowed"] = True
        with self.assertRaisesRegex(ValueError, "redistribution"):
            ingest.ingest(ASF, [AMC], manifest)

        malformed_inputs = (
            ("root 0 0 0 0 0 0", "root nan 0 0 0 0 0", "non-finite channels for bone root"),
            ("root 0 0 0 0 0 0", "root 0 0 0 0 0 0\nmystery 0", "unknown bone mystery"),
        )
        for original, replacement, reason in malformed_inputs:
            with self.subTest(reason=reason), tempfile.TemporaryDirectory() as directory:
                path = Path(directory) / "cmu-mini.amc"
                path.write_text(AMC.read_text().replace(original, replacement, 1))
                manifest = fixture_manifest()
                manifest["files"][1]["sha256"] = hashlib.sha256(path.read_bytes()).hexdigest()
                database = ingest.ingest_parser_fixture(ASF, [path], manifest)
                self.assertEqual(database["clips"][0]["rejected_frames"][0], {"source_frame": 1, "reason": reason})

    def test_source_manifest_contains_only_the_fixed_official_sources(self):
        fetch = load_script("m6_fetch_cmu_motion")
        source = json.loads((ROOT / "assets" / "reference" / "m6" / "cmu-motion-source-v1.json").read_text())
        fetch.validate_manifest(source)
        self.assertEqual(len(source["files"]), 5)
        self.assertFalse(source["redistribution_allowed"])

    def test_fetch_rejects_incomplete_provenance_and_trial_metadata(self):
        fetch = load_script("m6_fetch_cmu_motion")
        source = json.loads((ROOT / "assets" / "reference" / "m6" / "cmu-motion-source-v1.json").read_text())
        del source["required_attribution"]
        with self.assertRaisesRegex(ValueError, "attribution"):
            fetch.validate_manifest(source)
        source = json.loads((ROOT / "assets" / "reference" / "m6" / "cmu-motion-source-v1.json").read_text())
        del source["files"][1]["clip_id"]
        with self.assertRaisesRegex(ValueError, "motion metadata"):
            fetch.validate_manifest(source)

    def test_production_manifest_rejects_every_fixed_identity_mutation(self):
        fetch = load_script("m6_fetch_cmu_motion")
        source_path = ROOT / "assets" / "reference" / "m6" / "cmu-motion-source-v1.json"

        def mutate_skeleton_identity(manifest):
            manifest["files"][0]["id"] = "renamed_skeleton"
            manifest["files"][1]["skeleton_id"] = "renamed_skeleton"
            manifest["files"][2]["skeleton_id"] = "renamed_skeleton"

        mutations = (
            ("skeleton id", mutate_skeleton_identity),
            ("clip id", lambda manifest: manifest["files"][1].__setitem__("clip_id", "renamed_walk")),
            ("subject", lambda manifest: manifest["files"][1].__setitem__("subject", 36)),
            ("trial", lambda manifest: manifest["files"][1].__setitem__("trial", 24)),
            ("skeleton association", lambda manifest: manifest["files"][1].__setitem__("skeleton_id", "36_skeleton")),
        )
        for label, mutate in mutations:
            with self.subTest(label=label):
                manifest = json.loads(source_path.read_text())
                mutate(manifest)
                with self.assertRaisesRegex(ValueError, "fixed identity"):
                    fetch.validate_manifest(manifest)

    def test_parser_fixtures_use_an_explicit_non_production_entry_point(self):
        ingest = load_script("m6_cmu_motion_ingest")
        with self.assertRaisesRegex(ValueError, "fixed"):
            ingest.ingest(ASF, [AMC], fixture_manifest())
        database = ingest.ingest_parser_fixture(ASF, [AMC], fixture_manifest())
        self.assertEqual([clip["id"] for clip in database["clips"]], ["mini_walk"])

    def test_manifest_cli_path_uses_the_shared_fixed_validator_first(self):
        ingest = load_script("m6_cmu_motion_ingest")
        manifest = json.loads((ROOT / "assets" / "reference" / "m6" / "cmu-motion-source-v1.json").read_text())
        manifest["files"] = []
        with tempfile.TemporaryDirectory() as directory:
            with self.assertRaisesRegex(ValueError, "fixed|non-empty"):
                ingest.ingest_manifest(manifest, directory)

    def test_hard_evidence_distinguishes_measured_from_not_applicable(self):
        ingest = load_script("m6_cmu_motion_ingest")
        evaluate = load_script("m6_motion_evaluate")
        database = ingest.ingest_parser_fixture(ASF, [AMC], fixture_manifest())
        clip = database["clips"][0]
        self.assertEqual(
            set(clip["metrics"]),
            {
                "max_root_speed_error_millimeters_per_second",
                "max_foot_slide_millimeters",
                "max_trajectory_deviation_millimeters",
                "max_turn_discontinuity_microradians",
                "joint_limit_violations",
                "rejected_frames",
                "parsed_frames",
                "rejected_frame_rate_ppm",
                "source_hash_drift",
            },
        )
        self.assertEqual(
            {name: evidence["status"] for name, evidence in clip["evidence"].items()},
            {
                "retarget_failures": "not_applicable",
                "root_teleportations": "not_applicable",
                "undeclared_contacts": "not_applicable",
                "cross_cache_mutations": "not_applicable",
            },
        )

        report = evaluate.evaluate_database(database)
        self.assertEqual(
            set(report["hard_limit_observations"]),
            {"source_hash_drift", "joint_limit_violations"},
        )
        self.assertEqual(report["hard_limit_evidence"]["undeclared_contacts"]["status"], "not_applicable")
        self.assertNotIn("observed", report["hard_limit_evidence"]["undeclared_contacts"])
        self.assertEqual(report["hard_limit_evidence"]["root_teleportations"]["status"], "not_applicable")
        self.assertNotIn("observed", report["hard_limit_evidence"]["root_teleportations"])
        self.assertEqual(report["retarget_evidence"]["status"], "not_applicable")

    def test_checked_thresholds_equal_the_dated_observed_baseline(self):
        source = json.loads((ROOT / "assets" / "reference" / "m6" / "cmu-motion-source-v1.json").read_text())
        thresholds = json.loads((ROOT / "assets" / "reference" / "m6" / "motion-thresholds-v1.json").read_text())
        report = json.loads((ROOT / "docs" / "benchmarks" / "2026-08-18-m6-cmu-motion.json").read_text())
        expected_hashes = {entry["id"]: entry["sha256"] for entry in source["files"]}
        self.assertEqual(thresholds["source_hashes"], expected_hashes)
        self.assertEqual(report["source_hashes"], expected_hashes)
        measured = {"source_hash_drift", "joint_limit_violations"}
        unmeasured = {"root_teleportations", "undeclared_contacts", "cross_cache_mutations"}
        self.assertEqual(set(report["hard_limit_observations"]), measured)
        for name in measured:
            self.assertEqual(
                thresholds["hard_limits"][name],
                {
                    "limit": 0,
                    "evidence_status": "measured",
                    "baseline": report["hard_limit_observations"][name],
                },
            )
            self.assertEqual(report["hard_limit_evidence"][name]["status"], "measured")
        for name in unmeasured:
            self.assertEqual(thresholds["hard_limits"][name]["limit"], 0)
            self.assertEqual(thresholds["hard_limits"][name]["evidence_status"], "not_applicable")
            self.assertNotIn("baseline", thresholds["hard_limits"][name])
            self.assertNotIn("observed", report["hard_limit_evidence"][name])
        self.assertGreater(report["hard_limit_observations"]["joint_limit_violations"], 0)
        for metric, observed in report["threshold_baseline"].items():
            self.assertEqual(thresholds["soft_limits"][metric], {"baseline": observed, "limit": observed})


if __name__ == "__main__":
    unittest.main()
