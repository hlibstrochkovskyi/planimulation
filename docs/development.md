# Development and validation

## Current increment

Milestone A establishes spherical geometry, recipes, and inspection. The September 20 native/GPU increment moves desktop and headless calculations to Rust and adds two Three.js views: a flat atlas and a globe. Both display the same region IDs, fields, and selection. B3 now displaces the globe using computed elevation.

Milestone B1 adds static plate kinematics. B2 adds independent continentality, approximate crust thickness/density, and area fitting. B3 adds inspectable elevation contributions, distance-decayed boundary effects, and globe relief. Read [Explainable elevation](terrain.md) for the current `terrain-1` recipe and protocol 4 extension. No water, erosion, or geological time integration is implemented yet.

The diagnostic diffusion mode exists to exercise stateful native calculation, small dynamic messages, and responsive rendering. It is not a climate, erosion, or geological model. Its step number is not a calendar. Read [Native/GPU foundation](native-foundation.md) for the protocol, numerical definition, and limitations. The original [milestone A report](validation-milestone-a.md) is historical and describes the superseded TS/Canvas implementation.

## Daily workflow

Prerequisites: Node.js 22.12+, npm, Rust and a platform linker, a graphical session for desktop tests, and WebGL 2 for viewing. Linux x64 is the validated target. Rust 1.98.1 is the tested toolchain; other targets/toolchains are not claimed to be bitwise equivalent.

```sh
npm ci
npm test
npm run build
npm run desktop
```

`npm test` builds the release native executable, runs Cargo tests, and runs the TypeScript tests. `npm start` builds and launches. Cargo dependencies and npm dependencies have committed lockfiles. Initial dependency setup may require network access; the built application is offline.

```sh
npm run typecheck
cargo fmt --check --manifest-path native/Cargo.toml
cargo clippy --locked --manifest-path native/Cargo.toml --all-targets -- -D warnings
npm run headless
npm run headless -- path/to/recipe.json
npm run benchmark
npm run test:desktop
npm run benchmark:desktop
npm run package
npm run test:desktop -- release/Planimulation-linux-x64/planimulation
```

Headless and the non-GUI benchmark require a previous native build (`npm run build:native` or a full build). GUI scripts open and close real windows, use temporary profiles, and write screenshots/reports under ignored `artifacts/`. Do not close their windows manually during a run. The desktop suite stubs only native file dialogs, preserving real file handlers, preload validation, native execution, and GPU rendering. Run GUI benchmarks without simultaneous builds or other project benchmarks.

Packaging produces an unpacked app under `release/`; it neither installs globally nor signs a release. The native executable is a separate resource, outside `app.asar`, and is bundled for the build host. Rust and Node.js are not needed on the end user's machine. Cross-platform distribution, portable Linux libc baselines, signing, installers, and crash reporting remain future work.

## Responsibilities

- `native/src/lib.rs`: validated recipes, random stream, spherical model, diagnostic evolution. No Electron or rendering dependency.
- `native/src/tectonics.rs`: connected partition, independent random streams, rigid plate velocities, and boundary kinematics.
- `native/src/crust.rs`: independent spherical potential, area-weighted fitting, initial continentality, thickness, and density.
- `native/src/terrain.rs`: crust baseline, bounded boundary responses, graph-distance decay, independent detail, and elevation contributions.
- `native/src/wire.rs`, `native/src/main.rs`: versioned, bounded command and binary-output adapter.
- `src/native/client.ts`: process lifecycle, framing, validation, generation transactions, headless integration.
- `src/electron/`: trusted sender checks, native process ownership, native recipe dialogs, sandboxed preload.
- `src/renderer/`: DOM controls, background display triangulation, GPU views, picking, inspectors. Does not advance simulation state itself.
- `src/core/`: shared recipe/types and an independent TS geometry/generation regression reference. Desktop/headless do **not** use that generator.

No arbitrary filesystem path, shell command, raw IPC method, or Node.js object is exposed to the renderer. Commands are narrow named operations. Navigation, new windows, permissions, and network access are restricted. Native requests time out; a session accepts at most one outstanding request. Closing/reloading the window stops its native processes.

