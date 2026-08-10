# Cache v0 experiment

- Agents: 1000
- Frames: 120
- Seed: 2026
- Platform: macos/aarch64
- CPU: Apple M1 Max
- RAM: 68719476736 bytes
- Rust: rustc 1.94.1 (e408947bf 2026-03-25)
- Git commit: `884b54bc270d2b18ed24013f91f0103053532071`
- Git dirty: `true`
- Input hash: `9af7fc2ee98e384c413c1048886e04b2c3dbacc08dd7492ff60014f889dacf4e`

| Chunk ticks | Encoding | Bytes | Write fps | Read fps | Max error (m) | Cancel (ns) | Recovered chunks |
| ---: | --- | ---: | ---: | ---: | ---: | ---: | ---: |
| 30 | affine_i16 | 6749606 | 998.6 | 1112.8 | 0.000233 | 7806458 | 1 |
| 30 | millimeter_i32 | 7229613 | 1158.0 | 1079.8 | 0.000502 | 8335583 | 1 |
| 30 | f32 | 7229604 | 1126.5 | 1082.6 | 0.000000 | 9278292 | 1 |
| 60 | affine_i16 | 6749113 | 1446.6 | 541.6 | 0.000235 | 10946166 | 1 |
| 60 | millimeter_i32 | 7229119 | 1614.0 | 547.9 | 0.000502 | 10115542 | 1 |
| 60 | f32 | 7229109 | 1601.2 | 553.3 | 0.000000 | 7327000 | 1 |
| 120 | affine_i16 | 6748865 | 1955.7 | 259.8 | 0.000240 | 10025250 | 1 |
| 120 | millimeter_i32 | 7228872 | 1952.9 | 261.3 | 0.000502 | 9374333 | 1 |
| 120 | f32 | 7228863 | 2022.0 | 262.5 | 0.000000 | 13472750 | 1 |

Selected: `affine_i16` with `120`-tick chunks.

Selection rule: smallest bytes with <=0.001m error and read time <= matching f32 time * 1.10; ties prefer fewer chunks then affine_i16, millimeter_i32, f32.

This experiment does not establish 10,000- or 100,000-agent performance.
