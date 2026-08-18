import hashlib
import importlib.util
import io
import json
import tempfile
from pathlib import Path
import unittest


ROOT = Path(__file__).parents[1]
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
    def test_fetch_rejects_a_hash_mismatch_before_publish(self):
        fetch = load_script("m6_fetch_cmu_motion")
        with self.assertRaisesRegex(ValueError, "SHA-256"):
            fetch.verify_download(b"wrong", "0" * 64)

    def test_fetch_refuses_off_host_redirects_and_extra_manifest_files(self):
        fetch = load_script("m6_fetch_cmu_motion")
        manifest = fixture_manifest()
        opener = FakeOpener(
            {
                entry["url"]: (Path(ROOT / "tests" / "fixtures" / "m6" / entry["filename"]).read_bytes(), "http://example.com/stolen")
                for entry in manifest["files"]
            }
        )
        with tempfile.TemporaryDirectory() as directory:
            with self.assertRaisesRegex(ValueError, "redirect"):
                fetch.fetch_manifest(manifest, directory, opener=opener, require_fixed_sources=False)
            self.assertEqual(list(Path(directory).iterdir()), [])
        manifest["files"].append(dict(manifest["files"][-1], id="extra", filename="extra.amc"))
        with self.assertRaisesRegex(ValueError, "extra"):
            fetch.validate_manifest(manifest, require_fixed_sources=False)

    def test_ingest_uses_declared_units_world_space_feet_and_fixed_downsampling(self):
        ingest = load_script("m6_cmu_motion_ingest")
        database = ingest.ingest(ASF, [AMC], fixture_manifest())
        clip = database["clips"][0]
        self.assertEqual(clip["samples"][1]["velocity_millimeters_per_second"], [1000, 0])
        self.assertEqual(clip["left_foot_contacts"], [[0, 1]])
        self.assertEqual(clip["right_foot_contacts"], [[0, 1]])
        self.assertEqual([sample["source_frame"] for sample in clip["samples"]], [1, 5, 9])
        self.assertAlmostEqual(clip["samples"][0]["left_foot_position_millimeters"][0], 16.667, places=3)
        self.assertAlmostEqual(clip["samples"][1]["left_foot_position_millimeters"][0], 16.667, places=3)
        self.assertEqual(clip["metrics"]["max_foot_slide_millimeters"], 0)
        self.assertEqual(clip["metrics"]["max_trajectory_deviation_millimeters"], 0)
        self.assertEqual(clip["metrics"]["rejected_frame_rate_ppm"], 0)

    def test_ingest_records_malformed_frames_and_requires_two_retained_samples(self):
        ingest = load_script("m6_cmu_motion_ingest")
        malformed = AMC.read_text().replace("rfoot 0 90 0\n4", "4")
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "cmu-mini.amc"
            path.write_text(malformed)
            manifest = fixture_manifest()
            manifest["files"][1]["sha256"] = hashlib.sha256(path.read_bytes()).hexdigest()
            database = ingest.ingest(ASF, [path], manifest)
            clip = database["clips"][0]
            self.assertEqual(clip["rejected_frames"], [{"source_frame": 3, "reason": "missing bone rfoot"}])
            self.assertEqual(clip["metrics"]["rejected_frame_rate_ppm"], 111112)

            too_short = "\n".join(AMC.read_text().splitlines()[:8]) + "\n"
            path.write_text(too_short)
            manifest["files"][1]["sha256"] = hashlib.sha256(path.read_bytes()).hexdigest()
            with self.assertRaisesRegex(ValueError, "two retained"):
                ingest.ingest(ASF, [path], manifest)

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
                database = ingest.ingest(ASF, [path], manifest)
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

    def test_checked_thresholds_equal_the_dated_observed_baseline(self):
        source = json.loads((ROOT / "assets" / "reference" / "m6" / "cmu-motion-source-v1.json").read_text())
        thresholds = json.loads((ROOT / "assets" / "reference" / "m6" / "motion-thresholds-v1.json").read_text())
        report = json.loads((ROOT / "docs" / "benchmarks" / "2026-08-18-m6-cmu-motion.json").read_text())
        expected_hashes = {entry["id"]: entry["sha256"] for entry in source["files"]}
        self.assertEqual(thresholds["source_hashes"], expected_hashes)
        self.assertEqual(report["source_hashes"], expected_hashes)
        self.assertEqual(set(thresholds["hard_limits"].values()), {0})
        self.assertGreater(report["hard_limit_observations"]["joint_limit_violations"], 0)
        for metric, observed in report["threshold_baseline"].items():
            self.assertEqual(thresholds["soft_limits"][metric], {"baseline": observed, "limit": observed})


if __name__ == "__main__":
    unittest.main()
