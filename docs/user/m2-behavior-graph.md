# M2 behavior graph authoring

The first M2 authoring contract is a typed behavior graph. It is intentionally
bounded: graph nodes compile in Rust before a bake and never invoke Blender
Python inside an agent tick.

## Try the reference graph

1. In Blender, select **Create Reference Concourse**.
2. Open a Geometry Node Editor and choose **Crowd Behavior Graph** from the
   node-tree selector. Each node has a bounded type, stable ID, and typed JSON
   fields; links define composite children.
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

Each node needs a unique stable `id`. Composite nodes derive their non-empty
`children` list from graph links; one-child control-node fields remain explicit
typed fields. The optional `CrowdBehaviorGraphV1` text data block remains a
portable fallback, but the node tree takes precedence at compile time.

## Errors and corrections

| Error | Correct it by |
|---|---|
| `E_GRAPH_MissingEntry` | Set `entry_id` to an existing node ID. |
| `E_GRAPH_MissingNode` | Replace the missing child reference or create that node. |
| `E_GRAPH_Cycle` | Remove the cycle; loops are not part of the authoring language. |
| `E_GRAPH_UnreachableNode` | Connect the node from the entry path or remove it. |
| `E_GRAPH_InvalidNode` | Supply the reported required key, target, or positive duration. |

The **Bake Crowd Cache** operator compiles this graph and the editable M2
queue/lane/cost-region properties into an authorable native session. Its graph
decisions and queue/group evidence are saved in the cache sidecar, so selected
agent inspection remains available after reload. The remaining M2 acceptance
runners still cover the full 1,000-agent reference shot and terrain/foot
presentation.
