# Early implementation plan

Status: milestone A, the native/GPU foundation, B1 (static plate kinematics), B2 (initial crust), B3 (explainable elevation), B4 (initial water filling), and B5 (water-surface display), September 23, 2026. Water dynamics, erosion, and later simulation milestones remain proposals. Each milestone is independently demonstrable; milestone letters are not released versions.

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

B1 is implemented: connected plates, independent seed/motion streams, angular velocities, real boundary segments, relative opening/shear, and shared flat/globe layers and inspectors. B2 adds independent continentality, area fitting, and initial thickness/density. B3 adds explainable elevation and displaced globe geometry. B4 adds initial coverage/volume filling and connected water bodies. B5 adds separate globe water geometry and visible-surface picking. See [Plate kinematics](tectonics.md), [Initial crust](crust.md), [Explainable elevation](terrain.md), [Initial water](water.md), and [Water-surface display](water-surface.md). Image export, resolved-state export beyond headless reports, and prehistory integration are not implemented; B as a whole is not complete.

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

Current implementation scope is milestone A, the native/GPU integration and diagnostic transport test, B1 plate kinematics, B2 initial crust, B3 explainable elevation, B4 initial water filling, and B5 water-surface display. Remaining milestone B features follow as separate increments. Climate equations, economic modeling, and political entities must not block the first explainable terrain map.
