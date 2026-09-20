# Native/GPU foundation validation

Date: September 20, 2026. Scope: `surface-rust-1`, Linux x64. This records a small integration workload, not a performance guarantee for future climate or civilization systems.

## Environment

- ThinkPad T14 Gen 1, AMD Ryzen 5 PRO 4650U with integrated Radeon Vega graphics.
- 14.84 GiB usable system memory; approximately 11.36 GiB occupied at inspection, with other desktop applications running.
- Rust 1.98.1, Node.js 25.2.1, Electron 44.4.3, Three.js 0.186.0.
- Electron reported GPU compositing and WebGL enabled. No forced software-rendering flags were used.
- The default native/headless/desktop initial fingerprint was `5e97cb6f`.

## Numerical and integration checks

The automated suite contains four Rust tests and 21 TypeScript tests. Native tests cover every supported mesh level (0–6), a 20-seed reproducibility/transport ensemble, physical radius scaling, strict recipes, and fixed random-number vectors. TypeScript tests include the independent numerical reference, strict framing/decoding, cancellation and backpressure, projection/picking, and dual-view display geometry.

Checks passed for compilation, TypeScript strict checking, Rust formatting, Clippy with warnings denied, and whitespace validation. The full 21-test TypeScript listing can also be inspected with:

```sh
node --import tsx --test --test-reporter=spec --test-isolation=none tests/*.test.ts
```

The native/TS comparison exposed a cyclic boundary-list offset at the ±π branch cut, rather than different physical geometry. The implementation now anchors each ring to a fixed incident face. Integer topology agrees exactly; floating fields are checked within a scaled `1e-11` tolerance. Native repeats compare full arrays exactly.

The desktop suite verifies the same initial native fingerprint headlessly and in Electron, typed-array IPC, sandbox isolation, shared map/globe selection and inspector values, view/layer changes, diagnostic stepping, native recipe save/open, invalid imports, replacement, cancellation, and continued stepping of the retained world. Screenshots are generated for both views. GPU shader errors are checked in addition to renderer exceptions.

The final development build and unpacked Linux executable both passed this suite, with no renderer exceptions or detected shader errors. Both reported `5e97cb6f`. The packaged run used `npm run test:desktop -- release/Planimulation-linux-x64/planimulation`, exercising the binary under `resources/native` rather than the development path.

## Native process and transport sample

One `npm run benchmark` run measured the following. Native generation includes startup, calculation, serialization, pipe transport, validation, and Node decoding. The TS column is the independent generator only. Dynamic p95 is the nearest-rank 95th percentile of 20 requests, each advancing four steps and returning one scalar field. It excludes Electron IPC and rendering.

| Regions | Native generation + adapter | TS reference | Four steps + adapter p95 | Model arrays | Dynamic field |
| ---: | ---: | ---: | ---: | ---: | ---: |
| 642 | 32.2 ms | 39.2 ms | 0.80 ms | 0.26 MiB | 5.02 KiB |
| 2,562 | 34.4 ms | 132.3 ms | 1.02 ms | 1.05 MiB | 20.02 KiB |
| 10,242 | 122.6 ms | 366.0 ms | 1.89 ms | 4.22 MiB | 80.02 KiB |
| 40,962 | 418.3 ms | 1,070.2 ms | 7.86 ms | 16.88 MiB | 320.02 KiB |

Relative stock error after 80 diagnostic steps was at most approximately `1.56e-15` in this sample. The workload is deterministic graph diffusion, not hydrology. These are single-run observations, not stable timing thresholds or a general Rust-versus-TypeScript benchmark.

## Desktop dynamics and camera sample

`npm run benchmark:desktop` captured 180 requested-animation-frame intervals per combination while alternating camera zoom and receiving native diagnostic frames. The successful raw report timestamp was `2026-09-20T09:28:48.003Z`, stored locally at `artifacts/native-desktop-benchmark.json`.

| Regions | View | Median interval | p95 interval | Maximum interval | Steps during sample |
| ---: | --- | ---: | ---: | ---: | ---: |
| 10,242 | Flat map | 16.6 ms | 20.4 ms | 24.7 ms | 292 |
| 10,242 | Globe | 18.3 ms | 20.1 ms | 21.1 ms | 356 |
| 40,962 | Flat map | 18.6 ms | 20.4 ms | 23.0 ms | 300 |
| 40,962 | Globe | 18.8 ms | 20.5 ms | 21.9 ms | 308 |

Observed generate-to-ready durations were approximately 0.93 seconds at level 5 and 1.49 seconds at level 6. They include view preparation and automation polling overhead, not just Rust generation. UI readiness does not itself measure when the first GPU frame reaches the display.

The frame sample indicates usable interactive scheduling under this workload, not guaranteed 60 FPS. RAF intervals are not GPU execution timers. It does not stress high-frequency picking, massive city overlays, long-running climate, extreme zoom, or multiple simultaneous inspectors. The largest reported relative stock error at the sampled endpoints was approximately `8.66e-15`.

## Memory and packaging limits

At the end of the desktop run, Electron reported working sets of approximately 245 MiB for the browser process, 170 MiB for GPU, 86 MiB for a utility process, and 278 MiB for the renderer. Renderer peak working set was about 701 MiB during the run. These process counters can include shared pages and exclude the separately spawned native process; they must not be added and labeled unique total application memory.

This is substantially more than the raw model-array size. Display geometry, temporary worker arrays, browser overhead, and GPU resources matter. Memory optimization, chunked rendering, indexed meshes/LOD, and peak-memory profiling are required before increasing supported resolution substantially. The current evidence supports the bounded foundation, not million-region readiness.

The unpacked Linux x64 application is approximately 285 MiB. Its bundled native executable is approximately 668 KiB and resides outside `app.asar` at `resources/native/planimulation-core`. It is not an installer or a signed portable distribution. Vite still reports a nonfatal >500 kB renderer-chunk warning (approximately 565 kB uncompressed); the application loads local offline assets.

No terrain realism, physical climate accuracy, cross-platform bitwise reproducibility, checkpoint continuation, or scalability beyond level 6 is claimed. The next increment is inspectable geological generation with real elevation and explicit water accounting, followed by renewed performance and memory measurement.
