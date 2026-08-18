"""M5 Blender playback, render, and scale/profiling UI proof.

Covers the non-simulation half of the M5 10K gate, which the Rust scale report
cannot speak to:

- A cache baked at the declared M5 tier mix carries that mix, and playback
  reads it back as an aggregate rather than as a per-agent list.
- The populated cache stays procedural: one attached object and Geometry Nodes
  instances, not one Blender object per agent. This is the property the 100K
  gate turns on, proved here at the 10K gate's own scale.
- A frame renders from that playback.
- The scale and profiling panel is populated from a measured report and its
  gate adjudication, and keeps estimates visibly separate from measurements.

Run through `scripts/m5-blender-test.sh`, which builds the native wheel first.
"""

import json
import os
import sys
import tempfile
import time

import addon_utils
import bpy


EXTENSION = "bl_ext.user_default.blender_crowd"

#: Large enough that a per-agent-object implementation would be obvious, and
#: small enough to bake inside a test. The 10K and 100K claims rest on the
#: procedural path this asserts, not on this population.
#: The reference concourse's own population. Set M5_BLENDER_AGENTS to raise it
#: — 10000 for the 10K gate's viewport/playback evidence — which rewrites the
#: project's population count before compiling rather than asking the session
#: for more agents than the project declares.
AGENTS = int(os.environ.get("M5_BLENDER_AGENTS", "1000"))
#: Long enough for the reference concourse to actually emit its full
#: population. A short bake leaves most agents unspawned, and the procedural
#: instance assertion below would then pass or fail on emission timing rather
#: than on whether playback stayed procedural.
BAKE_TICKS = int(os.environ.get("M5_BLENDER_TICKS", "5000"))
DECLARED_PROFILE = "m5_background_10_90"


def fail(message):
    print("FAIL: {}".format(message))
    sys.exit(1)


def require(condition, message):
    if not condition:
        fail(message)


