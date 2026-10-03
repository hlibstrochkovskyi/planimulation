# Development and validation

## Current increment

Milestone A establishes spherical geometry, recipes, and inspection. The September 20 native/GPU increment moves desktop and headless calculations to Rust and adds two Three.js views: a flat atlas and a globe. Both display the same region IDs, fields, and selection. B3 now displaces the globe using computed elevation.

Milestone B1 adds static plate kinematics. B2 adds independent continentality, approximate crust thickness/density, and area fitting. B3 adds inspectable elevation contributions, distance-decayed boundary effects, and globe relief. B4 adds initial water filling; B5 adds separate globe water surfaces. C1 adds drainage structure; [C2b basin inspection](basin-inspection.md) integrates the hierarchy with model `basins-1`. Protocol 8 adds a [manual prescribed-water desktop view](prescribed-water-desktop.md) without changing that recipe or initial world arrays. The current `terrain-prep-1` generator adds bounded dry preparation and protocol-10 world arrays while retaining `basins-1`/protocol-9 compatibility. Timed water dynamics, ongoing or climate-driven erosion, and geological time integration are not implemented yet.

The opt-in `continuous-plates-1` recipe selects mesh-independent plate-root directions and keeps the same prepared-world protocol-10 layout. It is not the desktop default or a validated geological model. Existing `terrain-prep-1` and `basins-1` recipes remain unchanged.

The diagnostic diffusion mode exists to exercise stateful native calculation, small dynamic messages, and responsive rendering. It is not a climate, erosion, or geological model. Its step number is not a calendar. Read [Native/GPU foundation](native-foundation.md) for the protocol, numerical definition, and limitations. The original [milestone A report](validation-milestone-a.md) is historical and describes the superseded TS/Canvas implementation.

[C2a basin analysis](basins.md) supplies the native bed-connectivity hierarchy, merge thresholds, and prism-storage queries. C2b computes it during generation and exposes branch/threshold layers and hierarchy inspection. A standalone report is also available: `cargo run --release --locked --manifest-path native/Cargo.toml --example basins -- path/to/recipe.json` (or `-` for stdin), using any supported recipe version.

[C3a reservoir experiments](reservoir-experiment.md) add isolated leaf storage, prescribed volume pulses, an optional external collector, and complete experiment checkpoints. They do not modify generated world water or the desktop. Run `cargo run --release --locked --manifest-path native/Cargo.toml --example reservoir -- docs/scenarios/reservoir-sill.json` for a hand-verifiable example.

[C3b coupled pairs](reservoir-pair.md) add conservative exchange across one declared sill, receiver filling, and common storage above it, in a closed developer experiment. Run `cargo run --release --locked --manifest-path native/Cargo.toml --example reservoir_pair -- docs/scenarios/reservoir-pair.json`. That experiment does not route through a hierarchy.

[C3c spill connections](spill-connections.md) derive the geometric child–plateau graph, all contacts, candidate receiving branches, and potential region passages from the bed. Run `cargo run --release --locked --manifest-path native/Cargo.toml --example spill_connections -- docs/scenarios/spill-connections.json`. This does not choose between recipients or move water.

[C3d nested filling](nested-reservoir.md) combines explicit passages and active storage in a closed, initially dry binary-hierarchy experiment of at most 128 regions. Run `cargo run --release --locked --manifest-path native/Cargo.toml --example nested_reservoir -- docs/scenarios/nested-reservoir.json`. Ambiguous outlets are rejected; this is not planetary hydrology or a desktop control.

[C3e shared-sill allocation](spill-junction.md) tests explicit capped receiver weights and simultaneous input batches in a separate single-plateau leaf junction. Run `cargo run --release --locked --manifest-path native/Cargo.toml --example spill_junction -- docs/scenarios/spill-junction.json`. It does not change C3d's supported topology or infer hydraulic conductance from mesh contacts.

