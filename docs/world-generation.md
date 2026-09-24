# World generation: technical proposal

Initial proposal: September 19, 2026. The spherical surface, recipes, random streams, GPU flat/globe views, B1 static plate kinematics, B2 initial crust, B3 explainable elevation, B4 initial water filling, B5 water-surface display, C1 static drainage structure, and C2a–C2b basin analysis and inspection are implemented. Water dynamics, erosion, climate, and ecology remain proposals. See [DESIGN.md](../DESIGN.md), [development](development.md), [Plate kinematics](tectonics.md), [Initial crust](crust.md), [Explainable elevation](terrain.md), [Initial water](water.md), [Water-surface display](water-surface.md), [Drainage structure](drainage.md), and [Basin inspection](basin-inspection.md) for the concept and implemented algorithms with their limitations.

## 1. One world, multiple views

Model a closed spherical surface from the outset. A flat map projects its data; a globe displays another view. A detailed local environment is outside the current scope.

```text
seed + resolved parameters + versions
                  ↓
       surface and state fields
          ↙       ↓       ↘
       2D map   3D globe   headless analysis
```

The simulation does not read map pixels or use screen-space distances. Colors, lighting, vertical exaggeration, and camera position do not alter world state.

## 2. Surface representation

The implemented surface is a recursively subdivided icosahedron with vertices normalized onto the unit sphere. Each vertex is the center of a computational region. A barycentric dual region is assembled around it from parts of adjacent spherical triangles.

The core owns topology construction and produces stable identifiers and neighbor ordering without using a rendering library's mesh constructor.

For recursive four-way triangle subdivision, the number of centers is `10 * 4^L + 2`: level 5 gives 10,242 regions; level 6 gives 40,962. These are initial benchmark candidates, not promises of adequate detail or speed. Debugging uses smaller levels.

Regions are not exactly equal in area. Store each center's direction, area, neighbors, and distances. Shared boundaries also need length and orientation when transport is introduced. Areas must sum to `4πR²`. Physical region size depends on planet radius and resolution.

Wind is a vector tangent to the sphere. Neighbor transport uses shared-edge geometry. Local east/north components at different locations cannot be combined without transformation.

Alternatives include cubed-sphere grids for regular blocks and latitude/longitude grids for straightforward rasters. The proposed mesh avoids a special concentration of cells at the poles and provides a graph for future routes, but makes indexing and flux calculations more involved. The first geometry prototype evaluates that cost.

Changing computational resolution during a history is initially unsupported. Rendering LOD does not change computational regions. Changing simulation resolution creates a new recipe and regenerates the world.

## 3. User controls

Expose controls only for implemented mechanisms. Do not add decorative sliders for absent physics.

| Control | Generator meaning | Availability |
| --- | --- | --- |
| Seed | Reproducible initial randomness | First map |
| Resolution | Region count and computation cost | First map |
| Radius | Physical distances and areas | First map |
| Water | Target total water fraction **or** specified volume | Implemented B4 |
| Continental structure | Spatial parameters of crust distribution | First map |
| Relief | Scale of uplift/depressions and boundary effects | First map |
| Geological detail | Plate count, detail scales, and amplitudes | Advanced first-map controls |
| Axial tilt | Seasonal heating cycle | Climate |
| Thermal regime | Parameters of the approximate thermal model | Climate |
| Weather variability | Disturbance amplitude and correlation time | Dynamics |

Archipelago and supercontinent presets resolve to explicit crust parameters; they are not guarantees of a particular result. Measure and display actual continent sizes. Fully expand presets in saved recipes.

Mean temperature, actual ocean coverage, biome areas, mean precipitation, and river counts are outputs. A future target-temperature control would require explicit fitting of heating conditions.

The first climate family may fix day/year duration, atmospheric composition, and orbital conditions. Record those fixed values too. Extending their ranges requires implementing the associated dependencies.

## 4. Generation sequence

### A. Recipe validation and randomness

Validate types, finite values, supported ranges, and mutually compatible modes. Resolve defaults. Derive separate PRNG streams for geology, terrain detail, future resources, and weather from the seed and stable stream names using a specified mixing algorithm.

Geometry depends on its version and resolution. Continental structure and detail use their own streams. Rendering consumes no simulation randomness. Heaps, sorting, and equal-cost choices use a stable secondary key such as `cell_id`.

### B. Approximate plates and crust

