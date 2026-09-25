# Development and validation

## Current increment

Milestone A establishes spherical geometry, recipes, and inspection. The September 20 native/GPU increment moves desktop and headless calculations to Rust and adds two Three.js views: a flat atlas and a globe. Both display the same region IDs, fields, and selection. B3 now displaces the globe using computed elevation.

Milestone B1 adds static plate kinematics. B2 adds independent continentality, approximate crust thickness/density, and area fitting. B3 adds inspectable elevation contributions, distance-decayed boundary effects, and globe relief. B4 adds initial water filling; B5 adds separate globe water surfaces. C1 adds drainage structure; [C2b basin inspection](basin-inspection.md) integrates the hierarchy with model `basins-1` and protocol 7. Water dynamics, erosion, and geological time integration are not implemented yet.

The diagnostic diffusion mode exists to exercise stateful native calculation, small dynamic messages, and responsive rendering. It is not a climate, erosion, or geological model. Its step number is not a calendar. Read [Native/GPU foundation](native-foundation.md) for the protocol, numerical definition, and limitations. The original [milestone A report](validation-milestone-a.md) is historical and describes the superseded TS/Canvas implementation.

[C2a basin analysis](basins.md) supplies the native bed-connectivity hierarchy, merge thresholds, and prism-storage queries. C2b computes it during generation and exposes branch/threshold layers and hierarchy inspection. A standalone report is also available: `cargo run --release --locked --manifest-path native/Cargo.toml --example basins -- path/to/recipe.json` (or `-` for stdin), using a current `basins-1` recipe.

[C3a reservoir experiments](reservoir-experiment.md) add isolated leaf storage, prescribed volume pulses, an optional external collector, and complete experiment checkpoints. They do not modify generated world water or the desktop. Run `cargo run --release --locked --manifest-path native/Cargo.toml --example reservoir -- docs/scenarios/reservoir-sill.json` for a hand-verifiable example.

[C3b coupled pairs](reservoir-pair.md) add conservative exchange across one declared sill, receiver filling, and common storage above it, in a closed developer experiment. Run `cargo run --release --locked --manifest-path native/Cargo.toml --example reservoir_pair -- docs/scenarios/reservoir-pair.json`. That experiment does not route through a hierarchy.

[C3c spill connections](spill-connections.md) derive the geometric child–plateau graph, all contacts, candidate receiving branches, and potential region passages from the bed. Run `cargo run --release --locked --manifest-path native/Cargo.toml --example spill_connections -- docs/scenarios/spill-connections.json`. This does not choose between recipients or move water.

[C3d nested filling](nested-reservoir.md) combines explicit passages and active storage in a closed, initially dry binary-hierarchy experiment of at most 128 regions. Run `cargo run --release --locked --manifest-path native/Cargo.toml --example nested_reservoir -- docs/scenarios/nested-reservoir.json`. Ambiguous outlets are rejected; this is not planetary hydrology or a desktop control.

## Daily workflow

Prerequisites: Node.js 22.12+, npm, Rust and a platform linker, a graphical session for desktop tests, and WebGL 2 for viewing. Linux x64 is the validated target. Rust 1.98.1 is the tested toolchain; other targets/toolchains are not claimed to be bitwise equivalent.

```sh
npm ci
npm test
npm run build
npm run desktop
```

