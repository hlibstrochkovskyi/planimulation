# Native core and dual-view foundation

Status: implemented foundation increment, September 20, 2026. Terrain, climate, and civilization remain future work.

Historical baseline: recipe `surface-rust-1` and protocol 1 below describe the foundation increment. B1 adds protocol 2, B2 protocol 3, B3 protocol 4, and B4 now uses `water-1` with protocol 5. See [Plate kinematics](tectonics.md), [Initial crust](crust.md), [Explainable elevation](terrain.md), and [Initial water](water.md) for appended fields and checks. Process ownership and the diagnostic update mechanism remain unchanged. The measurements below apply to the earlier surface-only workload.

## Decision and scope

Use a Rust library/executable for authoritative model calculation, Electron/TypeScript for the analytical desktop interface, and Three.js/WebGL 2 for a flat map and a globe. These are two representations of one world, not independently generated maps. Settlements will be markers and routes will be overlays; detailed 3D cities are not required.

This retains the existing desktop controls and testing investment while putting numerical work outside the UI runtime. It also makes the same native binary usable by headless tests. It does not establish that every future simulation will be fast. Multithreading, Wasm, GPU simulation, shared memory, native graphics, and another desktop shell remain decisions to justify with workloads, not dependencies to introduce preemptively.

Current limits: 12–40,962 regions, scalar diagnostic field, single-threaded numerical stepping, no physical relief, no historical replay, no dynamic-state persistence. Both view geometries are retained in GPU memory for immediate switching. Memory and performance at hundreds of thousands or millions of regions are not validated.

## Ownership and publication

```text
Rust process: authoritative world and diagnostic step
    → bounded binary messages → Electron main-process adapter
    → structured-clone IPC → sandboxed renderer's display snapshot
    → display geometry worker / field texture → GPU atlas or globe
```

Generation starts a candidate native process; the last accepted process stays alive. The candidate delivers a world, and a Worker builds flat/globe display buffers. The renderer then queues acceptance and publishes the matching view in the same UI task. Canceling a pending generation or its view preparation closes the candidate, preserving the accepted world. Epochs reject stale dynamics requests. The renderer uses a separate request sequence to discard obsolete results. Reload, renderer termination, or window close cleans up sessions.

Native sessions accept one outstanding command, with a 60-second failure timeout. UI playback requests four diagnostic steps, waits for the response, then waits at least 33 ms before requesting another batch. This caps backlog; simulation does not depend on render frequency. Pause stops scheduling new batches; an already requested batch can complete. The inspector is refreshed from the same received field as the visible layer.

The initial mesh is not resent on each step. At 40,962 regions the full arrays occupy 16.88 MiB, while a field update is 320.02 KiB plus small metadata. Native stdout and Electron IPC still involve copies. No zero-copy or shared-memory claim is made. Browser display buffers use Float32; model values and transport accounting use Float64.

## Wire protocol 1

Input: one UTF-8 JSON command per line, at most 32 KiB. Commands are `generate` with a strictly validated recipe or `advance` with an integer step count from 1 to 100. Stdout is reserved for framed responses; diagnostics belong on stderr.

Output: a four-byte little-endian header length, UTF-8 JSON header, then exactly `byteLength` bytes of binary data. Headers are bounded to 32 KiB; bodies to 32 MiB in the adapter. Header fields include `protocol: 1`, `kind` (`world`, `frame`, or `error`), and `byteLength`. The incremental parser supports arbitrary pipe fragmentation without repeatedly concatenating a whole partially received world.

A world body has this fixed array order. Array lengths are derived from the validated recipe, not trusted arbitrary offsets:

| Array | Encoding | Length (`N = 10 × 4^level + 2`, `D = 6N − 12`) |
| --- | --- | ---: |
| Region centers | Float64, xyz | 3N |
| Triangulation faces | Uint32 | 60 × 4^level |
| Neighbor offsets | Uint32 | N + 1 |
| Neighbors | Uint32 | D |
| Neighbor distances, meters | Float64 | D |
| Region areas, m² | Float64 | N |
| Boundary offsets | Uint32 | N + 1 |
| Boundary directions, xyz | Float64 | 6D |
| Diagnostic signal | Float64 | N |