The implemented B1 subset uses a positive-cost multi-source graph partition to guarantee connected plates, rigid angular velocities, and local shared-edge relative motion. B2 adds an independent spherical continentality field with area fitting and approximate thickness/density. B3 consumes these for approximate elevation; neither plate identity nor crust dominance implies land or ocean.

Choose distributed plate centers on the sphere. Partition by spherical distance, optionally applying bounded boundary deformation while preserving plate connectivity. Generate a coherent large-scale continentality field: one plate may include different crustal regions.

Assign each plate an approximate rotation pole and angular velocity. Point velocity follows `v = ω × r`. At a boundary, compute relative velocity components across and along the boundary.

B3 uses convergence and divergence conditioned on continentality. Pure shear currently has no vertical effect. Approximate crust density is available; age and subduction polarity are not. They can later refine relief and resource-related geological features.

This is a static tectonics-inspired generator. It does not integrate millions of years of plate motion or establish scientifically accurate geological ages.

### C. Elevation

Implemented B3 uses a crust buoyancy baseline, strongest-source exponential boundary envelopes over physical graph distance, and independent bounded spherical detail. See [Explainable elevation](terrain.md) for exact formulas, controls, limitations, and checks. Erosion is not yet included.

Combine a crustal baseline, spatially distributed boundary effects, and bounded local detail. Boundary contributions decay with distance according to structure type, encouraging coherent mountain belts.

Sample a three-dimensional noise field at points on the sphere. Version its algorithm and scales. This avoids having to join opposite edges of a planar noise image. High-frequency details have smaller amplitudes and must not erase the large-scale structure.

Retain inspector contributions: crust baseline, boundary effects, detail, and later erosion. Their sum reconstructs the generated elevation within numerical tolerance.

### D. Ocean and sea level

B4 implements initial coverage/volume fitting and component classification. See [Initial water](water.md) for exact plateau/tie/full-coverage conventions, stock tolerances, and the analytical bed view. Dynamic basin budgets and erosion refitting remain future integration work.

Support two mutually exclusive modes: water volume or target coverage. In coverage mode, find a level using area-weighted regions, infer the corresponding volume, and retain the resulting physical recipe.

The early approximation estimates volume as `Σ area_i * max(0, level - bed_i)`. Error reflects terrain discretization and should decrease with refinement.

Initial equilibrium filling may leave disconnected water bodies. Classify connected components explicitly: the largest is the main ocean; others are inland water basins. A shared initial level is an initialization condition, not a permanent connection between isolated basins. Their dynamic budgets subsequently evolve separately.

In the first version, the ocean-coverage target means coverage under initial global filling. The inspector separately reports the main ocean and other water areas. Make this limitation explicit in the UI. Targeting the exact area of the largest connected ocean requires another fitting procedure and is not initially promised.

After erosion, preserve the chosen water volume and recompute the level. Do not restore the requested percentage by silently adding water.

### E. Catchments and depressions

[C1 drainage structure](drainage.md) implements bed-based receivers, equal-height routing, terminal catchments, and contributing land-area analysis without modifying terrain. [C2a basin analysis](basins.md) adds a native connectivity hierarchy, merge thresholds, and storage queries; [C2b inspection](basin-inspection.md) integrates them into generation and the desktop. Dynamic inventories, overflow routing, and erosion below remain proposals.

Identify downhill directions, flats, depressions, spill thresholds, and connections. Priority-Flood methods are candidates for spill analysis; retain physical terrain separately from auxiliary routing elevations.

Preserve physical depressions. Each needs area/volume relationships by water level and spill connections. Nested and merging depressions require a hierarchy; simply banning cycles in a river list does not solve this problem.

Early analysis can display potential drainage under unit test input. Label it drainage potential, not actual river discharge. Actual discharge requires precipitation, evaporation, and stored water.

Closed basins retain water and lose it through evaporation/infiltration. Spilling starts only at the appropriate level. Do not force every river to reach the ocean.

### F. Erosion and terrain preparation

Start with bounded material transfer on excessively steep slopes. Later add simplified erosion along runoff paths, including transport and deposition. Removed material must be recorded as transported, deposited, or exported from the modeled reservoir.

Early preparation may use explicitly hypothetical moisture input. Once climate exists, erosion can use its mean runoff. Significant terrain changes require recomputing catchments and water levels.

Do not mix thousands of abstract geological iterations with the observed calendar. Limit preparation passes; do not iterate indefinitely until the map appears attractive.

