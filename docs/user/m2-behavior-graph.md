# M2 behavior graph authoring

The first M2 authoring contract is a typed behavior graph. It is intentionally
bounded: graph nodes compile in Rust before a bake and never invoke Blender
Python inside an agent tick.

## Try the reference graph

1. In Blender, select **Create Reference Concourse**.
2. Open the Text Editor and select `CrowdBehaviorGraphV1`.
3. Return to the Crowd Project panel and select **Validate Behavior Graph**.

The bundled `leave_concourse` preset includes a selector, deterministic
probability branch, sequence, queue action, and navigation actions. The graph
is versioned at [behavior-graph-v1.schema.json](../../schemas/behavior-graph-v1.schema.json).

## Supported node families

- Composition: `selector`, `sequence`, `fallback`, `utility_selector`.
- Finite state: `state_switch` with explicit integer branches and fallback.
- Control: `interrupt`, `timer`, `probability`, `event`.
- Blackboard: `blackboard_compare`.
- Actions: `navigate`, `wait`, `queue`, `follow_lane`, `hold_position`.

Each node needs a unique stable `id`. Composite nodes reference a non-empty
`children` list; one-child control nodes reference `child`.

## Errors and corrections

| Error | Correct it by |
|---|---|
| `E_GRAPH_MissingEntry` | Set `entry_id` to an existing node ID. |
| `E_GRAPH_MissingNode` | Replace the missing child reference or create that node. |
| `E_GRAPH_Cycle` | Remove the cycle; loops are not part of the authoring language. |
| `E_GRAPH_UnreachableNode` | Connect the node from the entry path or remove it. |
| `E_GRAPH_InvalidNode` | Supply the reported required key, target, or positive duration. |

The current M1 bake continues to execute its frozen `commuter_v1` program.
The graph compiler is checked in ahead of attaching graph bytecode to the
simulation hot loop; this avoids claiming authorable runtime behavior before
the remaining M2 semantic, queue, group, and trace acceptance runners exist.
