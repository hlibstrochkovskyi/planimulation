# Coastal lowlands: `multiscale-geography-4`

November 15, 2026. Generator family for new desktop worlds (`NEW_WORLD_RECIPE`)
from November 16 to November 21, 2026, replacing `multiscale-geography-3`;
replaced in turn by [`multiscale-geography-5`](eroded-landscape.md). `DEFAULT_RECIPE`,
headless defaults, old recipes and fingerprints are unchanged.

## Problem

Versions 2 and 3 put only 6–8% of land below 200 m and 21–33% below 500 m, with
a median land height of 670–830 m (Earth: roughly a quarter of land below 200 m
and a median of a few hundred meters). Continents read as tan uplands with
narrow green rims. The cause is the variable continental column: at the
shoreline the crust is already 3–4 km thicker than the 35 km endmember, and the
crust baseline rises about 700 m over the first ~330 km inland.

## What changes relative to version 3

Version 4 reuses version 3 exactly (crust, plates, mountain belts, divergence,
multiscale detail, dry preparation and filled plains) and adds one explicit
elevation contribution, **coastal lowland** (`coastal_lowlands::lowering`,
`Terrain::lowland`, shown in the inspector):

1. After dry preparation and the initial water fit, graph distance `d` from the
   nearest wet region is computed over neighbor distances.
2. The compressible height of a dry region is its crust baseline above the
   water level, capped by its height above the water level without belt uplift.
3. A kept fraction `f(d) = 0.25 + (0.85 − 0.25)·smoothstep(d / 1200 km)` of that
   height remains; the rest is the (non-positive) coastal-lowland contribution.
4. Shallow depressions are then filled and water is refitted, as in version 3.

The low kept fraction near the coast is a proxy for passive-margin subsidence and
coastal-plain sediment; the interior factor below 1 lowers stable platforms.
Mountain belts and multiscale detail keep their full relief. Every dry region
stays above the water level, so the set of wet regions — and therefore every
shoreline — equals version 3. The refitted level can move inside the gap between
the highest wet and lowest dry bed (by under a meter on the review seeds). This
is an initial-condition choice, not modeled flexure, sediment transport or
erosion.

## Measurements

Subdivision 6 (40,962 regions), default recipe parameters, measured with
`cargo run --release --example land_metrics -- SEED MODEL`:

| Seed | Land below 200 m, v3 → v4 | Land below 500 m, v3 → v4 | Median land height, v3 → v4 | Land draining to closed sinks, v3 → v4 |
| --- | --- | --- | --- | --- |
| first-light | 6% → 22% | 22% → 54% | 791 → 462 m | 16.0% → 4.4% |
| 5c151f46 | 6% → 22% | 21% → 51% | 830 → 492 m | 15.5% → 8.5% |
| 4836187e | 8% → 28% | 33% → 61% | 669 → 379 m | 13.4% → 3.9% |

The parameters were chosen on these three seeds against rough Earth figures
(about a quarter of land below 200 m, half below 500 m); a first trial with
0.2/0.75 overshot (28–36% below 200 m).

## Retained limitations

- The ramp gives coastal plains a uniform seaward tilt. Where multiscale detail
  is weak, drainage lines run as short parallel streams straight to the sea.
- Land draining to closed sinks (4–9%) is now below Earth's endorheic share
  (~18%), which is largely climatic and is not modeled.
- Distance is measured to any wet region, including inland water bodies, and
  uses graph distance with the mesh's direction bias.
- A first variant that also compressed all non-belt height, including detail and divergence, made
  coastal plains featureless; it was replaced by baseline-only compression.
