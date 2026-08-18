# M5 10K cache-streaming preflight

Date: 2026-08-12  
Status: **measured cache preflight only; not the M5 10K acceptance report**

## Reproduction

```sh
cargo run --release -p crowd-bench -- cache-experiment \
  --agents 10000 --cache-frames 8 --out /tmp/blender-crowd-m5-cache-10k-short
```

The new `--cache-frames` argument keeps a smoke/preflight matrix bounded while
preserving the existing nine encoding/chunk candidates. Full M5 gate reports
must use a separately declared scene duration and retain the resulting raw
artifact.

## Result

The local macOS/aarch64 release run generated eight 10,000-agent fixture
frames (input hash `e08500aebc76f98ab898ab1732f82d010f75fe66b9e250718c8d104686be2764`).
The selected candidate was F32 with 30-tick chunks: 5,080,885 bytes, 123.3
write frames/s, 252.9 read frames/s, zero positional encoding error, and
37.0 ms cancellation/recovery probe. All nine candidates completed with one
recovered cancellation chunk.

Environment capture reported Rust `1.94.1`, but CPU and RAM were unavailable
to the restricted runner and the run had no named reference workstation or
Blender version. This makes it unsuitable as an M5 performance claim.

## What this proves

- A 10K fixture can be encoded, range-read, and cancellation-recovered by the
  cache matrix on this machine.
- The preflight explicitly measures cache size and encoding error.

## What it does not prove

It does not use a declared M5 stadium/city/formation scene, a production tick
duration, simulation tiers, GPU backend, Blender viewport, or render
extraction. It cannot satisfy the 10K gate or authorize beginning the 100K
gate.