[C3f spill networks](spill-network.md) combine nested entry, distinct sill plateaus, saturated transit and event-driven weighted frontiers. Run `cargo run --release --locked --manifest-path native/Cargo.toml --example spill_network -- docs/scenarios/spill-network.json`. Inputs are ordered and can be history-dependent; they are not simultaneous rainfall. Ambiguous nested entries and unrepresentable numerical updates fail atomically.

[C3g concurrent forcing](simultaneous-network.md) separately supplies constant regional inputs over normalized intervals, advancing all active stocks to global saturation events. Run `cargo run --release --locked --manifest-path native/Cargo.toml --example simultaneous_network -- docs/scenarios/simultaneous-network.json`. Input-list order is not a source priority; rates are per normalized interval, not physical discharge.

[C3h integration screening](spill-readiness.md) checks the existing region bound and nested-entry restriction against generated beds without constructing or running the bounded solver. Run `cargo run --release --locked --manifest-path native/Cargo.toml --example spill_readiness -- docs/scenarios/spill-connections.json`. Passing these necessary checks does not certify dynamic readiness.

[C3i multiple entries](multi-entry-network.md) adds a separately versioned branch-then-entry weight policy to bounded concurrent forcing. Run `cargo run --release --locked --manifest-path native/Cargo.toml --example multi_entry_network -- docs/scenarios/multi-entry-network.json`. It accepts different internal receiving leaves and retains the old C3f/C3g contracts.

The [manual prescribed-water desktop view](prescribed-water-desktop.md) accepts one named runoff action on a selected region, sends it to the exact native inventory, and renders separate regional water levels on the flat map and globe. It refuses unsupported routes and does not run on a clock. `Save water checkpoint` and `Open water checkpoint` export, validate, restore, and continue its separate exact version-2 JSON state. `Inspect exact budget` reconciles the active stocks and highlights the selected source's frozen-graph destination without changing physics.

