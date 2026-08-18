#!/usr/bin/env python3
"""Strict deterministic Acclaim ASF/AMC ingestion for the M6 CMU baseline."""

import hashlib
import json
import math
import re
import sys
import urllib.parse
from pathlib import Path


SOURCE_RATE = 120
TARGET_RATE = 30
DOWNSAMPLE_STEP = SOURCE_RATE // TARGET_RATE
SUPPORT_RADIUS = 15
CONTACT_HEIGHT_MM = 45.0
CONTACT_SPEED_MMPS = 120.0


def _identity():
    return ((1.0, 0.0, 0.0), (0.0, 1.0, 0.0), (0.0, 0.0, 1.0))


def _matmul(left, right):
    return tuple(
        tuple(sum(left[row][k] * right[k][column] for k in range(3)) for column in range(3))
        for row in range(3)
    )


def _transpose(matrix):
    return tuple(tuple(matrix[column][row] for column in range(3)) for row in range(3))


def _matvec(matrix, vector):
    return tuple(sum(matrix[row][column] * vector[column] for column in range(3)) for row in range(3))


def _rotation(axis, radians):
    cosine = math.cos(radians)
    sine = math.sin(radians)
    if axis == "X":
        return ((1.0, 0.0, 0.0), (0.0, cosine, -sine), (0.0, sine, cosine))
    if axis == "Y":
        return ((cosine, 0.0, sine), (0.0, 1.0, 0.0), (-sine, 0.0, cosine))
    if axis == "Z":
        return ((cosine, -sine, 0.0), (sine, cosine, 0.0), (0.0, 0.0, 1.0))
    raise ValueError("unsupported rotation axis {}".format(axis))


def _euler(values, order, degrees=True):
    matrix = _identity()
    for axis, value in zip(order, values):
        angle = math.radians(value) if degrees else value
        matrix = _matmul(matrix, _rotation(axis, angle))
    return matrix


def _add(left, right):
    return tuple(left[index] + right[index] for index in range(3))


def _distance_horizontal(left, right):
    return math.hypot(left[0] - right[0], left[2] - right[2])


def _distance(left, right):
    return math.sqrt(sum((left[index] - right[index]) ** 2 for index in range(3)))


def _ceil_metric(value):
    return int(math.ceil(max(0.0, value - 1e-9)))


def _clean_lines(path):
    lines = []
    for raw in Path(path).read_text(encoding="utf-8", errors="strict").splitlines():
        line = raw.split("#", 1)[0].strip()
        if line:
            lines.append(line)
    return lines


