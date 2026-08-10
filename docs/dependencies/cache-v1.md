# Cache v1 dependency decision

Date: 2026-08-10
Status: accepted for the M0 cache experiment and M1 working cache
Owning design: [M0 cache closure and M1 vertical slice](../superpowers/specs/2026-08-10-m0-cache-m1-vertical-slice-design.md)

## Decision

Cache v1 uses BLAKE3 for deterministic content identity and CRC-32C for binary
payload corruption detection. JSON Schema validation and temporary-directory
management are test-only dependencies. None of these libraries owns cache
layout, simulation state, compression, file publication, or Blender behavior.

The versions below are the exact versions resolved in `Cargo.lock` when this
decision was recorded.

| Crate | Resolved version | Scope | License | Upstream |
|---|---:|---|---|---|
| `blake3` | 1.8.6 | Runtime | CC0-1.0 OR Apache-2.0 OR Apache-2.0 WITH LLVM-exception | [BLAKE3-team/BLAKE3](https://github.com/BLAKE3-team/BLAKE3) |
| `crc32c` | 0.6.8 | Runtime | Apache-2.0/MIT | [zowens/crc32c](https://github.com/zowens/crc32c) |
| `jsonschema` | 0.33.0 | Tests only, default features disabled | MIT | [Stranger6667/jsonschema](https://github.com/Stranger6667/jsonschema) |
| `tempfile` | 3.27.0 | Tests only | MIT OR Apache-2.0 | [Stebalien/tempfile](https://github.com/Stebalien/tempfile) |

`proptest` was already a workspace test dependency before cache v1 and is not a
new production selection.

## Responsibilities

### BLAKE3

`blake3` hashes canonical project input and complete base-cache content. The
hash is a reproducibility and invalidation fingerprint; it is not a signature,
authentication mechanism, or authorization boundary. Cache manifests store the
32-byte digest as 64 lowercase hexadecimal characters.

### CRC-32C

`crc32c` checks each encoded binary payload independently. It uses the standard
Castagnoli polynomial and the standard `123456789 -> e3069283` check value is
pinned by a test. CRC-32C detects accidental corruption; it is not described as
cryptographically collision-resistant.

### JSON Schema validator

`jsonschema` proves that emitted JSON conforms to the checked versioned schema.
Default features are disabled, so the test dependency cannot resolve remote
HTTP/file references. Every cache schema is local and self-contained.

### Temporary directories

`tempfile` gives cache lifecycle tests unique, automatically cleaned directory
trees. It is not linked into `crowd-cache` outside test builds and does not
affect the working cache path policy.

## Rejected alternatives

- A hand-written CRC or hash was rejected because standardized algorithms have
  stable test vectors and reviewed optimized implementations; custom integrity
  code would add risk without product differentiation.
- Rust's default hashers were rejected because their output is not a stable
  serialized contract and is not intended for corruption detection.
- A cryptographic digest for every frame chunk was rejected for M0 because
  CRC-32C already satisfies accidental-corruption detection and the complete
  cache still receives a BLAKE3 identity hash.
- Compression libraries remain unselected. The M0 experiment first measures
  chunking and quantization without conflating them with a codec dependency.
