# Early implementation plan

Status: milestone A implementation, September 19, 2026. Later milestones remain proposals. Each milestone is independently demonstrable; milestone letters are not released versions.

## 1. Proposed stack

The first application uses Electron with a TypeScript renderer and a Web Worker for generation. A Node.js headless adapter uses the same core. Dependencies are pinned in the package manifest and lockfile. The current flat map uses Canvas 2D; Three.js is deferred until the 3D milestone needs it.

The core imports neither Electron, DOM/Worker APIs, nor rendering libraries. The Worker is an adapter. The desktop main process owns the window and native recipe dialogs; a sandboxed, context-isolated preload exposes two narrow recipe operations. GPU simulation or a native core is considered after benchmarks and reproducibility checks.

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
  worker         start/cancel generation, deliver results
  headless       recipes and batch checks without a UI
viewer/
  map            projection, layers, region selection
  globe          later: 3D view of the same data
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

Deliverable: the user changes a seed and parameters and receives structured terrain, oceans, and geological layers.

Work:

1. Continentality, plates, relative motion, and boundary types.
2. Large-scale relief and bounded spherical noise detail.
3. Initial water-level fitting and water-component classification.
4. Map, legends, zoom/pan, and an elevation-contribution inspector.
5. Worker generation with progress, cancellation, and protection against stale-result publication.
6. Export the resolved recipe and map image; report statistics and field checksums.

Check reproduction using data, not screenshots: rendering antialiasing may differ. Do not expose a control as functional when its mechanism is absent.

Acceptance: identical recipes reproduce; seeds produce variety; changing relief while holding other conditions fixed has a visible effect; boundaries and noise have no map-seam discontinuity; inspector units and contributions are correct. A canceled task cannot replace a newer world.

The first fixed ensemble might contain 20 seeds at moderate resolution. This is a smoke check for diversity and errors, not statistical proof of quality. Record water coverage, connected-component sizes, elevation distributions, time, and memory. A failed seed becomes a regression fixture rather than being silently excluded.

Completion produces the first usable artifact to open and explore. Rivers, biomes, and temperature are not yet presented as computed outputs.

## 5. Milestone C: catchments, lakes, and terrain preparation

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

After milestone B, add a globe as a short independent milestone: the same elevation, ocean, layers, and `cell_id`, with different viewing geometry and camera. Climate completion is not a prerequisite.

Acceptance: a selected region has identical data on the map and globe; view switching leaves the model hash unchanged; vertical exaggeration does not affect measurements. Detailed local terrain and decorative LOD come later.

## 9. Determinism and persistence

A recipe records generator/model versions, seed, PRNG algorithm, resolution, all resolved parameters, and fitted values. Preserve both the user's requested targets and the actual parameters, including their relationship.

From the start of dynamics, provide complete checkpoints: schema version, step and calendar, state arrays, PRNG states, accumulators, and necessary queues. The main test is that `A → save → load → B` matches an uninterrupted `A → B` in the supported environment.

Do not silently resume incompatible versions. Provide an explicit migration or a clear rejection while preserving original data. A version update does not promise that an old seed creates the same world.

History snapshots and full rewind follow reliable checkpoints. An exported image is not a state save.

## 10. Quality, performance, and the next layer

Numerical tolerances and performance budgets remain open until implementation exists. During milestone A, benchmark the target device, choose a baseline resolution, and record acceptable generation time, UI responsiveness, and memory. Do not promise millions of regions or millennia per second without measurements.

Automated checks focus on topology, units, balances, fluxes, determinism, checkpoint continuation, and directed experiments. Visual review complements them with seam checks, layer readability, and geographic structure. An attractive image does not replace conservation checks.

Population readiness means the environment provides water, wild food, seasonal hazards, traversal costs, and change history with stable behavior across an ensemble. Then introduce the groups described in [DESIGN.md](../DESIGN.md), consuming real stocks and modifying their environment.

Current implementation scope is milestone A. Milestone B follows as a separate increment. Climate equations, economic modeling, and political entities must not block the first explainable terrain map.
