# M5 backend support matrix

This matrix is the authoritative support boundary for the M5 spatial-field
kernel. A backend is not supported merely because an interface exists.

| Backend | Status | Correctness mode | Determinism claim | Fallback | Requirements | Unsupported |
| --- | --- | --- | --- | --- | --- | --- |
| `cpu_reference` | Implemented and tested | Exact CPU reference for finite input, cell density, and slot-order mean velocity | Bitwise on the same binary, machine, and stable input order | N/A | Standard Rust target | GPU acceleration, cross-machine bitwise identity, identity-aware avoidance from aggregate fields |
| Metal | Not implemented | No claim | No claim | `cpu_reference` | N/A | All Metal execution and performance claims |
| CUDA | Not implemented | No claim | No claim | `cpu_reference` | N/A | All CUDA execution and performance claims |
| Vulkan compute | Not implemented | No claim | No claim | `cpu_reference` | N/A | All Vulkan execution and performance claims |

## Fallback and parity, as enforced

`crowd_core::field::select_backend` is the single place a requested backend is
resolved. Asking for Metal, CUDA, or Vulkan returns the CPU reference together
with a `BackendSelection` recording `fell_back: true` and a reason naming the
backend that could not be provided, so a report says what actually ran rather
than what was requested.

`crowd_core::field::compare_kernels` is the comparison a candidate backend must
pass. Density is an integer agent count per cell and carries no tolerance — a
backend that moves an agent between cells has changed the field's meaning, not
its precision. Mean velocity is a float reduction and carries a declared
`KernelTolerance`, defaulting to 1e-3 m/s.

`crates/crowd-core/tests/m5_cpu_fallback.rs` proves the comparison
discriminates rather than merely existing: a pairwise-reduction CPU fixture
agrees within tolerance while *not* being bitwise identical (2.4e-7 m/s), a
0.01 m/s drift is rejected, and a cell-shifting backend is rejected regardless
of tolerance.

## Contract

`SpatialFieldKernel` takes immutable `FieldSample` values and produces only
aggregate `FieldValue` data. It cannot mutate stable IDs, routes, root motion,
layer composition, or cache records. A future GPU backend must compare against
the CPU reference with an explicit numeric tolerance and capture driver/API,
device, input hash, and fallback behavior in its scale report before being
listed as implemented.