`npm test` first type-checks the project (including tests), builds the release native executable, runs Cargo tests with `--all-targets` (including developer-example tests), and runs the TypeScript tests. `npm start` builds and launches. Cargo dependencies and npm dependencies have committed lockfiles. Initial dependency setup may require network access; the built application is offline.

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
- `native/src/water.rs`: area-weighted coverage fitting, fixed-volume filling, depths, and connected initial water bodies.
- `native/src/drainage.rs`: bed-based receivers, equal-height routing, terminal catchments, and topological land-area accumulation.
- `native/src/basins.rs`: plateau-batched connectivity hierarchy and level–storage analysis, owned by each generated world; `native/examples/basins.rs` emits its developer report.
- `native/src/reservoir.rs`: isolated single-reservoir storage curve, pulse budgets, and checkpoint validation; `native/examples/reservoir.rs` runs bounded developer experiments.
- `native/src/reservoir_pair.rs`: closed-pair inventories, conservative transfer, threshold/merged states, and checkpoint validation; `native/examples/reservoir_pair.rs` runs its bounded scenario.
- `native/src/spill_connections.rs`: standalone child–plateau incidence, direct candidate receivers, and sill-only passage queries; `native/examples/spill_connections.rs` emits its recipe-based report.
- `native/src/nested_reservoir.rs`: bounded binary-hierarchy filling, unique spill routes, exclusive active stocks, and checkpoints; `native/examples/nested_reservoir.rs` runs ordered entry pulses.
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
| Water-surface display | Wet-only caps, dry/full worlds, joint bed/water bound, visible-surface picking, hidden-water exclusion, zero-exaggeration ties, poles/seam identities |
| Initial water | Constructed basins, sills, plateaus, coverage/volume extremes, datum/area scaling, precision failure, 360 reproducible ensemble combinations, independent upstream fields, protocol corruption rejection |
| Desktop | Shared selection/data, picking across viewport resizes, view and layer changes, play/pause, recipe round trip, invalid import, cancellation and continued dynamics |
| Drainage | Constructed slopes/bowls/flats, weighted gradients and areas, 180 repeated ensemble combinations, acyclicity, terminal land-area balance, and binary corruption rejection |
| Basin analysis | Weighted nested bowls, simultaneous sills, independent threshold-component BFS, 120 synthetic and 32 generated cases, datum/area invariants, deep iterative hierarchy, unchanged upstream state |
| Isolated reservoir | Hand-computed levels/capacity, independent prism sums, exact-threshold overflow, closed boundaries, seeded pulses, precision rejection, transactional errors, JSON checkpoint continuation, CLI input bounds; no coupled-lake claim |
| Coupled pair | Receiver filling, reverse/simultaneous inputs, exact sill state, connection-area storage, symmetry, pulse partitioning, 90 seeded sequences, independent prism sums, physical-chain comparison, transactional errors, and checkpoint replay; no general network claim |
| Spill connections | No skipping lower children, nested entry regions, dead-end and alternative contacts, deterministic plateau paths, 72 independent raw-height reachability cases, deep/wide graph stress, finest generated world, unchanged water/wire state; no water-allocation claim |
| Nested filling | Near-before-far entry, source saturation, merge thresholds, exclusive stocks, 4,000 weighted oracle pulses, 500 mixed-entry/scaling/relabeling checks, JSON continuation, malformed frontiers, atomic failures, and bounded depth; ambiguous outlets rejected |
| Appearance | Review flat/globe screenshots; numerical tests alone cannot establish readable graphics |

Numerical comparisons use justified tolerances. Repeated operation in the same supported binary is exact. Native boundary rings start at a fixed incident face, avoiding an arbitrary change of starting vertex at the `atan2` ±π branch cut.

## Recipes and persistence

```json
{
  "schemaVersion": 1,
  "modelVersion": "basins-1",
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
  "detailAmplitudeMeters": 300,
  "water": { "mode": "coverage", "fraction": 0.71 }
}
```

All fields are required; unknown fields and unsupported versions are rejected. Seeds preserve their exact text and are limited to 128 UTF-16 code units. The UI shows radius in kilometers; the model stores meters. Native recipe file reads are bounded to 32 KiB. Levels 0–6 are supported. Alternatively, set `water` to `{ "mode": "volume", "volumeCubicMeters": 1.4e18 }`; never combine coverage and volume constraints. UI volume is in km³. Resolved level, stock, and coverage are retained in the native state and headless report; a saved input recipe recomputes them rather than exporting a full state archive.

`surface-1`, `surface-rust-1`, `tectonics-1`, `crust-1`, `terrain-1`, `water-1`, and `drainage-1` are not silently migrated. To explore an old seed under the new model, create a new recipe from its visible parameters and keep the old file unchanged; identical checksums are not promised. The old implementation remains available through Git history. Display-only exaggeration is not a recipe parameter.

The displayed fingerprint covers the **initial** recipe and native arrays. It is an FNV-1a regression checksum, not cryptography. Running the diagnostic deliberately does not relabel it as a live-state checksum. Save recipe regenerates the initial field at step zero. Full simulation checkpoints, rewind, and cross-version migrations are not implemented.

## Next increment

C1 supplies static drainage structure; C2a and C2b supply basin analysis and desktop inspection. C3a–C3d supply standalone geometry and reservoir experiments, including bounded binary nested filling with conserved stock and checkpoints. Next specify and test multiway/alternative-outlet allocation and pulse-order sensitivity without silently widening C3d's model. Planetary inventory assignment, a scalable dynamic representation, and dynamic-world persistence remain separate work. Resolved-world-state and image export remain outstanding milestone B work.

Keep commits coherent, messages and project records in English, and the owner's configured Git identity. Never add assistant attribution or co-author trailers.
