# Initial water: milestone B4

[B5 water-surface display](water-surface.md) now adds a separate globe water mesh without changing this model or protocol. B4 validation measurements below describe the analytical-bed build.

Implemented model: `water-1`, binary protocol 5. This increment adds initial global filling, an explicit water stock, and connected water bodies downstream of B3 elevation. It does not add drainage, flowing rivers, rainfall, erosion, salinity, or evolving lake budgets.

## Parameters and ownership

All earlier recipe fields remain required. The new required `water` field selects exactly one constraint:

```json
{ "mode": "coverage", "fraction": 0.71 }
```

or:

```json
{ "mode": "volume", "volumeCubicMeters": 1400000000000000000 }
```

Coverage is between 0 and 1 and refers to **all initial water**, not just the largest ocean. Volume is finite and nonnegative, bounded by reference-sphere area times 20,000 m. The upper bound is an engineering limit, not a statement of planetary habitability. The UI uses percent and km³; the native state uses fractions, meters, m², and m³.

The default is 71% target coverage. No water RNG is used. Water settings do not change terrain, crust, plates, or the diagnostic field. Native generation stores both the input constraint and the resolved level, volume, depths, and component IDs. Those fields enter the initial fingerprint. Headless output includes requested settings and resolved statistics, including every body's area and volume.

`Save recipe` exports versioned generation inputs, not a state checkpoint or a separate resolved-state archive. Reopening the same recipe with the supported binary recomputes the fitted state exactly. Preserve the headless report when an explicit record of the fitted inventory is needed. Old recipes, including `terrain-1`, are rejected rather than reinterpreted; older model implementations remain in Git history.

## Regional volume approximation

Each computational region has one authoritative bed elevation `h_i` and reference-sphere area `A_i`. For the initial level `L`:

```text
depth_i = max(0, L - h_i)
volume  = sum(A_i * depth_i)
wet_i   = depth_i > 0
```

This is a column/prism approximation, not integration over the display triangles or a curved spherical shell. It does not include slope-corrected area, water loading, or partial-region shoreline fractions. A level equal to a region's bed leaves that region dry. Mesh adjacency, not pixels or interpolated corners, determines connectivity.

All bodies begin at one globally fitted level. This is a prescribed initialization, **not** a claim that water can cross a dry divide. It neither demonstrates a filling pathway nor establishes permanent communication between basins. Future dynamics must track separate reservoirs and spill thresholds; repeated global redistribution during ordinary lake evolution would violate this distinction.

## Coverage fitting

1. Sort regions by increasing bed elevation, then region ID for deterministic ties.
2. Accumulate physical area in that order, considering a prefix only after the entire equal-elevation group is included.
3. Choose the prefix closest to the requested total wet area. Equal area errors prefer the smaller prefix. No plateau is split arbitrarily.
4. For a nonempty partial prefix, use the midpoint between the last included height and the next height. If floating-point rounding would leave the last included height dry, use the next height itself; its own depth remains zero.
5. Empty selection uses the minimum bed height and zero volume. Full selection uses the maximum bed height **plus 1 m**.
6. Compute actual depths and the inferred volume from that level.

The midpoint and full-coverage 1 m headroom are explicit versioned conventions. Coverage alone underdetermines volume, particularly when all regions are wet. The UI reports actual coverage separately from the target. A perfectly flat world can only be all dry or all wet; a 50% request chooses dry. Without equal-height plateaus, the area error is bounded by the largest region's area (the ensemble check uses this conservative bound).

## Volume fitting

Sort by bed height and raise the level through successive heights. The volume needed for the next height is `active_area * height_difference`. If the remaining stock fits before that height, stop and use `level + remaining_volume / active_area`. Otherwise subtract that capacity, include the next region, and continue. Above the highest bed, all reference area participates. Zero stock resolves to the minimum bed level and no wet regions.

Recompute the resulting stock from the depths. Accept error at most `max(1e-9 m³, requested_volume * 1e-10)`; otherwise report that the volume cannot be resolved at the current elevation precision. This matters for very small requested stocks spread over enormous areas. Do not silently replace an unrepresentable positive stock with an unrelated amount.

Generation is `O(N log N)` for sorting plus `O(N + E)` for connected components, with linear auxiliary storage. The algorithm has no unbounded iterative convergence loop.

For a future terrain-change step, reuse the **resolved volume**, not the original coverage target. Directed tests exercise this refitting primitive, but an erosion pipeline is not implemented. Once basins evolve independently, use their own budgets and spill topology rather than this global initial-fitting procedure.

## Connected water bodies

Breadth-first traversal of positive-depth neighbors assigns IDs in ascending first-region order: `0` is dry; `1..K` identify connected bodies. The largest by physical area is labeled the **main ocean**; equal areas retain the earlier ID. All other bodies are inland basins. This convention does not infer salinity, geological origin, or navigability. A dry world has no main ocean (`mainOceanId = 0`).

