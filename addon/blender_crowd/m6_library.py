"""Declarative, bounded M6 action/subgraph/preset library helpers.

This module deliberately produces graph data only.  Blender renders it through
the existing typed behavior editor and Rust remains the sole compiler/runtime
authority; no callback, script, or source-code field is accepted here.
"""

import copy


SCHEMA_VERSION = 1
PARAMETER_TYPES = {"bool", "number_i32", "string", "stable_id"}
NODE_TYPES = {
    "selector",
    "sequence",
    "fallback",
    "utility_selector",
    "state_switch",
    "interrupt",
    "timer",
    "probability",
    "event",
    "blackboard_compare",
    "navigate",
    "wait",
    "queue",
    "follow_lane",
    "hold_position",
}
_FORBIDDEN_FIELD_NAMES = {"callback", "runtime_callback", "source_code", "source-code", "script"}


def validate_library(value):
    """Validate a version-1 declarative library and return it unchanged."""
    _reject_forbidden_fields(value)
    _require_mapping(value, "library")
    _require_exact_keys(value, {"schema_version", "id", "actions", "subgraphs", "presets"}, "library")
    if value["schema_version"] != SCHEMA_VERSION:
        raise ValueError("unsupported brain library version")
    _require_id(value["id"], "library ID")

    actions = _validate_actions(value["actions"])
    subgraphs = _validate_subgraphs(value["subgraphs"], actions)
    _validate_presets(value["presets"], subgraphs)
    return value


def instantiate_preset(value, preset_id, instance_id, parameters):
    """Instantiate one checked preset as a deterministic namespaced graph."""
    validate_library(value)
    _require_id(preset_id, "preset ID")
    if not isinstance(instance_id, str) or not instance_id:
        raise ValueError("instance ID must be a non-empty unnamespaced string")
    if "::" in instance_id:
        raise ValueError("instance ID would create a namespace collision")
    _require_mapping(parameters, "parameters")

    preset = _by_id(value["presets"], preset_id, "preset")
    subgraph = _by_id(value["subgraphs"], preset["subgraph_id"], "subgraph")
    declared_parameters = _parameter_map(subgraph["parameters"], "subgraph {}".format(subgraph["id"]))
    resolved_parameters = dict(preset["parameters"])
    for key, parameter_value in parameters.items():
        if key not in declared_parameters:
            raise ValueError("unknown parameter {}".format(key))
        resolved_parameters[key] = parameter_value
    _validate_parameter_values(resolved_parameters, declared_parameters, "preset {}".format(preset_id))

    actions_by_id = {action["id"]: action for action in value["actions"]}
    action_ids = {
        node["action_id"]
        for node in subgraph["nodes"]
        if "action_id" in node
    }
    namespaced_actions = []
    for action_id in sorted(action_ids):
        action = copy.deepcopy(actions_by_id[action_id])
        action["id"] = _namespace(instance_id, action_id)
        namespaced_actions.append(action)

    namespaced_nodes = []
    for node in subgraph["nodes"]:
        emitted = copy.deepcopy(node)
        emitted["id"] = _namespace(instance_id, node["id"])
        if "children" in emitted:
            emitted["children"] = [_namespace(instance_id, child) for child in emitted["children"]]
        if "action_id" in emitted:
            emitted["action_id"] = _namespace(instance_id, emitted["action_id"])
        if "parameters" in emitted:
            emitted["parameters"] = {
                key: _substitute_parameter(parameter_value, resolved_parameters)
                for key, parameter_value in emitted["parameters"].items()
            }
        namespaced_nodes.append(emitted)

    namespaced_nodes.sort(key=lambda node: node["id"])
    namespaced_actions.sort(key=lambda action: action["id"])
    return {
        "schema_version": SCHEMA_VERSION,
        "id": "{}::{}::{}".format(value["id"], preset_id, instance_id),
        "entry_id": _namespace(instance_id, subgraph["entry_id"]),
        "actions": namespaced_actions,
        "nodes": namespaced_nodes,
    }


