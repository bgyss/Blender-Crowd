"""M5 scale and profiling: preflight estimates and measured evidence.

Two kinds of number appear in the M5 scale panel and they must never be
confused. An *estimate* is computed here from the declared population and
cache shape before anything is baked, so an artist can decide whether a shot
is affordable. A *measurement* is read back from a scale report produced by
`crowd-bench`, and is only ever as good as the run that produced it.

Every function below returns one or the other, labelled, and the panel prints
them under separate headings. Nothing here infers a measurement from an
estimate or vice versa.

Aggregates only, by design. The M5 UI gate forbids listing or drawing every
agent at 10K or 100K, so tier information is summarised into counts and
shares; drill-down to an individual agent goes through the existing agent
inspection path, one agent at a time.
"""

import json
from pathlib import Path


#: Report schema that first carried `metrics.per_tier`. An older report has no
#: per-tier evidence and is rejected rather than partially displayed.
MINIMUM_REPORT_SCHEMA = 5

#: Bytes per agent per cached frame in the F32 codec the cache experiment
#: selected: position (2), orientation, scale, phase, playback rate as f32,
#: plus the u64 stable ID and the small integer channels. Used for the cache
#: estimate only; the measured size comes from the cache report.
_ESTIMATED_CACHE_BYTES_PER_AGENT_FRAME = 48

#: Resident bytes per agent held by the simulation's columnar world. An
#: estimate from the column set, not a measured RSS figure.
_ESTIMATED_MEMORY_BYTES_PER_AGENT = 320

#: Order the panel lists tiers in, so a missing tier reads as absent rather
#: than reordering everything after it.
SIMULATION_TIERS = ("S0", "S1", "S2", "S3")
RENDER_TIERS = ("R0", "R1", "R2", "R3", "R4")

#: Simulation tier each render tier is paired with by `render_for`.
_RENDER_FOR = {"S0": "R0", "S1": "R1", "S2": "R2", "S3": "R3"}


def _format_bytes(count):
    value = float(count)
    for unit in ("B", "KiB", "MiB", "GiB"):
        if value < 1024.0 or unit == "GiB":
            return "{:.1f} {}".format(value, unit)
        value /= 1024.0
    raise AssertionError("unreachable: the GiB branch always returns")


def estimate(agent_count, cached_frames):
    """Preflight estimates for a declared population. Never a measurement."""
    if agent_count <= 0 or cached_frames <= 0:
        raise ValueError("estimates need a positive agent count and frame count")
    cache_bytes = agent_count * cached_frames * _ESTIMATED_CACHE_BYTES_PER_AGENT_FRAME
    memory_bytes = agent_count * _ESTIMATED_MEMORY_BYTES_PER_AGENT
    return {
        "kind": "estimate",
        "agent_count": agent_count,
        "cached_frames": cached_frames,
        "memory": "~{} resident (estimate)".format(_format_bytes(memory_bytes)),
        "cache": "~{} for {} frames (estimate)".format(_format_bytes(cache_bytes), cached_frames),
        # Extraction is procedural: one object carrying N points, so the cost
        # tracked here is the per-frame attribute write, not object creation.
        "extraction": "~{} attribute bytes per frame, 1 object (estimate)".format(
            _format_bytes(agent_count * _ESTIMATED_CACHE_BYTES_PER_AGENT_FRAME)
        ),
    }


def load_report(path):
    """Read a `crowd-bench run` scale report, or say why it cannot be used."""
    with Path(path).open(encoding="utf-8") as handle:
        report = json.load(handle)
    schema = report.get("schema_version", 0)
    if schema < MINIMUM_REPORT_SCHEMA:
        raise ValueError(
            "report schema {} predates per-tier metrics; rerun the scale "
            "measurement with a current build".format(schema)
        )
    if not report.get("metrics", {}).get("per_tier"):
        raise ValueError("report carries no per-tier metrics; it cannot support an M5 tier claim")
    return report