`Export world data` writes a [resolved initial-world report](resolved-world-export.md) from the accepted native generation state. It includes the generated physical fields and topology, but neither manual water continuation nor diagnostic playback. This report is not an importable checkpoint.

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
- `native/src/spill_junction.rs`: shared-sill leaf allocation with explicit weights, capped redistribution, simultaneous batches, and checkpoints; `native/examples/spill_junction.rs` emits the developer report.
- `native/src/spill_network.rs`: bounded event-driven receiving frontiers, nested storage, saturated transit witnesses, ordered input and checkpoints; `native/examples/spill_network.rs` emits the network experiment report.
- `native/src/spill_network/simultaneous.rs`: separately versioned concurrent forcing, global saturation events and audited interval transactions; `native/examples/simultaneous_network.rs` emits the developer report.
- `native/src/spill_readiness.rs`: read-only generated-bed restriction screening, conflicting entry witnesses and logical storage counts; `native/examples/spill_readiness.rs` emits the recipe-based report.
- `native/src/spill_network/simultaneous/multi_entry.rs`: explicit branch and inner-entry weights, reachable-entry witnesses and bounded concurrent intervals; `native/examples/multi_entry_network.rs` emits its developer report.
- `native/src/prescribed_water_inventory.rs`: exact bounded stocks, unique geographic spill, merge, checkpoint, and derived display fields.
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
| Desktop | Shared selection/data, picking across viewport resizes, view and layer changes, play/pause, manual prescribed-water spill display on both projections, recipe round trip, bounded PNG export of flat/globe views, invalid import, cancellation and continued diagnostics |
| Drainage | Constructed slopes/bowls/flats, weighted gradients and areas, 180 repeated ensemble combinations, acyclicity, terminal land-area balance, and binary corruption rejection |
| Basin analysis | Weighted nested bowls, simultaneous sills, independent threshold-component BFS, 120 synthetic and 32 generated cases, datum/area invariants, deep iterative hierarchy, unchanged upstream state |
| Isolated reservoir | Hand-computed levels/capacity, independent prism sums, exact-threshold overflow, closed boundaries, seeded pulses, precision rejection, transactional errors, JSON checkpoint continuation, CLI input bounds; no coupled-lake claim |
| Coupled pair | Receiver filling, reverse/simultaneous inputs, exact sill state, connection-area storage, symmetry, pulse partitioning, 90 seeded sequences, independent prism sums, physical-chain comparison, transactional errors, and checkpoint replay; no general network claim |
| Spill connections | No skipping lower children, nested entry regions, dead-end and alternative contacts, deterministic plateau paths, 72 independent raw-height reachability cases, deep/wide graph stress, finest generated world, unchanged water/wire state; no water-allocation claim |
| Nested filling | Near-before-far entry, source saturation, merge thresholds, exclusive stocks, 4,000 weighted oracle pulses, 500 mixed-entry/scaling/relabeling checks, JSON continuation, malformed frontiers, atomic failures, and bounded depth; ambiguous outlets rejected |
| Shared-sill allocation | Weighted/equal splits, capped receivers, local-first simultaneous inputs, 2,400 oracle updates, order/partition comparisons before merging, distinct transfer histories, repeated contacts, 127 leaves, C3b pair agreement, atomic errors, and exact continuation |
| Spill network | Frontier changes at first saturation, blocked/long transit, cycles/alternatives, nested entry, 1,200 analytic oracle updates, 60 independent raw-region reachability cases, explicit order sensitivity, C3d/C3e agreement, exact continuation and atomic precision rejection |
| Concurrent forcing | Input permutation, proportional subdivision, relabeling, 100 analytical weighted forks, 60 independent component-rate oracles, nested entry, multiple sources, threshold ties, C3e/C3f limits, exact continuation, and atomic failures |
| Integration screening | Witness paths, dead-end exclusion, 60 bounded solver-admission comparisons, independent ancestry counts, deep iterative hierarchy, generated worlds through subdivision 6, unchanged wire state, and water-setting independence |
| Multiple entries | Two-stage weighted oracle, distinct-sill gating, repeated-contact deduplication, inner/outer thresholds, exact continuation, C3g agreement, relabeling, 127-region bound, cycles, atomic failures and generated conflicting subtrees |
| Seeded expansion | Separate initial/external ledgers, generated provenance, v1/v2/v3 replay, exact-endpoint arithmetic witness, unchanged legacy rejection, guarded 10,242-region sample and explicit storage-cap failures; no desktop water or long forcing claim |
| Shared storage | Single region-ID index, weighted-prism comparison, v1–v4 replay, previously capped generated seeds, 20-seed release profile and short distributed-input continuation; no long-running planetary claim |
| Repeated forcing probe | Cyclic distributed profiles, complete-checkpoint replay after a selected interval, first-failure/atomicity reporting, two retained 20-seed forcing sweeps, event-budget and level-representation witnesses, and a sub-precision rejection; no generalized forcing claim |
| Appearance | Review flat/globe screenshots; numerical tests alone cannot establish readable graphics |

Numerical comparisons use justified tolerances. Repeated operation in the same supported binary is exact. Native boundary rings start at a fixed incident face, avoiding an arbitrary change of starting vertex at the `atan2` ±π branch cut.

## Recipes and persistence

```json
{
  "schemaVersion": 1,
  "modelVersion": "terrain-prep-1",
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
  "terrainPreparationPasses": 4,
  "water": { "mode": "coverage", "fraction": 0.71 }
}
```

All fields for the selected model version are required; unknown fields and unsupported versions are rejected. `terrainPreparationPasses` accepts integers 0–16 in `terrain-prep-1` and `continuous-plates-1`. Existing `basins-1` recipes must omit it and preserve their original physical fields and fingerprint. Seeds preserve their exact text and are limited to 128 UTF-16 code units. The UI shows radius in kilometers; the model stores meters. Native recipe file reads are bounded to 32 KiB. Levels 0–6 are supported. Alternatively, set `water` to `{ "mode": "volume", "volumeCubicMeters": 1.4e18 }`; never combine coverage and volume constraints. UI volume is in km³. Resolved level, stock, and coverage are retained in the native state and headless report; a saved input recipe recomputes them. Export world data records generated initial fields separately without creating a resumable state archive.