IDs are reproducible for a fixed initial world, not persistent identities across later merging, splitting, or regeneration. Stable historical basin identities need a separate policy when dynamics are introduced.

## Transport and views

Protocol 5 preserves earlier array ordering and appends:

| Field | Encoding |
| --- | --- |
| Initial level, resolved volume | 2 × Float64 |
| Depths | N × Float64 |
| Body IDs | N × Uint32 |
| Main ocean ID | Uint32 |

Additional payload: `20 + 12N` bytes. Diagnostic frames remain unchanged except for their protocol tag. Before publication, the TypeScript adapter validates finite values, nonnegative stocks/depths, depth reconstruction within 1e-8 m, volume balance, wet/dry identity, canonical connected components, and largest-body identity. Tests deliberately corrupt each new field category.

Both the flat map and globe share water-depth and water-body layers and the same inspector values. Depth colors run from shallow cyan to deep blue, scaled to the current world's maximum depth; dry regions are neutral gray. Body colors are categorical, not depth or salinity. The legend identifies the main-ocean ID.

**The analytical water layers render the geological bed, including underwater relief.** B4 itself does not add a raised water mesh. The subsequent [B5 surface layer](water-surface.md) adds one as a separate display mode, with an explicitly approximate shoreline. Neither mode changes water volume or native wet/dry classification.

## Validation

- Constructed weighted basins: separated IDs, largest-area ocean, deterministic ties, and connection only above a sill (not at zero depth).
- Unequal-area coverage fitting, plateaus, nearest-prefix ties, dry and fully flooded endpoints.
- Fixed-volume refitting after changing beds, common datum shifts, area/volume scaling, tiny-stock precision failure, and extreme supported radii.
- 20 seeds × 3 resolutions (0, 2, 4) × 6 water constraints = 360 combinations, each generated twice. Compare native water exactly, reconstruct volume, check wet adjacency, coverage error, coverage-to-volume refitting, and unchanged upstream fields and diagnostic-time state.
- Strict Rust/TypeScript settings validation, binary corruption rejection, physical-area/volume summaries, and unchanged water during display geometry preparation.
- Real desktop tests cover shared flat/globe inspector values, water legends, dry and fully flooded volume mode, volume-recipe save/import reproduction, recipe controls, and the existing cancellation/import/export/diagnostic checks.

### Recorded results: September 23, 2026

All 22 Rust tests and 33 TypeScript tests passed, retaining the earlier geometry, plate, crust, and elevation checks. TypeScript checking, Rust formatting, Clippy with warnings denied, and whitespace checks passed. Development and packaged Linux x64 desktop checks passed; flat and globe screenshots were reviewed. Packaging succeeded. The existing Vite warning about a renderer chunk above 500 kB remains advisory, not a failed check.

Default `first-light`, level 5, in the supported environment:

| Observation | Result |
| --- | ---: |
| Initial fingerprint | `5408377f` |
| Model-array bytes | 5,527,300 |
| Initial level | 112.47295174648913 m |
| Resolved stock | 1.4043843363492772 × 10¹⁸ m³ |
| Total wet area | 71.00062806486244% |
| Main ocean area | 69.26561589120649% |
| Inland water area | 1.7350121736559542% |
| Connected bodies | 19 |
| Maximum depth | 4,752.846494303114 m |

These are regression observations, not quality targets. Default bed elevations and upstream crust/plate fields remain unchanged from B3.

On the Ryzen 5 PRO 4650U laptop with 14.84 GiB RAM and hardware-enabled WebGL/compositing, the depth layer at 10× globe exaggeration measured:

| Regions | Generation plus both view preparations | Flat RAF p95 | Globe RAF p95 |
| ---: | ---: | ---: | ---: |
| 10,242 | 1,434 ms | 16.7 ms | 16.7 ms |
| 40,962 | 3,683 ms | 16.7 ms | 16.7 ms |

The benchmark ran without simultaneous project builds/tests. Each sample covers 180 requested-animation-frame intervals during alternating zoom and native diagnostic diffusion, not GPU completion times or future hydrology performance. Generation timing includes the desktop interaction and view preparation. The raw local report is `artifacts/native-desktop-benchmark.json`, overwritten on each run. A separate headless native-plus-adapter observation took about 156 ms, excluding display preparation; it is not a controlled comparison against B3.

## Next increments

B5 now supplies separate display water geometry with an explicit shoreline approximation. Catchments, flats, depression hierarchy, storage/overflow, and mass-accounted erosion belong to milestone C. Do not rename initial filling or diagnostic diffusion as hydrology.