def _validate_actions(actions):
    _require_list(actions, "actions")
    by_id = {}
    for action in actions:
        _require_mapping(action, "action")
        _require_exact_keys(action, {"id", "channel", "parameters"}, "action")
        _require_id(action["id"], "action ID")
        _require_id(action["channel"], "action channel")
        if action["id"] in by_id:
            raise ValueError("duplicate action ID {}".format(action["id"]))
        _validate_parameters(action["parameters"], "action {}".format(action["id"]))
        by_id[action["id"]] = action
    return by_id


def _validate_subgraphs(subgraphs, actions):
    _require_list(subgraphs, "subgraphs")
    by_id = {}
    for subgraph in subgraphs:
        _require_mapping(subgraph, "subgraph")
        _require_exact_keys(subgraph, {"id", "entry_id", "parameters", "nodes"}, "subgraph")
        _require_id(subgraph["id"], "subgraph ID")
        _require_id(subgraph["entry_id"], "subgraph entry ID")
        if subgraph["id"] in by_id:
            raise ValueError("duplicate subgraph ID {}".format(subgraph["id"]))
        parameters = _parameter_map(
            _validate_parameters(subgraph["parameters"], "subgraph {}".format(subgraph["id"])),
            "subgraph {}".format(subgraph["id"]),
        )
        _validate_nodes(subgraph["nodes"], subgraph["entry_id"], actions, parameters, subgraph["id"])
        by_id[subgraph["id"]] = subgraph
    return by_id


def _validate_presets(presets, subgraphs):
    _require_list(presets, "presets")
    seen = set()
    for preset in presets:
        _require_mapping(preset, "preset")
        _require_exact_keys(preset, {"id", "subgraph_id", "parameters"}, "preset")
        _require_id(preset["id"], "preset ID")
        _require_id(preset["subgraph_id"], "preset subgraph ID")
        if preset["id"] in seen:
            raise ValueError("duplicate preset ID {}".format(preset["id"]))
        seen.add(preset["id"])
        subgraph = subgraphs.get(preset["subgraph_id"])
        if subgraph is None:
            raise ValueError("preset {} references missing subgraph {}".format(preset["id"], preset["subgraph_id"]))
        parameters = _parameter_map(subgraph["parameters"], "subgraph {}".format(subgraph["id"]))
        _validate_parameter_values(preset["parameters"], parameters, "preset {}".format(preset["id"]))


def _validate_nodes(nodes, entry_id, actions, subgraph_parameters, subgraph_id):
    _require_list(nodes, "subgraph nodes")
    by_id = {}
    for node in nodes:
        _require_mapping(node, "node")
        allowed = {"id", "type", "children", "action_id", "parameters"}
        _require_known_keys(node, allowed, "node")
        _require_id(node.get("id"), "node ID")
        if node["id"] in by_id:
            raise ValueError("duplicate node ID {}".format(node["id"]))
        if node.get("type") not in NODE_TYPES:
            raise ValueError("unsupported node type {}".format(node.get("type")))
        if "children" in node:
            _require_list(node["children"], "node children")
            for child in node["children"]:
                _require_id(child, "node child ID")
        if "action_id" in node:
            action = actions.get(node["action_id"])
            if action is None:
                raise ValueError("node {} references missing action {}".format(node["id"], node["action_id"]))
            _validate_node_action_parameters(node, action, subgraph_parameters)
        elif "parameters" in node:
            raise ValueError("node {} declares parameters without an action".format(node["id"]))
        by_id[node["id"]] = node
    if entry_id not in by_id:
        raise ValueError("subgraph {} references missing entry node {}".format(subgraph_id, entry_id))
    for node in by_id.values():
        for child in node.get("children", []):
            if child not in by_id:
                raise ValueError("node {} references missing child {}".format(node["id"], child))