def parse_asf(path):
    sections = {}
    current = None
    for line in _clean_lines(path):
        if line.startswith(":"):
            current = line[1:].split()[0].lower()
            sections.setdefault(current, [])
        elif current:
            sections[current].append(line)
    for required in ("units", "root", "bonedata", "hierarchy"):
        if required not in sections:
            raise ValueError("ASF is missing :{}".format(required))

    units = {}
    for line in sections["units"]:
        parts = line.split()
        if len(parts) >= 2:
            units[parts[0].lower()] = parts[1]
    if units.get("angle", "").lower() != "deg":
        raise ValueError("ASF angles must be declared in degrees")
    try:
        length_scale_mm = float(units["length"]) * 25.4
    except (KeyError, ValueError) as error:
        raise ValueError("ASF requires a numeric length unit") from error

    root = {}
    for line in sections["root"]:
        parts = line.split()
        root[parts[0].lower()] = parts[1:]
    order = [value.upper() for value in root.get("order", [])]
    if sorted(order) != sorted(["TX", "TY", "TZ", "RX", "RY", "RZ"]):
        raise ValueError("ASF root order must declare all translation and rotation channels")
    axis = root.get("axis", ["XYZ"])[0].upper()
    if sorted(axis) != ["X", "Y", "Z"]:
        raise ValueError("ASF root axis order is invalid")
    try:
        root_position = tuple(float(value) * length_scale_mm for value in root.get("position", [0, 0, 0]))
        root_orientation = tuple(float(value) for value in root.get("orientation", [0, 0, 0]))
    except ValueError as error:
        raise ValueError("ASF root defaults must be numeric") from error

    bones = {}
    lines = sections["bonedata"]
    index = 0
    while index < len(lines):
        if lines[index].lower() != "begin":
            index += 1
            continue
        index += 1
        block = []
        while index < len(lines) and lines[index].lower() != "end":
            block.append(lines[index])
            index += 1
        if index == len(lines):
            raise ValueError("unterminated ASF bone block")
        index += 1
        fields = {}
        limit_text = ""
        collecting_limits = False
        for line in block:
            parts = line.split()
            key = parts[0].lower()
            if key == "limits":
                collecting_limits = True
                limit_text += " " + line[len(parts[0]):]
            elif collecting_limits and line.startswith("("):
                limit_text += " " + line
            else:
                collecting_limits = False
                fields[key] = parts[1:]
        name = fields.get("name", [None])[0]
        if not name or name in bones:
            raise ValueError("ASF bone names must be unique and non-empty")
        try:
            direction = tuple(float(value) for value in fields["direction"])
            length = float(fields["length"][0]) * length_scale_mm
            axis_values = tuple(float(value) for value in fields.get("axis", [0, 0, 0])[:3])
            axis_order = fields.get("axis", [0, 0, 0, "XYZ"])[3].upper()
        except (KeyError, IndexError, ValueError) as error:
            raise ValueError("ASF bone {} has invalid geometry".format(name)) from error
        dof = [value.lower() for value in fields.get("dof", [])]
        limits = [(float(low), float(high)) for low, high in re.findall(r"\(([-+0-9.eE]+)\s+([-+0-9.eE]+)\)", limit_text)]
        if limits and len(limits) != len(dof):
            raise ValueError("ASF bone {} limits do not match its DOFs".format(name))
        bones[name] = {
            "direction": direction,
            "length_mm": length,
            "axis": axis_values,
            "axis_order": axis_order,
            "dof": dof,
            "limits": limits,
        }

    children = {"root": []}
    parents = {}
    hierarchy_lines = [line for line in sections["hierarchy"] if line.lower() not in ("begin", "end")]
    for line in hierarchy_lines:
        parts = line.split()
        parent = parts[0]
        children.setdefault(parent, [])
        for child in parts[1:]:
            if child in parents:
                raise ValueError("ASF bone {} has multiple parents".format(child))
            parents[child] = parent
            children[parent].append(child)
    if set(parents) != set(bones):
        raise ValueError("ASF hierarchy does not contain every bone exactly once")
    return {
        "length_scale_mm": length_scale_mm,
        "root_order": order,
        "root_axis": axis,
        "root_position": root_position,
        "root_orientation": root_orientation,
        "bones": bones,
        "children": children,
    }


def parse_amc(path, skeleton):
    frames = []
    current = None
    known_bones = {"root", *skeleton["bones"]}
    for line in _clean_lines(path):
        if line.startswith(":"):
            continue
        if re.fullmatch(r"[0-9]+", line):
            current = {"source_frame": int(line), "channels": {}, "errors": []}
            frames.append(current)
            continue
        if current is None:
            raise ValueError("AMC channel data appears before its first frame")
        parts = line.split()
        bone = parts[0]
        if bone not in known_bones:
            current["errors"].append("unknown bone {}".format(bone))
            continue
        if bone in current["channels"]:
            current["errors"].append("duplicate bone {}".format(bone))
            continue
        try:
            values = [float(value) for value in parts[1:]]
        except ValueError:
            current["errors"].append("non-numeric channels for bone {}".format(bone))
            continue
        if any(not math.isfinite(value) for value in values):
            current["errors"].append("non-finite channels for bone {}".format(bone))
            continue
        current["channels"][bone] = values
    if not frames:
        raise ValueError("AMC contains no frames")
    required = {"root": len(skeleton["root_order"])}
    required.update({name: len(bone["dof"]) for name, bone in skeleton["bones"].items() if bone["dof"]})
    valid = []
    rejected = []
    previous = None
    for frame in frames:
        number = frame["source_frame"]
        errors = list(frame["errors"])
        if previous is not None and number <= previous:
            errors.append("non-increasing source frame")
        previous = number
        for bone, channel_count in required.items():
            if bone not in frame["channels"]:
                errors.append("missing bone {}".format(bone))
            elif len(frame["channels"][bone]) != channel_count:
                errors.append("bone {} expected {} channels".format(bone, channel_count))
        if errors:
            rejected.append({"source_frame": number, "reason": errors[0]})
        else:
            valid.append(frame)
    return frames[0]["source_frame"], valid, rejected, len(frames)


