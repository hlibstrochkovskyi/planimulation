# Mountain belts and filled plains: `multiscale-geography-3`

November 15, 2026. Generator family for new desktop worlds (`NEW_WORLD_RECIPE`),
selected by the owner after visual comparison with `multiscale-geography-2`.
`DEFAULT_RECIPE`, headless defaults, old recipes and fingerprints are unchanged.

## What changes relative to version 2

Version 3 reuses the version-2 crust (continental uplands), continuous plate
roots, crust baseline, divergence response, multiscale detail and bounded dry
preparation exactly. Two steps differ.

**Convergent mountain belts** (`mountain_belts::uplift`). Version 2 multiplies an
exponential envelope `A·exp(−d/W)` around each closing boundary segment by a
ridged factor. Because plate boundaries are nearly straight arcs, belts read as
uniform walls. Version 3:

- computes graph distance `d` to the nearest closing-boundary source and that
  source's amplitude `A` (version-1 closing response from the segment's mean
  continentality, at most 6000 m × relief scale);
- looks `d` and `A` up at a tangentially noise-warped direction (0.045 rad,
  frequency 4), so belts meander;
- varies belt width `W` between 0.55 and 1.45 × the recipe boundary width and
  along-belt strength between 0.35 and 1 with low-frequency noise (massifs and
  saddles);
- uses the profile `0.75·smoothstep(1 − d/1.6W) + 0.25·exp(−d/W)` and ridged
  noise at frequencies 16 and 32 to form ranges inside the belt.

**Filled shallow depressions** (`mountain_belts::fill_shallow_depressions`).
After dry preparation and initial water fitting, a priority flood from wet
regions gives each dry region's spill level. Connected closed dry depressions
shallower than 200 m are raised to their spill level and read as sediment-filled
plains; deeper depressions remain closed basins. Water is then refitted. The
added thickness is reported in the inspector together with dry preparation
("Dry preparation + filled plains"). This is an initial-condition choice, not
modeled sediment transport, lake water or erosion.

## Measurements

Subdivision 6 (40,962 regions), default recipe parameters, measured with
`cargo run --release --example land_metrics -- SEED MODEL`:

| Seed | Land draining to closed sinks, v2 → v3 | Closed dry depressions, v2 → v3 | Land below 500 m, v2 → v3 |
| --- | --- | --- | --- |
| first-light | 50.1% → 16.0% | 186 → 12 | 21% → 22% |
| 5c151f46 | 48.8% → 15.5% | 195 → 16 | 22% → 21% |
| 4836187e | 44.9% → 13.4% | 207 → 8 | 33% → 33% |

Earth's endorheic share is roughly 18% of land. Drainage lines in v3 are visibly
longer and connected. Belts meander and break into massifs, but still follow
plate boundaries.

## Retained limitations

- Land hypsometry is unchanged from version 2: only about 6% of land lies below
  200 m (Earth: roughly a quarter), so coastal lowlands are scarce and continents
  read as uplands. A tried lateral baseline smoothing (flexural-isostasy proxy,
  σ = 200 km) made this worse (median land height ~1570 m) and was removed.
- No overriding-plate asymmetry, trenches, foreland basins or plateaus behind
  belts are modeled. Graph distance keeps the mesh's direction bias.
- Version-3 peaks are lower on average than version 2 (p95 land height
  ~1400 m vs ~1800 m) because of along-belt strength modulation.
