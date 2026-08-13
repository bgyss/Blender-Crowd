# M5 backend support matrix

This matrix is the authoritative support boundary for the M5 spatial-field
kernel. A backend is not supported merely because an interface exists.

| Backend | Status | Correctness mode | Determinism claim | Fallback | Requirements | Unsupported |
| --- | --- | --- | --- | --- | --- | --- |
| `cpu_reference` | Implemented and tested | Exact CPU reference for finite input, cell density, and slot-order mean velocity | Bitwise on the same binary, machine, and stable input order | N/A | Standard Rust target | GPU acceleration, cross-machine bitwise identity, identity-aware avoidance from aggregate fields |
| Metal | Not implemented | No claim | No claim | `cpu_reference` | N/A | All Metal execution and performance claims |
| CUDA | Not implemented | No claim | No claim | `cpu_reference` | N/A | All CUDA execution and performance claims |
| Vulkan compute | Not implemented | No claim | No claim | `cpu_reference` | N/A | All Vulkan execution and performance claims |

## Contract

`SpatialFieldKernel` takes immutable `FieldSample` values and produces only
aggregate `FieldValue` data. It cannot mutate stable IDs, routes, root motion,
layer composition, or cache records. A future GPU backend must compare against
the CPU reference with an explicit numeric tolerance and capture driver/API,
device, input hash, and fallback behavior in its scale report before being
listed as implemented.