`surface-1`, `surface-rust-1`, `tectonics-1`, `crust-1`, `terrain-1`, `water-1`, and `drainage-1` are not silently migrated. `basins-1` is still supported and is never implicitly upgraded when opened or saved. `continuous-plates-1` can be opened, saved, exported, and run headlessly, but the desktop form has no model selector: submitting it creates a new default `terrain-prep-1` recipe using the displayed parameters. To explore a seed under another model, keep the original file unchanged; identical checksums are not promised. Display-only exaggeration is not a recipe parameter.

The displayed fingerprint covers the **initial** recipe and native arrays. It is an FNV-1a regression checksum, not cryptography. Running the diagnostic or manually adding prescribed water deliberately does not relabel it as a live-state checksum. Save recipe regenerates the initial field and initial water at step zero. Export PNG saves the currently visible atlas or globe view, including display-only settings and visible clipping; it does not save a complete map, physical state, or checkpoint. The desktop's separate water-checkpoint file contains the exact bounded manual inventory and its pinned generation recipe, but not diagnostic playback, camera state, climate, or a general simulation checkpoint. Rewind and cross-version migrations are not implemented.

## Next increment

The [first seasonal-temperature increment](seasonal-temperature.md) is a separate read-only climate derivation: twelve periodic monthly normals computed from initial latitude, elevation, and water coverage, with a native report and desktop layer. The [prescribed surface-wind increment](seasonal-wind.md) adds twelve east/north vector normals and a speed layer. Neither advances the world, alters manual water stocks, transports moisture, produces precipitation, or meets milestone D's full climate/water-cycle gates.

The [moisture-transport increment](moisture-transport.md) adds a conservative native column-water kernel and bounded headless runs driven by those seasonal winds. Shared barycentric-dual boundaries, outgoing-rate substeps, explicit stock budgets, and analytic rotation tests are implemented. Its original report preserves declared artificial column stocks without evaporation or precipitation. The desktop still displays static seasonal normals, and milestone D remains incomplete.

The separate [finite seasonal-moisture increment](seasonal-moisture.md) implements bounded coupling from an initially empty atmosphere and a finite active partition of generated water. [Typed snow/liquid/soil exchange](surface-water.md) supplies formed runoff; [delayed receiver-edge routing](runoff-transport.md) transfers it to finite terminal water that can return to vapor. Clock-aligned hourly-or-smaller coupling, graph/stock ledgers, complete six-stock budgets, matched disabled controls, and schema-3 same-build checkpoint continuation are implemented. It runs headlessly; lake levels/spills, groundwater, full basin ownership, changing shoreline, calibration, and desktop playback remain work. Moisture schemas 1/2 are deliberately rejected; source-free transport reports and exact manual water-inventory behavior are unchanged. Prior evidence is retained in the [version-2 record](seasonal-moisture-2.md).

C1 supplies static drainage structure; C2a/C2b supply analysis and desktop inspection; C3a–C3g remain standalone routing experiments. C3h measures generated-world restrictions; C3i separately resolves multiple nested receiving entries under explicit weights in bounded experiments. C3j–C3n import initial water, run bounded generated worlds, correct one measured exact-threshold failure, and remove duplicated branch curves in a versioned representation. C3o's two 100-interval forcing sweeps retain event-budget and level-representation failures separately. The manual desktop view now displays bounded prescribed-water steps, supports exact water-only checkpoint continuation, and inspects its exact budget; climate coupling remains separate product work. [Bounded dry terrain preparation](terrain-preparation.md) now changes new generated beds before initial water and has material-accounting tests. General simultaneous forcing still needs an explicit ownership and event-roundoff policy; sub-precision pending input remains another distinct task. Milestone B now has initial-field JSON export and current-view PNG export, while geological prehistory integration remains open.

Keep commits coherent, messages and project records in English, and the owner's configured Git identity. Never add assistant attribution or co-author trailers.
