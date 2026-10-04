# Bounded seasonal preparation diagnostics

Implemented October 4, 2026 as a read-only native analysis module and headless report. This checks annual stock/flow changes; it does **not** prepare or reset a world, export a warm-start checkpoint, or change any physical law. [Milestone D](implementation-plan.md#6-milestone-d-seasonal-climate-and-water-cycle) remains incomplete.

## Why preparation needs a separate check

Conservative integration and exact replay do not establish a usable initial climate. Repeating monthly temperature/wind can coexist with changing finite water inventories. [Earlier parameter controls](seasonal-sensitivity.md) already showed unequal first/second-year precipitation.

HydroPy's spin-up analysis checks spatial storage trends and warns that stable global means can hide slowly evolving individual cells. It also identifies snow accumulation where glacier transport is absent. These motivate spatial stock diagnostics, not copying its spin-up duration or treating its externally prescribed rainfall model as our closed atmospheric cycle. [Stacke and Hagemann, HydroPy v1.0, section 4.1](https://gmd.copernicus.org/articles/14/7795/2021/#section4).

Our operational criteria below are explicit, uncalibrated project choices. Neither the paper nor this implementation establishes them as sufficient physical readiness criteria.

## Read-only observation contract

`seasonal_moisture::preparation::Monitor` borrows one immutable native `Model`. It accepts an immutable `State` at an anchor and each next consecutive whole-year boundary. The year is the model's existing 365-day calendar, not an astronomical year. Duplicate, skipped, partial-year, foreign-origin, or invalid observations reject before changing the monitor's accepted anchor/history.

The monitor keeps O(N) preceding boundary stocks and regional flow totals, not every tick or a ten-year history. Work occurs at annual observations; no per-tick callback, renderer dependency, or checkpoint serialization is required by the monitor. This complexity statement is not a measured runtime/memory qualification at the largest grid.

Diagnostic version: `seasonal-preparation-analysis-1`. Supported physical models are explicitly `seasonal-moisture-3` through `seasonal-moisture-7`. A future snow/lake/thermal law must review this analysis rather than silently inheriting its cold-storage interpretation. Physical model and checkpoint versions remain unchanged.

Stocks, in column order, are liquid, snow, soil, pending runoff/transit, terminal water, and vapor. Signed low components belong to their corresponding condensed stocks. They are included separately in sums and changes; cumulative summation corrections are not water.

For each annual stock comparison:

- Sum absolute high-component and low-component changes across **all regions and all six stocks**, normalized by initial mobile mass with a 1 kg denominator floor.
- Report the maximum regional component-L1 change divided by that region's physical area, plus its region ID.

Taking absolute component differences before aggregation prevents spatial/stock cancellation. It conservatively includes even representation-only high/low redistribution: it is not the exact change of high+low physical mass, nor a certified outward-rounded interval bound.

Annual flow columns are precipitation, evaporation, generated runoff (`liquid_runoff + soil_drainage`), snowfall, and terminal delivery. They are differences of validated native cumulative binary64 leading totals. This is approximate differencing, not independently integrated annual flows or a correction-mass inventory. Decreasing or nonfinite differences reject rather than being clipped. Compare each annual regional flow with its preceding year after division by physical area.

At the existing density convention, kg/m² equals millimeters water equivalent. A large local terminal depth is mass divided by the receiving cell's area, not a simulated lake surface level.

## Recorded stationarity criteria

All conditions must hold for three consecutive eligible comparisons:

| Quantity | Default diagnostic condition |
| --- | --- |
| Global stock component-L1 change / initial mobile mass | ≤ 0.001 |
| Maximum regional stock component-L1 change | ≤ 1 mm WE |
| Every regional annual flow change | ≤ 0.1 mm WE + 0.001 × larger of the two annual totals in mm WE |
| Consecutive eligible comparisons | ≥ 3 |

The first observed year has no preceding annual flow interval; its comparison flag is null, not false or true. Consequently the earliest default candidate is the fourth observed year. Starting analysis at a restored whole-year state resets only this observer's history; it neither resets the physical clock nor invents preceding annual flows.

`stationarityCandidate` means these recorded conditions passed. It is **not** a climate-ready flag. With evaporation and precipitation disabled, a nonempty quiet world passes; a zero-water world also passes. A separate `positiveAtmosphericCyclingObserved` flag requires strictly positive annual precipitation and evaporation, but tiny positive flows still pass that flag. No minimum useful circulation, monthly phase consistency, physical calibration, or ecological suitability is inferred. Whole-year comparisons can miss within-year changes.

Criteria validation is separate from native budget tolerances. None of the project's physical conservation checks have been widened to accept a candidate.

## Structural cold-storage control

The monitor classifies each region using the twelve **operational monthly** temperatures: always cold if the maximum is ≤ 0°C, always warm if the minimum is > 0°C, otherwise seasonally warm. These are threshold classes for this fixed forcing, not realistic glacial regions or weather extremes.

Under the supported laws, always-cold snow cannot melt or sublime; soil cannot evaporate/drain; terminal water cannot evaporate or spill. Liquid on an initially wet always-cold region cannot evaporate or form land runoff. Their combined inventory is a structural lower-bound set of water without a return path. Always-cold **land liquid is excluded**, because it can generate runoff; transit and vapor are excluded because they can move. Other inaccessible inventories may exist, so this set is not a complete accessibility classification.

A directed zero-tilt fixture has genuinely always-cold and warm regions. Its cold snow/terminal gains reconcile with their recorded incoming flows, while the complete checkpoint and continuation remain unchanged by observation.

Crucially, **all ten generated trajectories below have zero always-cold regions**. Cold locks are a real supported-law limitation, but do not explain the observed decline in these standard worlds. Do not add a snow-return law as an asserted fix for these results.

## Measured trajectories

The [retained validation record](data/seasonal-preparation-validation.json) stores complete native report configurations, pins, criteria, all annual assessments, budgets, and failures. It is a diagnostic export, not a simulation checkpoint.

The default suite uses three subdivision-2 seeds, each at radii 1,000 and 6,371 km, with the existing 1 m initial active liquid layer. Two reference controls change only that layer to 10 m. All eight run ten years with a 900-second coupling ceiling and daily caller intervals. A second suite repeats the two 1,000 km reference depths with a 450-second ceiling. Both suites use explicit version 7; no default promotion follows.

All ten runs complete numerically and round-trip full checkpoints exactly. Maximum accepted daily global relative mass residual is below `8.95e-16`; maximum regional-ledger residual is below `4.51e-15`. At the ten-year clock limit, another hour is unavailable: continuation is explicitly null, not reported as a successful eleventh-year step. The directed shorter test verifies exact next-hour continuation.

None meets the recorded stationarity criteria by year ten. Representative 900-second results:

| Seed / radius / active depth | First-year precipitation | Tenth-year precipitation | Year-ten component-L1 / initial mass |
| --- | ---: | ---: | ---: |
| reference / 1,000 km / 1 m | 404.04 mm | 98.20 mm | 8.25% |
| coast / 1,000 km / 1 m | 422.95 mm | 106.85 mm | 8.22% |
| interior / 1,000 km / 1 m | 430.17 mm | 124.61 mm | 5.98% |
| reference / 6,371 km / 1 m | 157.53 mm | 51.96 mm | 4.10% |
| reference / 1,000 km / 10 m | 423.63 mm | 319.23 mm | 5.23% |
| reference / 6,371 km / 10 m | 157.53 mm | 170.85 mm | 2.62% |

The 1,000 km, 1 m reference shifts from 78.31% liquid and 14.44% terminal water after year one to 33.54% liquid and 62.51% terminal water after year ten. Year-ten snow is only 0.85% of initial mobile mass. Water is conserved while its ownership and location evolve; global conservation is not a stationarity test.

Increasing the layer to 10 m does not establish preparation. At 1,000 km, global precipitation appears almost constant near 432.19 mm in years four through seven, then falls to 319.23 mm by year ten. At 6,371 km, year-nine/year-ten precipitation differs by only about 0.00062 mm, but year-ten component-L1 change is 2.62% and the maximum local change is 858.28 mm. Comparing only rainfall would miss evolving inventories.

Halving the reference coupling ceiling preserves the decline, zero always-cold count, and lack of stationarity at both depths. Maximum absolute annual global precipitation differences are 0.18264 mm (1 m) and 0.26373 mm (10 m); maximum relative differences are below 0.085% and 0.062%, respectively. This single halving is not a spatial/local convergence certificate or arbitrary-parameter qualification.

Verification: the full `npm test` run passes native targets and 72 Node/adapter tests. The final targeted suite adds compatibility coverage for every supported physical version: five module tests and five integration tests cover spatial/stock/low-component cancellation, flow arithmetic and rejection, monthly thermal thresholds, quiet/zero-water candidates, immutable complete state, exact replay, criteria validation, invalid observations, and restored anchors. Clippy passes with warnings denied. No new desktop feature or GUI run is claimed.

## Reproduce and next gate

```sh
cargo test --locked --manifest-path native/Cargo.toml --lib preparation
cargo test --locked --manifest-path native/Cargo.toml --test seasonal_preparation
cargo run --release --locked --manifest-path native/Cargo.toml --example seasonal_preparation_report -- --smoke
cargo run --release --locked --manifest-path native/Cargo.toml --example seasonal_preparation_report
cargo run --release --locked --manifest-path native/Cargo.toml --example seasonal_preparation_report -- --refined
```

The smoke workload is two subdivision-1 reference depths for two years; it is not the decade qualification. Reports preserve failed native intervals, accepted complete annual observations, and atomicity witnesses rather than declaring numerical rejection a stationarity result.

Next: inspect reference-water liquid versus terminal ownership and their effective evaporation footprint, then test an explicit conservative return/redistribution mechanism on constructed controls before any versioned physical change. The measured stock transfer identifies where much of the water resides, not a unique causal explanation for all rainfall changes. Do not increase the clock limit, widen criteria, mask inconvenient regions, replenish deep water, or reset stocks just to obtain a pass. Physically paired spatial-resolution controls and eventual prepared-state export remain separate gates. Lake levels/spill, groundwater, snow sublimation, coherent weather, and ecology remain unimplemented.
