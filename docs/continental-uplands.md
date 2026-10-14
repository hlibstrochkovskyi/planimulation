# Variable initial continental columns

October 14, 2026. Opt-in `multiscale-geography-2` extends the frozen
[version-1 geographic fields](multiscale-geography.md). Both remain selectable;
the default remains `terrain-prep-1`. This is a structural initial-condition
increment, not geological history or completion of R3.

## Measured cause and hypothesis

In version 1, continentality saturates at one and fixes thickness at 35 km and
density at 2,800 kg/m³. The existing unloaded-column formula then fixes baseline
height at 166.6667 m, regardless of location inside fully continental regions.
For `first-light` at subdivision 6, such cells occupy 95.3050% of initial dry
area; their baseline minimum and maximum are exactly equal. Static plate
envelopes and detail remain, but there is no regional interior variation in that
baseline. This explains a structural limitation, not every depression.

Hypothesis: broad variation in initial continental columns can supply regional
uplands and downhill tendencies without rerolling continents, fabricating outlets,
or treating additional cell noise as geological evolution.

Real continental thickness varies with tectonic setting. The
[Christensen–Mooney study](https://www.usgs.gov/publications/seismic-velocity-structure-and-composition-continental-crust-a-global-view)
reports differing average thicknesses for extended crust and orogens, with a
weighted global estimate around 41 km. This motivates variable columns, **not**
our exact proxy, coefficients or a calibrated topographic prediction.
[CRUST1.0](https://igppweb.ucsd.edu/~gabi/crust1.html) also separates crustal
profiles from topography. No source code or observed dataset is copied here.

## Declared model choice

Retain version 1's seeded potential `p`, fixed reference threshold `t`,
continentality `c`, density, plate roots/motion, and roughness channels. Define:

```text
x = max(p - t, 0)
response = x / (x + 0.25)
thickness = 7000 + 28000*c + 15000*c*response       [m]
density = 3000 - 200*c                            [kg/m³]
baseline = -4500 + thickness*(1 - density/3300)
           - 7000*(1 - 3000/3300)                [m]
```

Potential excess is a procedural continental-interior proxy, **not physical
distance from the coast, crust age or a measured tectonic province**. The 0.25
response scale and 15 km maximum extra thickness are fixed versioned choices.
This law does not model plate compression or the passage of time. The inherited
baseline is approximate column buoyancy, not solved isostatic equilibrium,
mantle convection, water loading or elastic flexure.

Thickness is bounded by 7–50 km, density retains its old range, and baseline has
a conservative upper bound of 2,440 m. Increasing a column's thickness increases
this baseline by `delta_thickness * (1-density/3300)`. Oceanic `c=0` columns remain
exactly 7 km. No display or camera state enters these calculations.

Existing convergence/divergence/detail laws and dry smoothing remain unchanged.
With preparation disabled, paired tests show exact agreement of the roughness
and plate arrays, and the bed difference reconstructs from baseline alone.
With preparation enabled, changed slopes can change its conservative transfers.
Neither filling nor breaching silently edits the bed or receiver graph.

Material changes describe a **different initial world**, not an in-simulation
addition of crust or an unrecorded material flow. Initial water fitting follows
the complete new bed. Ongoing deformation would require actual material/rate
accounting and its own model.

## Persistence and observation

Select **Multiscale geography II · continental uplands**. The existing Recipe
fields and binary protocol-10 layout are retained; the explicit model pin selects
the material law. Export contains actual thickness, density, baseline and recipe.
Checkpoint regeneration uses the same pin. Independent TypeScript scalar checks
validate the new columns and baseline when decoding native frames; old-family
bounds are not relaxed. Old fingerprints remain pinned.

The thickness palette deliberately retains its common 7–35 km range and
**saturates above 35 km**, explicitly stated in the legend/inspector. Numeric
columns still expose the full range. This avoids changing the owner's existing
renderer work or disguising color saturation as a physical clamp.

## Controlled paired cohort

```bash
cargo run --locked --release --manifest-path native/Cargo.toml \
  --example geography_cohort -- artifacts/new-uplands-cohort 6 --uplands
```

Twenty ordinary seeds and the same three directed controls compare v1/v2 with
identical requested parameters and four old smoothing passes. New-directory-only
outputs retain recipes, native-compatible fingerprints, physical atlases and
metrics. Observer report version 2 labels the selected comparison explicitly.

Two distinct controls separate confounding effects:

- A static receiver ablation fixes the v1 terminal mask while changing terrain.
  The mask is an exogenous boundary set, **not candidate water ownership or a
  simulated water state**. It never replaces the world's actual drainage.
- A separate v2 initial world retains the baseline's resolved water volume
  through the existing fixed-volume initializer. It does not refit coverage.
  This remains an initial-state comparison, not ongoing global lake redistribution.

Recorded medians over the twenty ordinary worlds (mean of middle two values):

| Sampled diagnostic | v1 | v2 |
| --- | ---: | ---: |
| Connected dry landmasses | 24.5 | 25 |
| Area-weighted dry height p50 above initial water | 304.47 m | 782.63 m |
| Area-weighted dry height p95 above initial water | 1,337.19 m | 1,784.39 m |
| Closed dry terminal count | 223 | 220 |
| Initial dry area routed to closed terminals | 52.39% | 42.61% |
| Maximum dry receiver hops | 18 | 18.5 |
| Closed dry terminals with fixed v1 mask | 223 | 214 |
| Closed dry area fraction with fixed v1 mask | 52.39% | 40.17% |

The separate fixed-water-volume control has a median closed dry area fraction
of 42.44%, but a median 243.5 dry terminals. The maximum relative represented
water-volume difference from its requested baseline stock is `2.857e-14`, under
the unchanged initializer's precision contract. Thus reduced closed-catchment
attraction persists with water amount held fixed; fewer minima is not a general
consequence. All twenty ordinary coverage-paired cases reduce closed drainage
area, without replacing unsuccessful seeds or claiming an out-of-sample guarantee.
All 23 primary candidate fingerprints match on a second complete cohort run.

Retained manifests: `artifacts/uplands-cohort-l6/manifest.json` (initial run) and
`artifacts/uplands-controls-l6/manifest.json` (with explicit volume controls).
The twenty-pair overview, individual `first-light` comparison and actual desktop
flat/globe views were inspected. Visible changes are regional elevation variation,
not newly computed river valleys or a photorealistic Earth surface.

For `first-light`, fully continental dry baseline now spans about 606.83–1658.18 m.
The hypothesis is supported in a bounded sense: more regional relief and reduced
closed-catchment attraction without rerolling structural fields. The number of
minima changes little; this is **not** a general depression-removal solution.
Higher land elevations do not by themselves establish Earth realism. Closed
potential drainage is not a measured long-term endorheic fraction.

Remaining work: characterize depression depths/storage, retain meaningful closed
basins, improve structural mountain/valley forms and explicit geological
preparation, and qualify finer physical sampling/LOD. Reviewed
[constrained breaching literature](https://jblindsay.github.io/ghrg/pubs/2016_Lindsay_HP.pdf)
offers useful future geometry methods, but DEM flow enforcement is not physical
erosion; none is implemented or used to conceal these retained minima.

The subsequent [depression-geometry review](depression-geometry.md) implements
that characterization without changing v2. It finds smaller depression footprints
but deeper remaining basins, and selects an accounted uplands-incision comparison
before choosing a production geological-preparation model.

## Verification scope

Native controls check independent column values, monotonicity/bounds, unchanged
oceanic material, shared-direction sampling, matched pre-preparation contributions,
exact repeat, old pins and full-land/full-water extremes. Node checks include
independent scalar provenance, directed wrong-family/changed-column rejection,
explicit export and complete seasonal checkpoint continuation. Desktop smoke
tests both projections, numeric inspection, the saturated thickness legend and
recipe save/reopen/regeneration:

```bash
npm run build
node --import tsx scripts/test-geography-desktop.ts --uplands
```

Candidate pins: `d4d48032` at level 3 and `b3c1a10c` at level 6 for the recorded
default parameters with the v2 model selected. Version-1 level-3/6 pins remain
`0727ddc8`/`71acb77a`. No default or higher desktop resolution is promoted.

Full `npm test` passes 690 native tests across 126 targets and 96 Node tests,
retaining one pre-existing ignored native test. Added pins and final observer
changes receive separate focused native checks. Both v1 and v2 desktop smoke
runs pass, with no renderer errors; strict all-target Clippy, typechecking,
formatting and whitespace pass. Build succeeds with the pre-existing Vite
renderer-chunk advisory. Owner `map.ts`/`style.css` hashes remain unchanged and
their edits are excluded from this increment.