def _validate_node_action_parameters(node, action, subgraph_parameters):
    action_parameters = _parameter_map(action["parameters"], "action {}".format(action["id"]))
    node_parameters = node.get("parameters", {})
    _require_mapping(node_parameters, "node parameters")
    for name, parameter_value in node_parameters.items():
        if name not in action_parameters:
            raise ValueError("node {} uses unknown action parameter {}".format(node["id"], name))
        if isinstance(parameter_value, str) and parameter_value.startswith("$"):
            source_name = parameter_value[1:]
            source = subgraph_parameters.get(source_name)
            if source is None:
                raise ValueError("node {} references undeclared parameter {}".format(node["id"], source_name))
            if source["type"] != action_parameters[name]["type"]:
                raise ValueError("node {} parameter {} has incompatible type".format(node["id"], name))
        elif not _value_has_type(parameter_value, action_parameters[name]["type"]):
            raise ValueError("node {} parameter {} has invalid type".format(node["id"], name))


def _validate_parameters(parameters, owner):
    _require_list(parameters, "{} parameters".format(owner))
    seen = set()
    for parameter in parameters:
        _require_mapping(parameter, "parameter")
        _require_exact_keys(parameter, {"id", "type"}, "parameter")
        _require_id(parameter["id"], "parameter ID")
        if parameter["id"] in seen:
            raise ValueError("duplicate parameter ID {}".format(parameter["id"]))
        if parameter["type"] not in PARAMETER_TYPES:
            raise ValueError("unsupported parameter type {}".format(parameter["type"]))
        seen.add(parameter["id"])
    return parameters


def _parameter_map(parameters, owner):
    _validate_parameters(parameters, owner)
    return {parameter["id"]: parameter for parameter in parameters}


def _validate_parameter_values(values, declared, owner):
    _require_mapping(values, "{} parameters".format(owner))
    for name, parameter_value in values.items():
        declaration = declared.get(name)
        if declaration is None:
            raise ValueError("unknown parameter {}".format(name))
        if not _value_has_type(parameter_value, declaration["type"]):
            raise ValueError("parameter {} has invalid type".format(name))
    missing = sorted(set(declared) - set(values))
    if missing:
        raise ValueError("missing parameter {}".format(missing[0]))


def _value_has_type(value, parameter_type):
    if parameter_type == "bool":
        return isinstance(value, bool)
    if parameter_type == "number_i32":
        return isinstance(value, int) and not isinstance(value, bool) and -(1 << 31) <= value < (1 << 31)
    return isinstance(value, str) and bool(value)


def _substitute_parameter(value, parameters):
    if isinstance(value, str) and value.startswith("$"):
        return parameters[value[1:]]
    return value


def _namespace(instance_id, identifier):
    return "{}::{}".format(instance_id, identifier)


def _by_id(values, identifier, kind):
    for value in values:
        if value["id"] == identifier:
            return value
    raise ValueError("unknown {} {}".format(kind, identifier))


def _reject_forbidden_fields(value):
    if isinstance(value, dict):
        for key, nested in value.items():
            if str(key).lower() in _FORBIDDEN_FIELD_NAMES:
                raise ValueError("source-code or runtime callback fields are not supported")
            _reject_forbidden_fields(nested)
    elif isinstance(value, list):
        for nested in value:
            _reject_forbidden_fields(nested)


def _require_mapping(value, description):
    if not isinstance(value, dict):
        raise ValueError("{} must be an object".format(description))


def _require_list(value, description):
    if not isinstance(value, list):
        raise ValueError("{} must be a list".format(description))


def _require_id(value, description):
    if not isinstance(value, str) or not value or "::" in value:
        raise ValueError("{} must be a non-empty unnamespaced string".format(description))


def _require_exact_keys(value, required, description):
    _require_known_keys(value, required, description)
    missing = required - set(value)
    if missing:
        raise ValueError("{} is missing {}".format(description, sorted(missing)[0]))


def _require_known_keys(value, allowed, description):
    unknown = set(value) - allowed
    if unknown:
        raise ValueError("{} has unsupported field {}".format(description, sorted(unknown)[0]))