def main():
    if os.environ.get("CROWD_SOURCE_ADDON"):
        from addon import blender_crowd

        blender_crowd.register()
        from addon.blender_crowd import m5_scale, operators, project, render_workflow
    else:
        addon_utils.enable(EXTENSION, default_set=True)
        from bl_ext.user_default.blender_crowd import m5_scale, operators, project, render_workflow
    import blender_crowd_native

    scene = bpy.context.scene
    props = scene.crowd_project
    require(bpy.ops.crowd.create_reference_project() == {"FINISHED"}, "reference project failed")
    if AGENTS != 1000:
        # Scale the authored population rather than the session request: a
        # session cannot exceed the agents its project actually declares.
        declared = sum(population.count for population in props.populations)
        require(declared > 0, "the reference project declared no populations")
        remaining = AGENTS
        for index, population in enumerate(props.populations):
            share = AGENTS * population.count // declared
            if index == len(props.populations) - 1:
                share = remaining
            population.count = max(1, share)
            remaining -= population.count
    scene.frame_end = scene.frame_start + BAKE_TICKS - 1

    artifacts = os.environ.get("M5_ARTIFACT_DIR") or tempfile.mkdtemp(prefix="blender-crowd-m5-")
    os.makedirs(artifacts, exist_ok=True)

    # --- Bake at the declared M5 tier mix -------------------------------
    cache_dir = os.path.join(tempfile.mkdtemp(prefix="blender-crowd-m5-cache-"), "cache")
    compiled = blender_crowd_native.compile_project(json.dumps(project.extract_ir(scene)))
    session = compiled.create_session(agent_count=AGENTS, fidelity_profile=DECLARED_PROFILE)
    bake_started = time.perf_counter()
    outcome = session.bake(cache_dir, BAKE_TICKS, blender_crowd_native.CancelToken())
    bake_seconds = time.perf_counter() - bake_started
    require(outcome["status"] == "complete", "M5 declared-profile cache did not bake")

    # An undeclared profile must not silently become a default mix.
    try:
        compiled.create_session(agent_count=AGENTS, fidelity_profile="90_percent_background")
        fail("an unknown fidelity profile was accepted")
    except ValueError as error:
        require("E_FIDELITY_PROFILE" in str(error), "unknown profile raised the wrong error")

    # --- Playback stays procedural --------------------------------------
    objects_before = len(scene.objects)
    playback = operators.attach_cache_path(scene, cache_dir)
    objects_after = len(scene.objects)
    require(playback.agent_count == AGENTS, "M5 cache did not attach the declared population")
    require(
        objects_after - objects_before < 10,
        "attaching {} agents added {} persistent objects".format(
            AGENTS, objects_after - objects_before
        ),
    )

    scene.frame_set(scene.frame_end)
    bpy.context.view_layer.update()

    # --- The declared mix survives into the cache -----------------------
    summary = m5_scale.playback_tier_histogram(playback)
    histogram = summary["tiers"]
    require(
        sum(histogram.values()) + summary["not_present"] == AGENTS,
        "tier histogram covered {} of {} agents".format(
            sum(histogram.values()) + summary["not_present"], AGENTS
        ),
    )
    drawn = histogram["R1"] + histogram["R2"]
    require(drawn > 0, "no agent was present at the inspected tick")

    # Every agent in the cache lives as a point on the one attached object.
    # This is the property the 100K gate turns on, and it is about the whole
    # population rather than the subset visible at any one tick.
    require(
        len(playback.object.data.attributes["crowd_render_tier"].data) == AGENTS,
        "the attached object carries {} of {} agents as data".format(
            len(playback.object.data.attributes["crowd_render_tier"].data), AGENTS
        ),
    )

    depsgraph = bpy.context.evaluated_depsgraph_get()
    instances = sum(1 for item in depsgraph.object_instances if item.is_instance)
    # Compared against the agents actually present at this tick, not against
    # the whole population: the reference concourse emits over time, so a
    # fraction of the cache is en route at any given frame. Anchoring to the
    # population instead would make this assert the emission schedule rather
    # than that playback stayed procedural.
    require(
        instances >= drawn,
        "populated M5 cache evaluated {} procedural instances for {} present agents".format(
            instances, drawn
        ),
    )
    # The declared mix targets 10% R1 / 90% R2 of the agents actually present.
    # A stable hash partitioning a finite population lands near that, not
    # exactly on it, so this checks the band the profile declares rather than
    # an exact count.
    background_share = histogram["R2"] / float(drawn)
    require(
        0.85 <= background_share <= 0.95,
        "cache background share {:.3f} is outside the declared 90% target".format(background_share),
    )
    require(histogram["R1"] > 0, "the declared mix committed no midground agents")
    require(
        histogram["R0"] == 0,
        "the declared background mix must not draw any agent at full-character R0",
    )
    require(bpy.ops.crowd.summarize_m5_playback() == {"FINISHED"}, "playback tier summary failed")
    require("R2" in props.m5_playback_tiers, "playback summary did not report the background tier")

    # --- Render ----------------------------------------------------------
    render_workflow.configure_reference_scene(scene)
    scene.render.engine = render_workflow._eevee_engine_identifier(scene)
    capture = os.path.join(artifacts, "m5-procedural-playback.png")
    scene.render.filepath = capture
    render_started = time.perf_counter()
    bpy.ops.render.render(write_still=True)
    render_seconds = time.perf_counter() - render_started
    require(
        os.path.isfile(capture) and os.path.getsize(capture) > 0,
        "M5 playback render was not written",
    )

    # --- Preflight estimates ---------------------------------------------
    props.m5_s1_count = histogram["R1"]
    props.m5_s2_count = histogram["R2"]
    require(bpy.ops.crowd.estimate_m5_preflight() == {"FINISHED"}, "M5 preflight estimate failed")
    for field in ("m5_estimated_memory", "m5_estimated_cache", "m5_estimated_extract"):
        value = getattr(props, field)
        require("estimate" in value, "{} did not label itself an estimate: {}".format(field, value))
    require(
        props.m5_measured_summary == "No scale measurement attached",
        "estimating overwrote the measured field: {}".format(props.m5_measured_summary),
    )

    # --- Measured evidence ------------------------------------------------
    report_path = os.environ.get("M5_REPORT")
    require(bool(report_path), "set M5_REPORT to a crowd-bench scale report")
    require(os.path.isfile(report_path), "M5_REPORT does not exist: {}".format(report_path))
    props.m5_report_path = report_path
    adjudication_path = os.environ.get("M5_ADJUDICATION", "")
    props.m5_adjudication_path = adjudication_path

    require(bpy.ops.crowd.load_m5_report() == {"FINISHED"}, "attaching the M5 scale report failed")
    require(
        props.m5_measured_summary != "No scale measurement attached",
        "the measured summary stayed empty after attaching a report",
    )
    require("ticks/s" in props.m5_measured_summary, "measured summary omitted throughput")
    require("phase" in props.m5_bottleneck, "the profiling view named no bottleneck")
    require(
        "evaluated" in props.m5_animation_scheduling,
        "animation scheduling evidence was not surfaced: {}".format(props.m5_animation_scheduling),
    )
    if adjudication_path:
        require(
            props.m5_gate_result.startswith("PASS") or props.m5_gate_result.startswith("FAIL"),
            "gate result was not adjudicated: {}".format(props.m5_gate_result),
        )
    else:
        require(
            "Not adjudicated" in props.m5_gate_result,
            "an unadjudicated report must not read as a passing gate",
        )

    # A report predating per-tier metrics must be refused, not partly shown.
    stale = os.path.join(artifacts, "stale-report.json")
    with open(report_path, encoding="utf-8") as handle:
        report = json.load(handle)
    report["schema_version"] = 4
    report["metrics"].pop("per_tier", None)
    with open(stale, "w", encoding="utf-8") as handle:
        json.dump(report, handle)
    props.m5_report_path = stale
    # An operator that reports an ERROR raises through `bpy.ops` rather than
    # returning CANCELLED, so the refusal is caught here rather than compared.
    try:
        bpy.ops.crowd.load_m5_report()
        fail("a report without per-tier metrics was accepted as M5 tier evidence")
    except RuntimeError as error:
        require(
            "per-tier" in str(error),
            "the refusal did not explain the missing per-tier evidence: {}".format(error),
        )
    props.m5_report_path = report_path

    print(
        "M5 Blender scale: PASS {}".format(
            json.dumps(
                {
                    "agents": AGENTS,
                    "declared_profile": DECLARED_PROFILE,
                    "render_tier_histogram": histogram,
                    "not_present_at_tick": summary["not_present"],
                    "procedural_instances": instances,
                    "present_at_tick": drawn,
                    "scene_objects_added_by_attach": objects_after - objects_before,
                    "bake_seconds": round(bake_seconds, 3),
                    "render_seconds": round(render_seconds, 3),
                    "capture": capture,
                    "gate_result": props.m5_gate_result,
                    "bottleneck": props.m5_bottleneck,
                    "animation_scheduling": props.m5_animation_scheduling,
                },
                sort_keys=True,
            )
        )
    )
    bpy.ops.wm.quit_blender()


try:
    main()
except SystemExit:
    raise
except Exception as error:
    fail("unexpected {}: {}".format(type(error).__name__, error))