### G. Climate, water, and state preparation

Terrain and drainage structure can precede climate, but river filling and ecology require coupled calculations. Proposed order:

1. Establish seasonal heating, thermal inertia, and simplified wind belts.
2. Evaporate water from available stocks and transport atmospheric moisture across mesh edges.
3. Compute precipitation from transported moisture, including a terrain-lifting dependency; subtract precipitation from the atmospheric store.
4. Allocate water to snow, soil, surface storage, and groundwater.
5. Route runoff and update lakes, evaporation, and slow vegetation state.
6. Repeat seasonal cycles until a bounded settling criterion or preparation limit is reached.

Compare corresponding phases of successive years when evaluating settling, not neighboring days: the seasonal cycle must remain. Disable stochastic weather initially to diagnose the seasonal model, then enable it to examine stable statistics.

Record preparation time separately from observation time. If the criterion is unmet, retain a diagnostic status rather than silently labeling the world equilibrated.

Initially hold terrain fixed during climate preparation. Alternating climate and erosion passes are a separate extension and do not block the first seasonal world.

### H. Ecology and geological resources

Update vegetation from available water, thermal conditions, and soil state. Bound growth and mortality rates. Classify biomes from smoothed conditions to avoid switching after a single shower.

Geological features already exist after plate generation. Add full deposits later using an independent RNG stream, with quantity, quality, and depth. Discovery and extraction arrive with population. Potential agricultural yield is not the same as available wild food.

## 5. Dynamics after generation

The provisional main step is one model day, with smaller transport substeps where stability requires them. Daily quantities are averages, not a detailed hourly weather simulation.

Step duration is an explicit input. Transport cannot remove more than the available stock. A shared-edge flux subtracts from the source and adds to the recipient. Limit all outgoing flows jointly rather than independently.

Compute new values from beginning-of-step state into a separate buffer. This avoids artificial advantages for regions processed earlier. Flux integration and subsystem ordering are versioned model choices.

Stability depends on region size and transport speed. Choose substeps from a permissible-transfer condition and compare against smaller steps. Do not hide unstable behavior by indiscriminately clamping negative values.

## 6. What the user sees

The first map includes shaded physical terrain, elevation with a metric legend, land/water, plates, boundary types, continentality, and elevation contributions. Navigation supports panning, zooming, and region selection. Controls show the seed, resolved parameters, and generation progress; a new request cancels an obsolete task.

The flat projection must split triangles crossing the seam and map all display fragments back to the original `cell_id`. Projection distorts polar shapes; measurements come from spherical geometry. Early categorical layers should expose actual computational regions instead of concealing them through smoothing.

The globe uses the same centers and elevations, with shared-corner display interpolation. A labeled vertical-exaggeration factor improves readability without affecting physical heights or reference areas/distances. [B5 water-surface display](water-surface.md) adds wet-region caps at the initial water level, occluded by the interpolated bed. This approximate visible shoreline does not replace native wet/dry classification or measurements; analytical layers still expose the bed.

The current product provides a flat analytical map and a globe with computed relief, not detailed 3D cities or a street-level environment. Future settlements are markers sized or styled from their modeled properties. Visual detail must never acquire routing or resource consequences without an explicit scale-coupling model.

Dynamics adds Play/Pause, single stepping, a date, and temperature, precipitation, wind, snow, river, soil-moisture, and vegetation layers. Every layer distinguishes current values, period averages, and climatic normals.

## 7. Primary references and adaptation boundaries

- [Red Blob Games: Procedural map generation on a sphere](https://www.redblobgames.com/x/1843-planet-generation/) demonstrates a spherical graph, procedural geology, and rendering. Our icosphere choice is a separate proposal. Its demonstration's random moisture assignment does not meet our causal climate goals.
- [Barnes, Lehman, Mulla: Priority-Flood](https://arxiv.org/abs/1511.04463) describes depression analysis/filling, including applicability to irregular meshes. Analysis filling does not imply removing physical lakes.
- [Barnes, Callaghan, Wickert: Fill–Spill–Merge](https://esurf.copernicus.org/articles/9/105/2021/) addresses water routing through depression hierarchies. Adapting it to our spherical mesh needs separate design and verification.
- [Three.js: BufferGeometry](https://threejs.org/docs/pages/BufferGeometry.html) documents buffered geometry and attributes for the proposed viewer.

These references support individual techniques, not the realism of the complete proposed model. Reusing external code requires license and attribution checks.