All numerical binary encoding is little-endian. The adapter validates lengths, finite floats, topology index/offset bounds, metadata, and version before publication. A frame contains only N Float64 signal values, tick, and relative mass error. This is an internal versioned protocol, not yet a stable public API or checkpoint format.

## Diagnostic transport definition

For each undirected neighbor pair `(i,j)`, calculate once:

```text
exchange = 0.1 × min(area[i], area[j]) × (signal[i] − signal[j])
delta[i] -= exchange
delta[j] += exchange

signal_next[i] = signal[i] + delta[i] / area[i]
density[i] = (signal[i] + 1) / 2
total_stock = sum(area[i] × density[i])
```

All exchanges use the previous step, and accumulated deltas are applied simultaneously. Equal-and-opposite exchange conserves area-weighted stock within floating-point tolerance. Maximum degree six and coefficient 0.1 make every new signal a convex combination of previous signals, preserving their range without clipping. Tests compare 100 steps against ten batches of ten and check constant-field equilibrium.

This is a graph-diffusion load test, not a spatially calibrated diffusion PDE. It does not use boundary length, diffusion coefficients, or a physical timestep; resolution convergence and hydrological meaning are explicitly not claimed. The purpose is to validate ownership, determinism, conservative update mechanics, transport size, and concurrent visualization.

## Geometry and version transition

The barycentric dual icosphere keeps stable IDs, twelve five-neighbor cells, and six neighbors elsewhere. Boundaries start at a fixed incident-face centroid before serialization. This canonical start avoids ±π branch-cut ordering differences between Rust and JavaScript math. Integer topology is compared with an independent TS implementation; floating arrays use a scaled `1e-11` comparison. Native repetitions compare entire arrays exactly.

The native recipe version is `surface-rust-1`. The previous `surface-1` used JavaScript math and different fingerprint serialization, so it is rejected rather than silently reinterpreted. The native fingerprint is FNV-1a over compact serde serialization of the recipe followed by the fixed binary array body. It identifies the initial world only. The Cargo lockfile and native build determine this numerical implementation; bitwise equivalence across architectures, compiler upgrades, or future model versions is not promised.

The flat atlas clips seam-crossing polygons into the map rectangle and triangulates their potentially nonconvex projected outlines. The globe uses region triangle fans. Region IDs are retained in vertex attributes; both meshes sample one scalar texture. View changes and camera controls neither run generation nor change model values. Selecting a rendered triangle identifies its original region. Projection, clipping, shading, and Float32 precision are presentation details, not changes to physical geography.

## Measurement and remaining work

The development machine is a ThinkPad T14 Gen 1 with AMD Ryzen 5 PRO 4650U, integrated Radeon Vega graphics, and 14.84 GiB usable RAM. Approximately 11.36 GiB was in use at inspection. Existing swap occupancy alone is not evidence of active paging. Measurements must be interpreted in the context of concurrent desktop workloads.

`npm run benchmark` includes native process startup, generation, serialization, pipe transport, validation, and Node decoding. Its TS reference column is generation/checksum only, so the comparison is not a pure language microbenchmark. The repeated dynamic sample includes four steps plus pipe/decoding, but not Electron or GPU rendering.

`npm run benchmark:desktop` samples 180 requested-animation-frame intervals while alternating zoom and running diagnostic batches, at levels 5 and 6 in both views. It records Electron GPU feature status and Electron process memory (not the separate native process). This measures presentation scheduling under an integration workload, not individual GPU execution time. It is neither an FPS promise for future geology nor a comprehensive worst-case responsiveness test. Raw reports live in `artifacts/`; measured values and caveats are retained in the [validation report](validation-native-foundation.md).

Before increasing scope: preserve finite/balance invariants, add actual geological fields with units, keep static and changing payloads separate, and repeat the integration benchmark. Large-world chunking/LOD, transport profiling, crash recovery, cross-platform packages, accessibility work, and real physical checkpoints remain explicit follow-ups.

Implementation references: [Electron IPC](https://www.electronjs.org/docs/latest/api/ipc-renderer) specifies structured-clone transfer semantics; [Three.js WebGLRenderer](https://threejs.org/docs/pages/WebGLRenderer.html) documents WebGL 2 rendering and resource disposal; [OrbitControls](https://threejs.org/docs/pages/OrbitControls.html) provides view navigation. These references concern software mechanics, not scientific validation.
