# Milestone A validation record

Date: September 19, 2026. Environment: Linux x64, development Node.js 25.2.1, Electron 44.4.3. This record concerns the surface laboratory, not terrain or climate realism.

## Completed checks

- Strict TypeScript checking and production renderer/main/preload build.
- Fourteen automated tests covering recipe rejection and round trips, fixed random sequences, stream isolation/resumption, spherical geometry, topology, region selection, radius scaling, a 20-seed ensemble, and projection coverage.
- A real Electron interaction run: initial generation, inspection, zoom, layer changes, save/import through native-dialog handlers, rejection of a future model version, replacement of an in-flight generation, cancellation, and preservation of the previous valid world.
- Exact reproduction between independent Electron workers and through desktop recipe save/load.
- Full-array comparison between desktop and headless execution: exact integer topology and scaled floating-point differences below `1e-12`.
- Screenshot inspection of `artifacts/surface-desktop.png`.
- Linux x64 unpacked application creation and the same complete interaction suite against `release/Planimulation-linux-x64/planimulation`, producing `artifacts/surface-desktop-packaged.png`.
- Initial generation benchmarks at levels 3–6, recorded in [development.md](development.md).

## Issues found and resolved

1. A boundary half-space test assumed convex dual cells. Barycentric regions can be slightly concave. Picking now tests the constituent spherical triangles; tests independently check containment through spherical area decomposition.
2. Opposite flat-map edges produced tiny trigonometric differences near a selection tie. Longitude input is now canonicalized, and candidate ties use stable region ordering.
3. The first Electron run required a runtime download, exceeding the automation launch timeout. The run was repeated after installation completed.
4. An initial assumption of identical Node/Electron checksums was disproved. Full-array comparison measured a maximum scaled difference of `4.927791634296056e-16`, with identical topology. Tests now distinguish exact same-runtime reproduction from explicitly tolerated cross-runtime numerical agreement. This limitation is documented rather than hidden by rounding the saved data.

## Remaining boundaries

- The diagnostic signal is not terrain, water, climate, resources, or vegetation.
- Recipe persistence is not simulation checkpoint persistence.
- The coarse surface does not resolve local landscapes; rendered pixels do not increase physical resolution.
- UI generation timing excludes subsequent projection/rendering. Array-memory figures exclude Electron and temporary allocations.
- No scientific realism claim follows from these tests.
- Windows/macOS packaging and runtime behavior are not validated by this Linux run.