def _frame_world(skeleton, frame):
    root_values = dict(zip(skeleton["root_order"], frame["channels"]["root"]))
    translation = tuple(root_values["T" + axis] * skeleton["length_scale_mm"] for axis in "XYZ")
    root_position = _add(skeleton["root_position"], translation)
    motion_angles = [root_values["R" + axis] for axis in skeleton["root_axis"]]
    root_rotation = _matmul(
        _euler(skeleton["root_orientation"], skeleton["root_axis"]),
        _euler(motion_angles, skeleton["root_axis"]),
    )
    endpoints = {}
    rotations = {"root": root_rotation}
    positions = {"root": root_position}
    violations = 0

    def visit(parent):
        nonlocal violations
        parent_rotation = rotations[parent]
        parent_position = positions[parent] if parent == "root" else endpoints[parent]
        for name in skeleton["children"].get(parent, []):
            bone = skeleton["bones"][name]
            values = frame["channels"].get(name, [])
            for value, limits in zip(values, bone["limits"]):
                if value < limits[0] or value > limits[1]:
                    violations += 1
            axis_matrix = _euler(bone["axis"], bone["axis_order"])
            dof_angles = {channel[1].upper(): value for channel, value in zip(bone["dof"], values) if channel.startswith("r")}
            dof_order = "".join(channel[1].upper() for channel in bone["dof"] if channel.startswith("r"))
            dof_matrix = _euler([dof_angles[axis] for axis in dof_order], dof_order) if dof_order else _identity()
            local_rotation = _matmul(_matmul(axis_matrix, dof_matrix), _transpose(axis_matrix))
            rotation = _matmul(parent_rotation, local_rotation)
            rotations[name] = rotation
            positions[name] = parent_position
            offset = tuple(component * bone["length_mm"] for component in bone["direction"])
            endpoints[name] = _add(parent_position, _matvec(rotation, offset))
            visit(name)

    visit("root")
    return root_position, root_rotation, endpoints, violations


def _wrap_angle(angle):
    return (angle + math.pi) % (2.0 * math.pi) - math.pi


def _contact_windows(samples, key):
    positions = [sample[key] for sample in samples]
    speeds = [0.0]
    for index in range(1, len(samples)):
        elapsed = (samples[index]["source_frame"] - samples[index - 1]["source_frame"]) / SOURCE_RATE
        speeds.append(_distance_horizontal(positions[index], positions[index - 1]) / elapsed)
    candidates = []
    for index, position in enumerate(positions):
        start = max(0, index - SUPPORT_RADIUS)
        end = min(len(positions), index + SUPPORT_RADIUS + 1)
        support_minimum = min(candidate[1] for candidate in positions[start:end])
        candidates.append(position[1] <= support_minimum + CONTACT_HEIGHT_MM and speeds[index] <= CONTACT_SPEED_MMPS)
    windows = []
    start = None
    for index, candidate in enumerate(candidates + [False]):
        if candidate and start is None:
            start = index
        elif not candidate and start is not None:
            if index - start >= 2:
                windows.append([start, index - 1])
            start = None
    return windows


def _max_foot_slide(samples, key, windows):
    maximum = 0.0
    for start, end in windows:
        positions = [samples[index][key] for index in range(start, end + 1)]
        for left in range(len(positions)):
            for right in range(left + 1, len(positions)):
                maximum = max(maximum, _distance_horizontal(positions[left], positions[right]))
    return _ceil_metric(maximum)


def _reconstructed_position(source_frame, retained):
    if len(retained) < 2:
        raise ValueError("piecewise-linear reconstruction requires two retained samples")
    right = 1
    while right < len(retained) and retained[right]["source_frame"] < source_frame:
        right += 1
    if right == len(retained):
        right = len(retained) - 1
    left = right - 1
    first = retained[left]
    second = retained[right]
    span = second["source_frame"] - first["source_frame"]
    amount = (source_frame - first["source_frame"]) / span
    return tuple(first["root_position"][axis] + amount * (second["root_position"][axis] - first["root_position"][axis]) for axis in range(3))


