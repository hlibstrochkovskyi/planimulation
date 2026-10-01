# Early implementation plan

Status reviewed October 2, 2026: desktop milestones A, native/GPU foundation, B1–B5, C1 and C2a/C2b are implemented. C3a–C3g and C3i are standalone reservoir/routing experiments; C3h screens older structural restrictions on generated worlds; C3j transfers generated initial water into read-only exclusive inventories; C3k advances it in a bounded headless experiment; C3l adds a guarded larger version and records measured failures; C3m fixes a measured exact-threshold rounding failure; C3n replaces duplicated branch curves; C3o records bounded repeated-forcing and replay outcomes; C3p adds an opt-in resolution-aware level inverse; C3q records a directed event-roundoff witness without changing the solver. A separately versioned exact initial-water report and a bounded native-core prescribed-water inventory provide exact input accounting and one unambiguous spill/merge path on generated worlds. A [manual desktop view](prescribed-water-desktop.md) displays accepted prescribed steps on both projections. Timed planetary water dynamics, desktop checkpoint export/import, erosion and later simulation milestones remain unimplemented. Milestone B has current-view PNG export, but resolved-state export and prehistory integration remain open; milestone C is not complete. Milestone letters are not released versions, and no calendar schedule or percentage-complete claim is implied.

The [manual prescribed-water desktop view](prescribed-water-desktop.md) has derived regional depth/body fields, flat/globe display and explicit refusal for unsupported topology. The next product gate is desktop export/import of the exact bounded checkpoint and a clearer budget inspector; run/pause needs an explicit input sequence, not an invented climate clock. General [versioned water accounting](water-accounting-contract.md) remains a gate for arbitrary simultaneous forcing. [C3i](multi-entry-network.md) supplies a bounded explicit multiple-entry policy, [C3j](initial-water-inventory.md) maps initial water, [C3k](seeded-network.md) advances one small world with a separate initial ledger, [C3l](expanded-seeded-network.md) conditionally advances 10,242-region worlds, [C3m](exact-limit-commit.md) corrects one exact-threshold rounding failure, and [C3n](shared-storage-index.md) removes duplicated curves for bounded generated runs. [C3o](repeated-forcing-probe.md) finds eight atomic event-budget rejections in one 20-seed repeated-forcing sweep and seven level-representation rejections in a smaller-input sweep. [C3p](resolution-aware-levels.md) resolves all seven smaller-input rejections in that fixed sample with an opt-in version, while leaving the eight large-input event-budget rejections unchanged. [C3q](event-budget-roundoff.md) shows why an adjacent-float stock nudge cannot resolve a retained event witness. Laboratory test counts do not substitute for the accounting gate. See [C3h's findings and ordering](spill-readiness.md).

## 1. Selected foundation stack

The application uses Electron with a TypeScript interface and Three.js/WebGL 2 for both the flat atlas and globe. An independent Rust executable owns geometry, generation, and the diagnostic transport state. A Node.js headless adapter uses the same executable. Dependencies are recorded in npm and Cargo lockfiles.

The Rust library imports no desktop or rendering libraries. The main process owns the native process lifecycle, validates commands, and handles recipe dialogs; the sandboxed, context-isolated preload exposes named operations only. Binary native messages are bounded and versioned. Geometry is delivered once; diagnostic updates contain only a field and metadata. IPC still copies data; it is not zero-copy. A renderer Worker prepares display geometry, not simulation state. The old TypeScript generator remains an independent numerical test reference.

See [Native/GPU foundation](native-foundation.md) for implemented contracts, measurements, and limitations. A small surface/transport workload is not evidence that the complete future simulator already meets its performance goals.

The first UI uses standard DOM controls without a framework or development web server. Services, a server database, and distributed computation are unnecessary at this stage.

## 2. Minimal responsibilities

```text
core/
  random         seed, independent streams, PRNG state
  surface        sphere, topology, areas, distances
  generation     crust, plates, terrain, initial water filling
  model          state fields, units, versions
  diagnostics    invariants, statistics, checksums
adapters/
  native         start/cancel generation, bounded binary results, diagnostic frames
  headless       recipes and batch checks without a UI
viewer/
  map            projection, layers, region selection
  globe          3D view of the same data; elevation follows geological generation
  controls       parameters, legends, inspector
```

This is a responsibility map, not a requirement to create every directory immediately. Add climate, hydrology, ecology, and civilization when their milestones begin. A general event platform or plugin framework is unnecessary for the first map.

Use separate arrays keyed by stable `cell_id` for dense fields. Reuse geometry and adjacency. Neighbor lists can use compact offsets and indices. Categories use integers; continuous-field precision follows error measurements. Prefer double-precision calculations for balance totals.

Use meters for elevation and distance, m² for area, m³ for stored water, and m³/s for discharge; display conversions may use kilometers and mm/day. Converting precipitation to volume must include area. Angles, days, temperatures, and normalized indices must not silently substitute for one another.

## 3. Milestone A: geometry and reproducible recipes

Deliverable: a surface with adjacency, areas, and stable identifiers; a simple region map; recipe import/export.

Work:

1. Define configuration validation, resolved defaults, and versions.
2. Specify a PRNG and independent-stream derivation with known reference sequences.
3. Build the icosphere, dual regions, areas, and adjacency.
4. Display the flat projection, including seam handling and region selection.
5. Run the same operation headlessly and measure time/memory at several resolutions.

Acceptance: connected surface; reciprocal neighbors; two faces per edge of the closed triangulation; no duplicate vertices; positive areas summing to sphere area. The same recipe reproduces data in the selected environment. Selection near either side of the seam returns correct identifiers.

Known risks include topology near original icosahedron vertices, projection seams, poles, and identifier ordering. Resolve these before implementing water transport.

## 4. Milestone B: first visual generator

B1 is implemented: connected plates, independent seed/motion streams, angular velocities, real boundary segments, relative opening/shear, and shared flat/globe layers and inspectors. B2 adds independent continentality, area fitting, and initial thickness/density. B3 adds explainable elevation and displaced globe geometry. B4 adds initial coverage/volume filling and connected water bodies. B5 adds separate globe water geometry and visible-surface picking. See [Plate kinematics](tectonics.md), [Initial crust](crust.md), [Explainable elevation](terrain.md), [Initial water](water.md), and [Water-surface display](water-surface.md). The desktop can export the currently visible atlas or globe view to PNG. Resolved-state export beyond headless reports and prehistory integration are not implemented; B as a whole is not complete.

Deliverable: the user changes a seed and parameters and receives structured terrain, oceans, and geological layers.

Work:

1. Continentality, plates, relative motion, and boundary types.
2. Large-scale relief and bounded spherical noise detail.
3. Initial water-level fitting and water-component classification.
4. Map, legends, zoom/pan, and an elevation-contribution inspector.
5. Native generation with progress, cancellation, and protection against stale-result publication.
6. Export the resolved recipe and map image; report statistics and field checksums.

Check reproduction using data, not screenshots: rendering antialiasing may differ. Do not expose a control as functional when its mechanism is absent.

Acceptance: identical recipes reproduce; seeds produce variety; changing relief while holding other conditions fixed has a visible effect; boundaries and noise have no map-seam discontinuity; inspector units and contributions are correct. A canceled task cannot replace a newer world.

The first fixed ensemble might contain 20 seeds at moderate resolution. This is a smoke check for diversity and errors, not statistical proof of quality. Record water coverage, connected-component sizes, elevation distributions, time, and memory. A failed seed becomes a regression fixture rather than being silently excluded.

Completion produces the first usable artifact to open and explore. Rivers, biomes, and temperature are not yet presented as computed outputs.

## 5. Milestone C: catchments, lakes, and terrain preparation

[C1 drainage structure](drainage.md) is implemented: bed receivers, deterministic flat routing, terminal catchments, and contributing dry-land area. It preserves closed sinks and does not alter terrain or water. Current desktop recipes use `basins-1`, protocol 8; milestone C is not complete.

[C2a basin analysis](basins.md) adds a Rust connectivity hierarchy, merge thresholds, and level–storage queries. [C2b inspection](basin-inspection.md) integrates its unchanged `basin-analysis-1` algorithm into generation, versioned binary transport, two map/globe layers, and a hierarchy inspector. [C3a](reservoir-experiment.md)–[C3e](spill-junction.md) supply storage, geometric connection and allocation experiments. [C3f](spill-network.md) combines nested storage and multiple plateaus in a bounded ordered network; [C3g](simultaneous-network.md) adds constant concurrent forcing. [C3h](spill-readiness.md) measures generated-world restrictions, [C3i](multi-entry-network.md) supplies a bounded explicit policy for different receiving leaves, [C3j](initial-water-inventory.md) maps generated initial water to read-only exclusive stocks, [C3k](seeded-network.md) advances those stocks in a bounded headless run, [C3l](expanded-seeded-network.md) adds guarded larger runs, [C3m](exact-limit-commit.md) resolves one measured arithmetic failure, [C3n](shared-storage-index.md) removes duplicated curves, [C3o](repeated-forcing-probe.md) records repeatability and numerical failures, and [C3p](resolution-aware-levels.md) addresses the smaller-input level representation failure. These are standalone tools, not desktop/planet-scale water dynamics. Pending-input storage, splitting, robust long-running forcing and erosion remain unimplemented.

[Prescribed runoff routing](prescribed-runoff-routing.md) is a native-core input stage on generated worlds: exact integer volumes follow existing static drainage receivers and balance at canonical wet-body or closed-sink terminals. It borrows only the downhill-accumulation stage of Fill–Spill–Merge. An opt-in [bounded inventory](prescribed-water-inventory.md) transfers those inputs to active basin stocks, conservatively spills through a unique open geographic passage, merges full siblings, and checkpoints the changed frontier. The [desktop view](prescribed-water-desktop.md) displays manual accepted steps on flat and globe projections. It rejects multiple direct spill sources or multiple open receivers; it does not implement changing drainage, climate forcing, or timed playback.

Deliverable: correct drainage structure and a demonstration of basin filling under prescribed water input.

Work includes flow directions, flats, spill thresholds, depression hierarchy, storage/overflow, and bounded erosion with material accounting. Layers include catchments, depressions, lake levels, and drainage potential.

Acceptance: water cannot move uphill without an appropriate water-surface level; routing has no endless cycles; a closed bowl stores water; overflow starts at its threshold; nested basins and flats work; input, storage, and outflow agree within a specified tolerance.

Use small constructed terrains: a slope, a bowl, a bowl with a sill, nested bowls, a flat with an outlet, and a closed basin. These expose behavior that is difficult to diagnose on a random planet.

Prescribed rain at this stage is an experimental input. Climate-derived river discharge is not yet claimed.

## 6. Milestone D: seasonal climate and water cycle

Deliverable: run, pause, and observe several years; rivers receive computed water inputs.

Work includes seasonal heating, thermal memory, a tangent wind field, evaporation, moisture transport, precipitation, snow, soil and simplified groundwater stores, runoff, and lakes. Initial state preparation has a criterion and an iteration limit. Introduce coherent weather disturbances after establishing the baseline seasonal behavior.

Acceptance: a complete recorded water budget, including any external reservoir; no negative stocks or NaNs; consistent shared-edge fluxes; changing resolution or decreasing the step does not cause uncontrolled qualitative changes.

Directed experiments include stopping precipitation, recovery after drought, snowmelt, moisture transport across a constructed mountain barrier, and changing thermal inertia. Expectations apply to the simplified model and are not presented as universal climate laws.

Daily values, monthly totals, and long-term averages have separate labels. Playback speed and frame rate do not alter simulation outcomes.

## 7. Milestone E: ecology and explanations

Deliverable: vegetation and soil respond with delays; biomes describe accumulated conditions. The inspector exposes time series and recorded drivers of change.

Work includes growth, water/temperature limitations, recovery, water-retention and erosion feedback, and natural productivity for future food supply. Geological deposits with independent randomness follow later.

Acceptance: biomes do not flicker with every short fluctuation; stable conditions produce bounded states; drought leaves a measurable effect; recovery takes time. Run ensembles of ordinary and extreme supported parameters, retaining distributions and problematic seeds.

Preserve differences between worlds. Checks detect broken laws rather than requiring every planet to have the same forest area.

## 8. Adding 3D

The native/GPU foundation provides a globe and flat map sharing layers and `cell_id`. B3 adds the same computed elevation to both, with displaced globe geometry and display-only exaggeration. B4 adds shared water-depth and body-ID layers coloring the bed. B5 adds separate water-surface geometry in the Land and water layer, using a documented shoreline approximation without changing physical coverage. Climate completion is not a prerequisite. No detailed 3D cities or street-level environment are required; settlements and routes will be analytical overlays.

Acceptance: a selected region has identical data on the map and globe; view switching leaves the model hash unchanged; vertical exaggeration does not affect measurements. Globe LOD can follow measured need; a detailed local environment is outside the current scope.

## 9. Determinism and persistence

A recipe records generator/model versions, seed, PRNG algorithm, resolution, all resolved parameters, and fitted values. Preserve both the user's requested targets and the actual parameters, including their relationship.

From the start of dynamics, provide complete checkpoints: schema version, step and calendar, state arrays, PRNG states, accumulators, and necessary queues. The main test is that `A → save → load → B` matches an uninterrupted `A → B` in the supported environment.

Do not silently resume incompatible versions. Provide an explicit migration or a clear rejection while preserving original data. A version update does not promise that an old seed creates the same world.

History snapshots and full rewind follow reliable checkpoints. An exported image is not a state save.

## 10. Quality, performance, and the next layer

Numerical tolerances and performance budgets remain open until implementation exists. During milestone A, benchmark the target device, choose a baseline resolution, and record acceptable generation time, UI responsiveness, and memory. Do not promise millions of regions or millennia per second without measurements.

Automated checks focus on topology, units, balances, fluxes, determinism, checkpoint continuation, and directed experiments. Visual review complements them with seam checks, layer readability, and geographic structure. An attractive image does not replace conservation checks.

Population readiness means the environment provides water, wild food, seasonal hazards, traversal costs, and change history with stable behavior across an ensemble. Then introduce the groups described in [DESIGN.md](../DESIGN.md), consuming real stocks and modifying their environment.

Current implementation scope is milestone A, the native/GPU integration and diagnostic transport test, B1–B5 generation/display, C1 drainage, C2a/C2b basin analysis/inspection, C3a–C3g standalone routing experiments, C3h generated-world screening, C3i bounded multiple-entry allocation, C3j read-only initial-inventory transfer, C3k bounded headless continuation, C3l guarded larger runs, C3m's versioned exact-endpoint correction, C3n's shared storage index, C3o's bounded stress probe, C3p's opt-in level inverse, C3q's event-roundoff diagnostic, and the separate bounded manual prescribed-water desktop path. C3h found conflicting nested receiving entries in all twenty sampled subdivision-3 worlds; C3i accepts such topology with explicit weights on bounded extracted subtrees. At 10,242 regions, the fixed 20-seed sample accepted all twenty v4 **single** intervals, but only twelve completed C3o's 100 distributed large-input intervals; eight rejected an internal event budget atomically. The smaller-input sweep completed thirteen in v4 and all twenty in opt-in v5. The event-budget residual and eventual small-stock precision remain gates before reliable continuous forcing. The desktop path has explicit rejection behavior and is not general water dynamics. Remaining milestone B features follow as separate increments. Climate, economy and political entities must not block the current natural-world foundation.