def load_adjudication(path):
    """Read an `crowd-bench m5-gate` adjudication beside a report."""
    with Path(path).open(encoding="utf-8") as handle:
        adjudication = json.load(handle)
    if "checks" not in adjudication:
        raise ValueError("not an M5 gate adjudication: no checks recorded")
    return adjudication


def declared_tier_counts(report):
    """Committed S and R populations, from what ran rather than what was asked for."""
    simulation = {name: 0 for name in SIMULATION_TIERS}
    render = {name: 0 for name in RENDER_TIERS}
    for tier in report["metrics"]["per_tier"]:
        name = tier["tier"]
        if name not in simulation:
            raise ValueError("report names an unknown simulation tier: {}".format(name))
        simulation[name] = tier["agents_final"]
        render[_RENDER_FOR[name]] = tier["agents_final"]
    return {"simulation": simulation, "render": render}


def measured_summary(report):
    """One line of measured headline numbers, with the machine that produced them."""
    metrics = report["metrics"]
    environment = report.get("environment", {})
    return "{} agents, {:.1f} ticks/s, {:.0f}% arrived, {} on {}".format(
        metrics["agents_spawned"],
        metrics["ticks_per_second_achieved"],
        metrics["completion_rate"] * 100.0,
        report.get("solver", "unknown solver"),
        environment.get("cpu", "unknown hardware"),
    )


def bottleneck(report):
    """The dominant measured phase, which is what a profiling view has to name.

    Reported as a share of measured phase time rather than a frame budget: the
    scale runner is not frame-locked, so a percentage of where time actually
    went is the honest statement.
    """
    shares = report["metrics"].get("phase_time_shares") or []
    if not shares:
        return "No phase timings in this report"
    dominant = max(shares, key=lambda entry: entry["nanos"])
    return "{} phase, {:.1f}% of measured phase time".format(
        dominant["phase"], dominant["share"] * 100.0
    )


def animation_scheduling_summary(report):
    """What camera/focus animation scheduling actually saved, per tier."""
    lines = []
    for tier in report["metrics"]["per_tier"]:
        share = tier.get("animation_evaluation_share")
        if share is None:
            continue
        lines.append("{}: {:.0f}% of classifications evaluated".format(tier["tier"], share * 100.0))
    return "; ".join(lines) if lines else "No animation scheduling recorded"


def gate_result(adjudication):
    """Pass/fail plus the failing check names, never a bare boolean."""
    failures = [
        "{}.{}".format(check["tier"], check["name"]) if check.get("tier") else check["name"]
        for check in adjudication["checks"]
        if not check["passed"]
    ]
    if adjudication["passed"]:
        return "PASS ({} checks)".format(len(adjudication["checks"]))
    return "FAIL: {}".format(", ".join(failures))


def playback_tier_histogram(playback):
    """Aggregate the attached cache's render tiers at the current frame.

    Reads the point-cloud attributes the cache already publishes, so the cost
    is one pass over two attribute arrays rather than per-agent object
    inspection — which is the whole point of the procedural path at 10K and
    100K. No agent is listed or named.

    Only agents present in the cache at this tick are tiered. An agent that has
    not yet emitted, or has already reached its destination and left, has no
    committed tier at this tick; it is counted under `not_present` rather than
    being reported as a full-character R0, which would badly overstate the
    render cost of a staggered population.
    """
    attributes = playback.object.data.attributes
    tier_attribute = attributes.get("crowd_render_tier")
    visible_attribute = attributes.get("crowd_visible")
    if tier_attribute is None or visible_attribute is None:
        raise ValueError(
            "the attached playback object carries no crowd_render_tier/crowd_visible attributes"
        )
    tiers = {name: 0 for name in RENDER_TIERS}
    not_present = 0
    for tier_element, visible_element in zip(tier_attribute.data, visible_attribute.data):
        if not int(visible_element.value):
            not_present += 1
            continue
        index = int(tier_element.value)
        if index < 0 or index >= len(RENDER_TIERS):
            raise ValueError("cache contains an out-of-range render tier: {}".format(index))
        tiers[RENDER_TIERS[index]] += 1
    return {"tiers": tiers, "not_present": not_present}