def _sha256(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def ingest(asf_path, amc_paths, manifest):
    if manifest.get("schema_version") != 1 or manifest.get("license_id") != "CMU-Mocap-Free-All-Uses":
        raise ValueError("ingestion requires the versioned CMU license manifest")
    if manifest.get("redistribution_allowed") is not False:
        raise ValueError("raw and converted CMU redistribution must remain disabled")
    if manifest.get("source_host") != "mocap.cs.cmu.edu" or urllib.parse.urlsplit(manifest.get("terms_url", "")).hostname != "mocap.cs.cmu.edu":
        raise ValueError("ingestion requires official CMU source and terms hosts")
    if not isinstance(manifest.get("required_attribution"), str) or not manifest["required_attribution"].strip():
        raise ValueError("ingestion requires CMU attribution")
    if manifest.get("source_frame_rate_hz") != SOURCE_RATE or manifest.get("target_frame_rate_hz") != TARGET_RATE:
        raise ValueError("ingestion requires declared 120 Hz source and 30 Hz target rates")
    profile = manifest.get("retarget_profile", {})
    left_foot = profile.get("left_foot_bone")
    right_foot = profile.get("right_foot_bone")
    files = manifest.get("files", [])
    by_filename = {entry.get("filename"): entry for entry in files}
    skeleton_entry = by_filename.get(Path(asf_path).name)
    if not skeleton_entry or skeleton_entry.get("kind") != "skeleton":
        raise ValueError("ASF is not declared by the source manifest")
    if _sha256(asf_path) != skeleton_entry.get("sha256"):
        raise ValueError("ASF source hash drift")
    skeleton = parse_asf(asf_path)
    for foot in (left_foot, right_foot):
        if foot not in skeleton["bones"]:
            raise ValueError("retarget foot bone {} is absent from ASF".format(foot))

    clips = []
    source_hashes = {skeleton_entry["id"]: skeleton_entry["sha256"]}
    for amc_path in amc_paths:
        entry = by_filename.get(Path(amc_path).name)
        if not entry or entry.get("kind") != "motion" or entry.get("skeleton_id") != skeleton_entry["id"]:
            raise ValueError("AMC is not declared for the supplied skeleton")
        actual_hash = _sha256(amc_path)
        if actual_hash != entry.get("sha256"):
            raise ValueError("AMC source hash drift")
        source_hashes[entry["id"]] = actual_hash
        origin, valid_frames, rejected, parsed_count = parse_amc(amc_path, skeleton)
        world_frames = []
        joint_violations = 0
        for frame in valid_frames:
            root_position, root_rotation, endpoints, violations = _frame_world(skeleton, frame)
            forward = _matvec(root_rotation, (0.0, 0.0, 1.0))
            world_frames.append(
                {
                    "source_frame": frame["source_frame"],
                    "root_position": root_position,
                    "facing_radians": math.atan2(forward[0], forward[2]),
                    "left_foot_position": endpoints[left_foot],
                    "right_foot_position": endpoints[right_foot],
                }
            )
            joint_violations += violations
        retained = [frame for frame in world_frames if (frame["source_frame"] - origin) % DOWNSAMPLE_STEP == 0]
        if len(retained) < 2:
            raise ValueError("clip {} has fewer than two retained 30 Hz samples".format(entry["id"]))
        samples = []
        max_root_speed_error = 0.0
        max_turn = 0
        for index, frame in enumerate(retained):
            if index == 0:
                velocity = [0, 0]
                slope = 0
            else:
                previous = retained[index - 1]
                elapsed = (frame["source_frame"] - previous["source_frame"]) / SOURCE_RATE
                exact_velocity = [
                    (frame["root_position"][0] - previous["root_position"][0]) / elapsed,
                    (frame["root_position"][2] - previous["root_position"][2]) / elapsed,
                ]
                velocity = [int(round(component)) for component in exact_velocity]
                max_root_speed_error = max(max_root_speed_error, math.hypot(exact_velocity[0] - velocity[0], exact_velocity[1] - velocity[1]))
                horizontal = _distance_horizontal(frame["root_position"], previous["root_position"])
                vertical = abs(frame["root_position"][1] - previous["root_position"][1])
                slope = int(round(vertical * 1_000_000 / horizontal)) if horizontal else 0
                max_turn = max(max_turn, abs(int(round(_wrap_angle(frame["facing_radians"] - previous["facing_radians"]) * 1_000_000))))
            samples.append(
                {
                    "tick": index,
                    "source_frame": frame["source_frame"],
                    "root_position_millimeters": [round(value, 6) for value in frame["root_position"]],
                    "velocity_millimeters_per_second": velocity,
                    "facing_microradians": int(round(frame["facing_radians"] * 1_000_000)),
                    "left_foot_position_millimeters": [round(value, 6) for value in frame["left_foot_position"]],
                    "right_foot_position_millimeters": [round(value, 6) for value in frame["right_foot_position"]],
                    "contact": "none",
                    "slope_millionths": slope,
                }
            )
        left_windows = _contact_windows(samples, "left_foot_position_millimeters")
        right_windows = _contact_windows(samples, "right_foot_position_millimeters")
        left_ticks = {tick for start, end in left_windows for tick in range(start, end + 1)}
        right_ticks = {tick for start, end in right_windows for tick in range(start, end + 1)}
        for sample in samples:
            left = sample["tick"] in left_ticks
            right = sample["tick"] in right_ticks
            sample["contact"] = "both_feet" if left and right else "left_foot" if left else "right_foot" if right else "none"
        trajectory_deviation = 0.0
        for frame in world_frames:
            reconstructed = _reconstructed_position(frame["source_frame"], retained)
            trajectory_deviation = max(trajectory_deviation, _distance(frame["root_position"], reconstructed))
        rejected_rate = int(math.ceil(len(rejected) * 1_000_000 / parsed_count))
        max_slide = max(
            _max_foot_slide(samples, "left_foot_position_millimeters", left_windows),
            _max_foot_slide(samples, "right_foot_position_millimeters", right_windows),
        )
        clips.append(
            {
                "id": entry.get("clip_id", entry["id"]),
                "source_file": entry["filename"],
                "source_sha256": actual_hash,
                "source_frame_rate_hz": SOURCE_RATE,
                "retained_frame_rate_hz": TARGET_RATE,
                "parsed_frame_count": parsed_count,
                "retained_sample_count": len(samples),
                "rejected_frames": rejected,
                "source_frame_provenance": [sample["source_frame"] for sample in samples],
                "left_foot_contacts": left_windows,
                "right_foot_contacts": right_windows,
                "samples": samples,
                "metrics": {
                    "max_root_speed_error_millimeters_per_second": _ceil_metric(max_root_speed_error),
                    "max_foot_slide_millimeters": max_slide,
                    "max_trajectory_deviation_millimeters": _ceil_metric(trajectory_deviation),
                    "max_turn_discontinuity_microradians": max_turn,
                    "joint_limit_violations": joint_violations,
                    "retarget_failures": 0,
                    "rejected_frames": len(rejected),
                    "parsed_frames": parsed_count,
                    "rejected_frame_rate_ppm": rejected_rate,
                    "root_teleportations": 0,
                    "undeclared_contacts": 0,
                    "source_hash_drift": 0,
                    "cross_cache_mutations": 0,
                },
            }
        )
    return {
        "schema_version": 1,
        "database_id": manifest.get("dataset_id", "cmu-motion-v1"),
        "retarget_profile_id": profile.get("profile_id", "cmu-acclaim-humanoid-v1"),
        "source_provenance": "{}; {}; raw redistribution disabled".format(manifest["license_id"], manifest["required_attribution"]),
        "source_manifest_id": manifest.get("dataset_id"),
        "source_hashes": source_hashes,
        "clips": clips,
    }


def ingest_manifest(manifest, source_dir):
    files = manifest.get("files", [])
    skeletons = [entry for entry in files if entry.get("kind") == "skeleton"]
    databases = []
    for skeleton in skeletons:
        motions = [entry for entry in files if entry.get("kind") == "motion" and entry.get("skeleton_id") == skeleton["id"]]
        databases.append(
            ingest(
                Path(source_dir) / skeleton["filename"],
                [Path(source_dir) / entry["filename"] for entry in motions],
                manifest,
            )
        )
    if not databases:
        raise ValueError("manifest contains no skeletons")
    combined = dict(databases[0])
    combined["clips"] = []
    combined["source_hashes"] = {}
    for database in databases:
        combined["clips"].extend(database["clips"])
        combined["source_hashes"].update(database["source_hashes"])
    combined["clips"].sort(key=lambda clip: clip["id"])
    return combined


def main(argv=None):
    args = list(sys.argv[1:] if argv is None else argv)
    if len(args) != 3:
        print("usage: m6_cmu_motion_ingest.py MANIFEST.json SOURCE_DIR DATABASE.json", file=sys.stderr)
        return 2
    try:
        manifest = json.loads(Path(args[0]).read_text(encoding="utf-8"))
        database = ingest_manifest(manifest, args[1])
        output = Path(args[2])
        output.parent.mkdir(parents=True, exist_ok=True)
        temporary = output.with_suffix(output.suffix + ".tmp")
        temporary.write_text(json.dumps(database, indent=2, sort_keys=True) + "\n", encoding="utf-8")
        temporary.replace(output)
    except (OSError, TypeError, ValueError, json.JSONDecodeError) as error:
        print(str(error), file=sys.stderr)
        return 1
    print("wrote {}".format(args[2]))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
