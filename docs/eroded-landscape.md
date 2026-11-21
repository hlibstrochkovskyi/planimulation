# River-carved land: `multiscale-geography-5`

November 21, 2026. Experimental generator family, selectable as **Multiscale
geography V · experimental**. `NEW_WORLD_RECIPE` stays `multiscale-geography-4`
until version 5 is reviewed. Old recipes and fingerprints are unchanged.

## Problem

The owner's review of version-4 worlds: the planet looks like a toy. Mountains
read as smooth bands with a shadowed wall; rivers run over terrain they never
cut, as straight parallel streams on uniform slopes. The cause is in the
generator: relief is noise and tectonic envelopes, and no process ever eroded it.
`stream-power-incision-1` existed as a standalone primitive but was never part of
a world.

## What changes relative to version 4

Version 5 reuses version 4 and adds three elevation contributions. The
inspector and the resolved initial-world export expose all three.

- **Ocean floor** (`Terrain::seafloor`, `bathymetry-1`) is applied before dry
  preparation and the initial water fit. See [Bathymetry](bathymetry.md).
- **Fluvial relief adjustment** (`Terrain::erosion`) follows coastal lowland
  compression. Its negative part supplies sediment. Positive values come from
  fitting the regional relief envelope.
- **Routed sediment** (`Terrain::sediment`, `sediment-1`) aggrades the prepared
  bed after the inherited shallow-depression fill and before final drainage and
  basin analysis. The world recipe pins the coefficients below. The native V
  packet and its resolved export also name `sediment-1` explicitly.

Version 5 is the only family that accepts subdivision 7 (163,842 regions, about
56 km spacing).

## Erosion model

Dry land is replaced by a steady-state stream-power landscape. Uplift balances
detachment-limited incision `E = K·A^m·S` with `n = 1` and `m = 0.5`, so along
the drainage network

```text
h_i = h_receiver + k_i · A_i^(−m) · L_i
```

where `A` is the contributing area, `L` the link length and `k = U/K` the
steepness. Water is the base level. This builds the drainage network and its
valleys together, as in uplift–erosion terrain generation (Cordonnier et al.
2016), ordered upstream with the Braun & Willett (2013) stack.

1. **Initial routing.** A priority flood from all water gives every dry region a
   path to water over its lowest sill, so closed dry depressions do not stall the
   network.
2. **Steepness.** `k_i = T̄_i / χ̄_i`, where `T` is the version-4 height above the
   water level and `χ = Σ A^(−m)·L` is the integrated drainage coordinate along
   the path to water, both smoothed by two neighbor-averaging passes. Then
   `h ≈ k·χ` region by region, and the regional height follows version 4. A
   fixed log-normal factor (σ = 0.3, stream `multiscale-geography-5/steepness`)
   breaks grid symmetry.
3. **Receivers.** Each iteration recomputes receivers by steepest descent on the
   new heights, with each link's slope scaled by a fixed factor in `[0.6, 1]` (a
   mesh-independent form of Rho8, Fairfield & Leymarie 1991). Without it, uniform
   slopes produced parallel valleys along the grid's preferred directions. The
   network settles within 40 iterations on the review seeds.
4. **Relief.** The final height is the version-4 envelope (two-pass smoothed)
   plus the fluvial relief around its own two-pass mean. The relief is damped on
   low ground by a weight `T̄ / 1500 m`, clamped to [0.35, 1], because on Earth
   local relief grows with elevation and plains stay flat. Every dry region stays
   at least 5 m above water, so shorelines do not move.

## Sediment model

A region with area `a` and negative fluvial adjustment `e` supplies
`a × max(0, −e)` cubic meters of solid-equivalent material. The stage uses a
single downstream pass over the actual dry/flat drainage receivers of the
prepared bed. Wet cells serve as base level at the fitted water surface.
Each reach passes on the sum of local and incoming material minus the volume
it deposits. The stage draws no random numbers. The inherited 200 m
shallow-depression fill belongs to dry preparation. It runs before routing and
has its own elevation contribution.

