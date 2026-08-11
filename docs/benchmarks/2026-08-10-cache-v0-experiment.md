# Cache v0 experiment

- Agents: 1000
- Frames: 120
- Seed: 2026
- Platform: macos/aarch64
- CPU: Apple M1 Max
- RAM: 68719476736 bytes
- Rust: rustc 1.94.1 (e408947bf 2026-03-25)
- Git commit: `ae7a568e7709cb3661a1e1863a3e9414460877cd`
- Git dirty: `true`
- Input hash: `9af7fc2ee98e384c413c1048886e04b2c3dbacc08dd7492ff60014f889dacf4e`

| Chunk ticks | Encoding | Bytes | Write fps | Read fps | Max error (m) | Cancel (ns) | Recovered chunks |
| ---: | --- | ---: | ---: | ---: | ---: | ---: | ---: |
| 30 | affine_i16 | 6749606 | 528.0 | 922.0 | 0.000233 | 16353375 | 1 |
| 30 | millimeter_i32 | 7229613 | 606.6 | 921.7 | 0.000502 | 19622084 | 1 |
| 30 | f32 | 7229604 | 584.8 | 948.9 | 0.000000 | 23257708 | 1 |
| 60 | affine_i16 | 6749113 | 942.6 | 461.4 | 0.000235 | 16191584 | 1 |
| 60 | millimeter_i32 | 7229119 | 766.8 | 456.7 | 0.000502 | 18184959 | 1 |
| 60 | f32 | 7229109 | 677.8 | 455.3 | 0.000000 | 8701917 | 1 |
| 120 | affine_i16 | 6748865 | 935.6 | 215.9 | 0.000240 | 13139042 | 1 |
| 120 | millimeter_i32 | 7228872 | 886.6 | 214.5 | 0.000502 | 11385834 | 1 |
| 120 | f32 | 7228863 | 1300.7 | 214.5 | 0.000000 | 23532500 | 1 |

Selected: `affine_i16` with `120`-tick chunks.

Selection rule: smallest bytes with <=0.001m error and read time <= matching f32 time * 1.10; ties prefer fewer chunks then affine_i16, millimeter_i32, f32.

This experiment does not establish 10,000- or 100,000-agent performance.
