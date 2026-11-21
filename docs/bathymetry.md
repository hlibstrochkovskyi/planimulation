# Bathymetry in multiscale geography V

`bathymetry-1` supplies an explicit native `seafloor` bed contribution for
`multiscale-geography-5`. The terrain ledger adds it to the legacy crust,
convergence, divergence, and detail terms before dry preparation and the
initial water fit. The elevation and depth layers consume the resulting bed
and water fields. The inspector reports the signed contribution as **Ocean
floor**. The recipe and initial-world export identify the generating family,
with the bathymetry coefficients pinned by that family. Bathymetry draws no
random numbers.

## Model

**Datum and eligibility.** Fit a provisional water level on the input bed using
the recipe's coverage or volume setting. The model moves only cells below that
level and keeps them submerged at that datum. It uses continentality 0.5 as the
crust transition. For wet continental cells the adjustment fades out from 900
to 1,500 km from the interpolated crust transition on the continental side.
Distances use spherical edge lengths on the surface graph. Dry preparation,
coastal lowlands, and the final fit can subsequently change the wet set.

**Ages and thermal depth.** Oceanic sides of divergent ocean–ocean contacts
sample a zero-age axis. A deterministic shortest-path search stays on oceanic
crust in the source plate. Distance from the closest axis divided by half the
local positive normal opening gives the age. The half-rate is the mean at the
source across its contributing segments, bounded to 5–70 km/Myr (5–70 mm/yr).
Age is capped at 180 Myr, also used where no ridge is reachable. The GDH1
thermal depth in meters is `2600 + 365 sqrt(t)` for age `t < 20 Myr` and
`5651 - 2473 exp(-0.0278 t)` thereafter. At 180 Myr the depth is 5,634 m.
One same-plate neighbor average softens discontinuities between different
nearest-source rates. The axis and self-weight are fixed, and the weight of a
neighbor is `0.3 × (1 − smoothstep(edge length / 240 km))`. The thermal target
retains 60% of the input bed's non-buoyancy residual, clamped to ±250 m before
scaling. [Stein & Stein, 1992](https://doi.org/10.1038/359123a0)

**Subduction.** At convergence, the oceanic side descends beneath continental
crust. For two oceanic sides the larger value of crust density plus
`0.2 kg/m³ × age in Myr` descends, with ties going to the lower plate ID. Axis
samples require a submerged incoming region. The target axis depth is
`7600 + 1500 v/(0.04 + v) + 750 t/180` meters, with normal closing `v` in
m/year and age `t` in Myr. The cut fades smoothly over 175 km on that plate,
with a 300 m outer rise peaking 150 km from the axis and ending at 300 km.
Continental collisions have no trench source. The prescribed shape follows
the observed seaward rise. [Caldwell et al., 1976](https://www.sciencedirect.com/science/article/pii/0012821X76902156)

**Continental margin.** A shoreline source is interpolated on an edge between
wet and dry cells with continental crust at the dry endpoint. The wet graph
carries distance to that shoreline. Distance from the source shoreline to
subduction sets activity, with a smooth transition from fully active within
100 km to passive beyond 600 km. The passive profile takes 160 km to reach the
160 m shelf break, then 250 km to the slope foot at about 3 km, then 480 km to
merge into the local thermal depth. The active profile narrows the shelf and
slope to 60 and 155 km. For coarse meshes the passive end receives an extra
shelf width of up to 30 km, half the nominal equilateral center spacing beyond
60 km subject to that cap. This compensates for sparse sampling and dry
preparation across the break. The model follows the observed contrast between
passive and active shelf widths. The chosen widths also account for the
coarseness of the Planimulation mesh. The basin and trench targets are applied relative
to the provisional datum. Subtracting the input bed yields the stored term,
bounded to ±12,000 m. [Harris et al., 2014](https://doi.org/10.1016/j.margeo.2014.01.011)

## Reproduce the comparison

```sh
export CARGO_BUILD_JOBS=4
cargo run --release --locked --manifest-path native/Cargo.toml --example bathymetry_report
cargo test --release --locked --manifest-path native/Cargo.toml --lib bathymetry
```

`bathymetry_report` records area-weighted depth percentiles, mean and maximum
for the largest connected water body, share at 0–200 m and deeper than 3 km,
and the depth on the sampled V ridge and trench axes in both beds. Its land
statistics use all dry regions, with low land measured from 0 to 200 m above
the final fitted water level. `Switched %` is the share of the planet with a
different wet/dry classification in IV and V. The report uses the new-world
recipe parameters and varies only the family, subdivision, and listed seed.
At subdivision 7 it applies the IV preparation stages to the common input bed
as a measurement control. At subdivision 6 the example checks that the same
procedure exactly reproduces the supported IV world and water fit.

Earth percentiles and area shares were computed from the ETOPO1 Ice Surface
grid sampled every 20th point in latitude and longitude with cosine latitude
weights, excluding the repeated ±180° column. Negative heights formed the
marine sample. The reported maximum depth is NOAA's full-resolution ETOPO1
value. Shelf area agrees with the 7–8% range in the NOAA-hosted coastal ocean
reference. [ETOPO1 grid](https://doi.org/10.7289/V5C8276M),
[gridded data subset](https://erddap.marine.ie/erddap/griddap/etopo180.nc?altitude%5B0:20:10800%5D%5B0:20:21600%5D),
[NOAA hypsographic curve](https://www.ncei.noaa.gov/sites/default/files/2023-01/Hypsographic%20Curve%20of%20Earth%E2%80%99s%20Surface%20from%20ETOPO1.pdf),
[The Global Coastal Ocean](https://repository.library.noaa.gov/view/noaa/39955/noaa_39955_DS1.pdf)

## Measurements

| Seed | Subd. | Family | D10 / D50 / D90 (m) | Mean (m) | Max (m) | Shelf (%) | >3 km (%) |
| --- | --- | --- | --- | --- | --- | --- | --- |
| Earth | — | ETOPO1 | 549 / 4097 / 5379 | 3686 | 10911 | 7.45 | 75.19 |
| first-light | 6 | IV | 2294 / 4691 / 4805 | 4135 | 4883 | 1.60 | 87.12 |
| first-light | 6 | V | 496 / 4479 / 5617 | 3869 | 8967 | 7.46 | 71.99 |
| 5c151f46 | 6 | IV | 2192 / 4748 / 4859 | 4165 | 4919 | 1.80 | 86.80 |
| 5c151f46 | 6 | V | 540 / 4363 / 5635 | 3871 | 9060 | 7.10 | 72.17 |
| 4836187e | 6 | IV | 2194 / 4826 / 4906 | 4231 | 4957 | 2.18 | 87.23 |
| 4836187e | 6 | V | 530 / 4771 / 5639 | 4025 | 9044 | 7.42 | 73.77 |
| first-light | 7 | IV control | 2351 / 4768 / 4871 | 4203 | 5006 | 1.58 | 87.45 |
| first-light | 7 | V | 388 / 4512 / 5625 | 3886 | 9063 | 8.11 | 73.55 |
| 5c151f46 | 7 | IV control | 2269 / 4840 / 4939 | 4251 | 5015 | 1.56 | 87.19 |
| 5c151f46 | 7 | V | 426 / 4394 / 5633 | 3887 | 9150 | 7.86 | 74.05 |
| 4836187e | 7 | IV control | 2370 / 4908 / 4981 | 4325 | 5044 | 1.87 | 87.93 |
| 4836187e | 7 | V | 431 / 4793 / 5645 | 4040 | 9042 | 7.89 | 75.27 |

| Seed | Subd. | Family | Ridge D50 (m) | Trench D50 / max (m) | Land (%) | Land <200 m (%) | Level (m) | Switched (%) |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| Earth | — | ETOPO1 | — | — | 29.049 | 28.13 | 0.0 | — |
| first-light | 6 | IV | 3863 | 4132 / 4729 | 28.999 | 21.60 | 298.2 | 0.000 |
| first-light | 6 | V | 2582 | 8602 / 8967 | 29.001 | 35.15 | 543.9 | 1.087 |
| 5c151f46 | 6 | IV | 3863 | 4258 / 4870 | 28.999 | 21.77 | 348.8 | 0.000 |
| 5c151f46 | 6 | V | 2567 | 8514 / 9060 | 29.001 | 32.63 | 584.0 | 1.296 |
| 4836187e | 6 | IV | 4063 | 4479 / 4868 | 29.000 | 28.11 | 395.5 | 0.000 |
| 4836187e | 6 | V | 2605 | 8492 / 9044 | 29.001 | 39.25 | 568.1 | 1.828 |
| first-light | 7 | IV control | 3822 | 4099 / 4779 | 29.000 | 24.80 | 363.9 | 0.000 |
| first-light | 7 | V | 2609 | 8626 / 9063 | 29.000 | 36.21 | 548.3 | 1.041 |
| 5c151f46 | 7 | IV control | 3849 | 4343 / 4963 | 29.000 | 25.23 | 428.4 | 0.000 |
| 5c151f46 | 7 | V | 2594 | 8531 / 9150 | 29.000 | 33.23 | 580.5 | 1.105 |
| 4836187e | 7 | IV control | 4048 | 4496 / 5024 | 29.000 | 33.55 | 470.3 | 0.000 |
| 4836187e | 7 | V | 2621 | 8515 / 9042 | 29.000 | 40.87 | 577.1 | 1.462 |

At subdivision 7, generating V took 1.27–1.31 s across the three seeds.
The standalone bathymetry stage took 78–111 ms, including the provisional fit.
These release timings cover the native stages measured by the example.

The three subdivision-6 worlds put 7.10–7.46% of the ocean on the shelf,
with 7.86–8.11% at subdivision 7. Median ridge-axis depths of 2.57–2.62 km
and deepest trenches of 8.97–9.15 km are distinct from the surrounding floor.
The median ocean bed is still deeper than the ETOPO1 sample, and the increased
share of low land accompanies the rise in the fitted datum. The switches in
wet/dry classification cover 1.04–1.83% of the planet, with total land area
near 29.00% in both families.

## Acceptance and limits

The native fixtures cover the thermal law, spreading rate and plate isolation,
trench polarity at the three crust pairings, passive and active shelf widths,
continental interior protection, deterministic output, and its magnitude bound
across subdivisions 0–7. The TypeScript test verifies a nonzero contribution
through the real core binary and resolved export. The desktop check uses the V
recipe for `first-light` and checks the depth and elevation layers and a trench
region's inspector value. The inspector's explanatory copy still describes
the original V placeholders, even though its signed Ocean floor row uses the
native contribution.

Age is a static estimate using present-day plate motion. Old floor can
accumulate at the age cap in basins without spreading contacts. Ridge and
trench axes follow sampled plate edges, so stair steps, gaps, and coarse
features remain at the desktop mesh scale. The comparison holds erosion at the
version-5 base commit's zero contribution. A future erosion contribution can
change the final fit and needs a fresh comparison. Volume recipes preserve the
requested initial water volume in the final fit, with the resulting coverage
free to move. The model covers thermal subsidence and prescribed regional
profiles. It omits plate history, sediment loading, lithospheric flexure,
fracture offsets, hotspots, and sub-cell trench cross sections.