## Testing procedural behavior

| Concern | Check |
| --- | --- |
| Topology | All native levels 0–6: connected graph, reciprocal adjacency, twelve pentagons, two faces per edge, Euler characteristic 2 |
| Geometry | Unit vectors, positive areas, sum `4πR²`; independent TS reference and known spherical octant |
| Units | Doubling radius multiplies areas by four and distances by two |
| Randomness | Fixed FNV-1a/Mulberry32 reference outputs, independent streams, 20-seed ensembles |
| Native determinism | Repeated generations compare complete arrays; headless and desktop fingerprints agree for the same binary |
| Cross-language agreement | Integer topology matches; floating values match within `1e-11 * max(1, abs(reference))`, not bitwise |
| Transport | Conserved area-weighted stock, finite/bounded fields, constant-field equilibrium, identical batched/unbatched steps |
| Protocol | Fragmented and coalesced frames, size/version checks, malformed arrays, NaNs, strict recipes |
| Lifecycle | Cancel/replacement, retained active world, prepared-world cancellation, stale epoch and concurrent step rejection |
| Projection | Polar/seam coverage, triangle area coverage, stable IDs in both views |
| Plates | 180 ensemble combinations: complete coverage, connected nonempty ownership, exact shared-boundary coverage; controlled speed/radius/count changes |
| Kinematics | Constructed divergence/convergence/shear/quiet cases, tangency, bounded speed, common-reference-frame invariance, retained oblique components |
| Crust | 900 ensemble combinations; physical-area fitting, plateaus/extremes, monotonicity, units, seam continuity, raw-field refinement, independent plate/crust controls |
| Elevation | Constructed boundary/graph cases, 60-seed-resolution ensemble, contribution reconstruction, amplitude/width/radius response, immutability during diagnostics |
| Relief display | Shared corners, reversible exaggeration and radius safety limit, unchanged flat/model geometry, displaced raycasting, finite normals |
| Desktop | Shared selection/data, view and layer changes, play/pause, recipe round trip, invalid import, cancellation and continued dynamics |
| Appearance | Review flat/globe screenshots; numerical tests alone cannot establish readable graphics |

Numerical comparisons use justified tolerances. Repeated operation in the same supported binary is exact. Native boundary rings start at a fixed incident face, avoiding an arbitrary change of starting vertex at the `atan2` ±π branch cut.

## Recipes and persistence

```json
{
  "schemaVersion": 1,
  "modelVersion": "terrain-1",
  "randomVersion": "fnv1a-utf8-mulberry32-1",
  "seed": "first-light",
  "subdivision": 5,
  "radiusMeters": 6371000,
  "plateCount": 12,
  "maxPlateSpeedCmPerYear": 8,
  "continentalFraction": 0.38,
  "continentalScale": 1,
  "reliefScale": 1,
  "boundaryWidthKm": 300,
  "detailAmplitudeMeters": 300
}
```

All fields are required; unknown fields and unsupported versions are rejected. Seeds preserve their exact text and are limited to 128 UTF-16 code units. The UI shows radius in kilometers; the model stores meters. Native recipe file reads are bounded to 32 KiB. Levels 0–6 are supported.

`surface-1`, `surface-rust-1`, `tectonics-1`, and `crust-1` are not silently migrated. To explore an old seed under the new model, create a new recipe from its visible parameters and keep the old file unchanged; identical checksums are not promised. The old implementation remains available through Git history. Display-only exaggeration is not a recipe parameter.

The displayed fingerprint covers the **initial** recipe and native arrays. It is an FNV-1a regression checksum, not cryptography. Running the diagnostic deliberately does not relabel it as a live-state checksum. Save recipe regenerates the initial field at step zero. Full simulation checkpoints, rewind, and cross-version migrations are not implemented.

## Next increment

Next add initial water fitting with an explicit volume budget and connected-component classification. Use constructed bowls and controlled parameter changes before tuning a seed ensemble. Both atlas and globe must read those same physical fields; visual vertical exaggeration must not change reference areas, physical heights, or model outcomes.

Keep commits coherent, messages and project records in English, and the owner's configured Git identity. Never add assistant attribution or co-author trailers.
