# Cache v0 experiment

- Agents: 1000
- Frames: 120
- Seed: 2026
- Platform: macos/aarch64
- CPU: Apple M1 Max
- RAM: 68719476736 bytes
- Rust: rustc 1.94.1 (e408947bf 2026-03-25)
- Git commit: `fb5ee70536eb6e90007a85d1de26a09dc3c2f60f`
- Git dirty: `true`
- Input hash: `9af7fc2ee98e384c413c1048886e04b2c3dbacc08dd7492ff60014f889dacf4e`

| Chunk ticks | Encoding | Bytes | Write fps | Read fps | Max error (m) | Cancel (ns) | Recovered chunks |
| ---: | --- | ---: | ---: | ---: | ---: | ---: | ---: |
| 30 | affine_i16 | 6749606 | 1017.7 | 1159.6 | 0.000233 | 11243959 | 1 |
| 30 | millimeter_i32 | 7229613 | 1096.8 | 1135.2 | 0.000502 | 10602791 | 1 |
| 30 | f32 | 7229604 | 985.6 | 1145.9 | 0.000000 | 10946208 | 1 |
| 60 | affine_i16 | 6749113 | 1282.0 | 552.7 | 0.000235 | 10941250 | 1 |
| 60 | millimeter_i32 | 7229119 | 1448.6 | 539.7 | 0.000502 | 12277583 | 1 |
| 60 | f32 | 7229109 | 1583.7 | 547.0 | 0.000000 | 7722708 | 1 |
| 120 | affine_i16 | 6748865 | 1861.6 | 260.4 | 0.000240 | 11532333 | 1 |
| 120 | millimeter_i32 | 7228872 | 1785.9 | 257.0 | 0.000502 | 11473667 | 1 |
| 120 | f32 | 7228863 | 1836.7 | 258.4 | 0.000000 | 10988125 | 1 |

Selected: `affine_i16` with `120`-tick chunks.

Selection rule: smallest bytes with <=0.001m error and read time <= matching f32 time * 1.10; ties prefer fewer chunks then affine_i16, millimeter_i32, f32.

This experiment does not establish 10,000- or 100,000-agent performance.