**Alluvial plains and fans.** Integrated carrying capacity is
`C = 30,000 m × A × S`, where `A` is contributing dry area in m² and `S` is
the dimensionless bed gradient to the receiver. This is the finite-volume
analogue of a capacity law with area and slope exponents both equal to 1.
The coefficient is a calibration length. A low-gradient trunk can retain
`max(0, load − C)` where its contributing dry area is at least 25,000 km²,
its gradient is at most 1.5 m/km and its bed is at most 380 m above water.
A fan can form if the steepest upstream link is at least 6 m/km and
2.5 times steeper than the receiving reach, whose slope is at most 4 m/km.
These limits restrict deposition to places where transport slows or incoming
and locally sourced material exceed the capacity of a gentle channel.
The caps are 34 m for plains and 90 m for fans, further limited to 1 cm below
the lowest donor bed. Existing flat links stay passable. The v5 coastal
lowland kept fractions are 0.48 at the coast and 0.935 inland with the inherited
1,200 km ramp, allowing room for alluvial aggradation. [Davy & Lague, 2009](https://doi.org/10.1029/2008JF001146)

**Closed basins.** The local spill is the lowest shared boundary saddle of
the basin's downhill catchment. Received material raises its lowest cells
together, following the area–elevation storage curve up to the spill or the
available supply. A 12 km thickness bound applies. Surplus stays in the
reported unplaced basin inventory for this generation pass. Any remaining
closed floors are endorheic depressions, with their outlet geometry available
to `rivers-1` for its evaporation, lake extent and spill calculations. The
stage never lowers a sill. Existing inland wet bodies can also receive up to
160 m at each entry cell, limited by submerged accommodation.

**Delta fronts and shelves.** The strongest mouths deposit first and may
draw only on their own delivered volume. In the main ocean, a river with at
least 225,000 km² of contributing dry area can build a delta front if 45% of its
load can lay at least 20 m across one region. That share fills accommodation
outward from the mouth through connected waters within 125 km and at most
180 m initial depth. Front construction requires at least 20 m of added
thickness per credited cell. This implements the supply–accommodation mass
balance behind delta progradation at mesh scale. The remaining load can
form a shelf wedge along connected water within 300 km and 400 m initial
depth. Its accommodation tapers linearly with both distance and depth and
is capped at 160 m of shelf aggradation per region, including earlier deposits.
The unused volume is reported as marine bypass. [Paola et al., 2011](https://doi.org/10.1146/annurev-marine-120709-142856)

**Water fit.** Deposits stay at least 8 m below the pre-sediment water surface.
A coverage recipe refits the datum after deposition, preserving the set of
wet regions. Deltas prograde below the water surface and preserve the mapped
shoreline. For volume recipes, the stage budgets at most half the initial
clearance to the lowest dry terrain times pre-existing wet area for underwater
deposits. The final fit preserves the specified water volume and the shoreline
even where the clearance is small.

## Measurements

Release runs used the default recipe parameters at each listed resolution,
with generation timed before report postprocessing. The control is geography V
at commit `9963809`. It routed and deposited zero cubic meters. Its erosion
supply column integrates the available negative fluvial adjustment. The later
column includes both the retuned V lowland factors and the sediment stage.
Land height and relief statistics weight regions equally to match the original
landscape report. Local relief is the largest height difference to a dry neighbor.

| Seed | Subd. | Land median (m) | p99 (m) | Max (m) | Below 200 m (%) | Local relief (m) at <200 / 500–1,000 / >2,000 m | Dry sinks | Native generation (s) |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| first-light | 6 | 408 → 402 | 1866 → 1837 | 2961 → 2940 | 21.9 → 22.0 | 67/174/766 → 65/168/762 | 0 → 0 | 0.49 → 0.48 |
| 5c151f46 | 6 | 451 → 442 | 1751 → 1743 | 2696 → 2697 | 21.1 → 21.1 | 69/174/706 → 65/171/692 | 2 → 1 | 0.48 → 0.44 |
| 4836187e | 6 | 363 → 361 | 1825 → 1819 | 3088 → 3083 | 27.9 → 27.8 | 61/178/768 → 57/175/754 | 5 → 1 | 0.48 → 0.48 |
| first-light | 7 | 399 → 392 | 1918 → 1915 | 3258 → 3247 | 24.7 → 25.3 | 54/133/651 → 52/131/610 | 2 → 0 | 2.10 → 2.20 |
| 5c151f46 | 7 | 441 → 432 | 1807 → 1800 | 3193 → 3189 | 23.2 → 23.7 | 54/133/587 → 52/131/574 | 8 → 4 | 2.14 → 2.39 |
| 4836187e | 7 | 347 → 341 | 1870 → 1864 | 3350 → 3336 | 30.6 → 31.1 | 47/131/605 → 45/129/595 | 8 → 1 | 2.25 → 2.22 |

The ledger below uses km³ except for the relative residual and the percent
shares of **deposited** material. Shares list plains, fans, delta fronts,
shelf wedges and basins, in that order. Basin totals include existing inland
water. The native report prints the absolute residual in m³ as well.

| Seed | Subd. | Control supply | V supply | Deposited | Marine bypass | Basin remainder | Relative residual | Deposit shares (%) |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| first-light | 6 | 4,110,373 | 4,031,979 | 1,407,333 | 2,176,066 | 448,580 | 4.852e-15 | 29.34/2.63/10.47/54.95/2.61 |
| 5c151f46 | 6 | 4,265,554 | 4,177,466 | 1,314,317 | 2,342,939 | 520,210 | 2.005e-15 | 26.88/2.06/14.32/55.10/1.64 |
| 4836187e | 6 | 3,915,755 | 3,841,639 | 1,341,700 | 1,605,218 | 894,722 | 8.785e-16 | 29.82/2.70/9.24/53.37/4.88 |
| first-light | 7 | 3,084,737 | 3,014,015 | 1,046,307 | 1,551,491 | 416,217 | 3.484e-15 | 12.05/6.39/12.28/67.68/1.61 |
| 5c151f46 | 7 | 3,281,444 | 3,202,857 | 1,075,235 | 1,714,144 | 413,477 | 5.913e-15 | 10.54/7.23/12.26/68.72/1.25 |
| 4836187e | 7 | 2,921,402 | 2,853,315 | 1,032,077 | 1,010,234 | 811,003 | 2.497e-15 | 11.78/7.14/9.00/68.12/3.96 |

Depositional footprints are zero in the control. Shelf area below is the
main ocean's 0–200 m depth band immediately before and after deposition.
All six runs had zero wet/dry switches. Area is in km².

| Seed | Subd. | Deltas | Delta area | Plain area | Fan area | Dry basins filled to local spill | Marine shelf area | Water coverage (%) |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| first-light | 6 | 82 | 1,829,932 | 14,249,655 | 437,603 | 0 | 26,749,428 → 27,772,008 | 70.99900 |
| 5c151f46 | 6 | 95 | 2,140,064 | 12,161,500 | 361,839 | 3 | 25,415,266 → 26,674,020 | 70.99925 |
| 4836187e | 6 | 74 | 1,629,029 | 14,373,349 | 440,745 | 5 | 26,639,863 → 27,381,617 | 70.99909 |
| first-light | 7 | 85 | 1,490,567 | 5,207,799 | 900,645 | 0 | 29,066,269 → 29,888,399 | 71.00014 |
| 5c151f46 | 7 | 87 | 1,605,872 | 4,519,712 | 1,049,331 | 14 | 28,229,811 → 29,152,869 | 71.00011 |
| 4836187e | 7 | 72 | 1,196,496 | 5,258,284 | 1,018,932 | 10 | 28,304,840 → 29,058,589 | 71.00005 |

Filling a local catchment to its sill can connect it to another closed catchment.
The number of filled basins is measured on the retuned pre-sediment bed. The
final dry-sink count uses rebuilt drainage and is compared with the control bed.

```sh
export CARGO_BUILD_JOBS=4
for sub in 6 7; do
  for seed in first-light 5c151f46 4836187e; do
    cargo run --release --locked --manifest-path native/Cargo.toml \
      --example eroded_landscape_report -- "$seed" "$sub"
  done
done
```

## Desktop inspection

The subdivision-7 `first-light` world (`b44fc8c0`) was inspected on the atlas
and globe in Land and water and Bed elevation. The close-ups center on the
largest delta's mouth at 20.893° S, 106.847° E (region 82095) and the largest
fan at 40.749° N, 133.707° W (region 16184). The depth layer also shows the
submerged coastal wedge. The natural-color surface emphasizes a continuous
coast, and the fan reads as aggradation below the mountain front at this mesh
spacing.

## Natural ensemble on version 5

`NATURAL_MODEL=multiscale-geography-5 cargo run --release --example natural_ensemble -- --threads 4`
(subdivision 6, commit `82e04e5`: `bathymetry-1` and lowland factors 0.5/0.95, before
`sediment-1`) passed all
seven laws in all 27 standard and 15 extreme runs (294 of 294 checks), in 7 min
16 s. Downstream calibrations hold:

| Seed | Forest year 0, v4 → v5 | Mean NPP, v4 → v5 (Pg/yr) | Mean forager capacity, v4 → v5 (M) |
| --- | --- | --- | --- |
| first-light | 22.5% → 22.9% | 114.3 → 116.6 | 14.18 → 14.47 |
| 5c151f46 | 34.2% → 35.6% | 120.4 → 122.9 | 13.12 → 13.42 |
| 4836187e | 23.6% → 25.7% | 143.9 → 149.9 | 19.71 → 20.14 |

## Acceptance checks

- Native checks cover independently integrated deposited volume, the source
  ledger, an overloaded low-gradient trunk and mountain front, sediment
  passage through steep reaches, mouth supply and shallow-ocean eligibility,
  and deterministic deposition. An underfed dry basin stays endorheic. A fed
  basin fills to its spill without cutting its rim. Both water modes are tested.
- The elevation ledger includes sediment within the existing numerical
  tolerance of `1e-8` m. Families IV and earlier retain their wire format and
  output. The version-V world packet uses protocol 11. The decoder checks the
  field size, version, bounds and reconstructed elevation.
- The Node tests exercise native generation and the resolved export, including
  the export bound at subdivision 7.

## Limitations

- This is a single initial bed construction pass with a synthetic erosion
  supply and a static network. The routed budget tracks changes to the bed as
  equal-density prisms. It omits compaction, lithology, grain size, reworking,
  sea-level history, load-driven subsidence and sediment redistribution by
  marine currents.
- Marine bypass and unplaced basin inventory are diagnostic totals. The
  generator assigns them no bed location or later transport. It does not
  reroute excess from a filled basin to its next downstream basin in this pass.
- Delta fronts exclude distributary channels and emergent lobes. Reported
  mapped areas are region footprints, with a 5 m deposit threshold for plains
  and fans. These quantities depend on mesh spacing. S6 cells are roughly
  110 km across and S7 cells roughly 56 km across, so fan geometry is resolved
  as reach-scale aggradation.
- The inherited dry preparation fills shallow depressions independently of
  the routed budget. The signed fluvial term can also raise terrain as part of
  the envelope fit. Neither adjustment contributes sediment.
- Mountain belts retain their version-4 envelopes. Rivers still use one
  receiver per region. Display-only meanders and surface detail alter no
  sediment or elevation fields.
