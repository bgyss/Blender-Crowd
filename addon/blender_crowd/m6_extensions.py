"""Coarse Python facade for the versioned M6 extension boundary.

Extensions exchange declared data channels and receive an explicit fallback;
they never receive a callback into the per-agent simulation hot loop.
"""


SCHEMA_VERSION = 1


def validate_manifest(manifest):
    if not isinstance(manifest, dict):
        raise ValueError("extension manifest must be an object")
    if manifest.get("schema_version") != SCHEMA_VERSION:
        raise ValueError("unsupported extension schema version")
    if not manifest.get("id"):
        raise ValueError("extension manifest ID must be non-empty")
    channels = manifest.get("channels")
    if not isinstance(channels, list):
        raise ValueError("extension channels must be a list")
    names = set()
    for channel in channels:
        if not isinstance(channel, dict) or not channel.get("name"):
            raise ValueError("extension channel name must be non-empty")
        if channel["name"] in names:
            raise ValueError("duplicate extension channel {}".format(channel["name"]))
        names.add(channel["name"])
        if int(channel.get("version", 0)) <= 0:
            raise ValueError("extension channel version must be positive")
        if not channel.get("inputs") or not channel.get("outputs"):
            raise ValueError("extension channel inputs and outputs are required")
        if int(channel.get("cost_budget_millionths", 0)) <= 0:
            raise ValueError("extension channel cost budget must be positive")
        if not channel.get("deterministic"):
            raise ValueError("extension channel must be deterministic")
        if not channel.get("failure_isolated"):
            raise ValueError("extension channel must be failure isolated")
    return manifest


def validate_call(manifest, channel_name, inputs, estimated_cost_millionths):
    validate_manifest(manifest)
    channel = next(
        (item for item in manifest["channels"] if item["name"] == channel_name),
        None,
    )
    if channel is None:
        raise ValueError("unknown extension channel {}".format(channel_name))
    undeclared = sorted(set(inputs) - set(channel["inputs"]))
    if undeclared:
        raise ValueError("undeclared extension input {}".format(undeclared[0]))
    if int(estimated_cost_millionths) > int(channel["cost_budget_millionths"]):
        raise ValueError("extension cost budget exceeded")
    return channel


def run_isolated(manifest, channel_name, inputs, estimated_cost_millionths, operation, fallback):
    channel = validate_call(manifest, channel_name, inputs, estimated_cost_millionths)
    try:
        return {"status": "accepted", "value": operation(), "channel_version": channel["version"]}
    except Exception as error:  # noqa: BLE001 - the boundary must isolate extension failures.
        return {"status": "fallback", "value": fallback, "reason": str(error)}
