# Accepted seasonal summaries and matched parameter sensitivity

Implemented October 4, 2026 as a headless native report. This adds observation and comparison, not a climate law, default change, checkpoint schema, desktop parameter control, or weather generator. It uses the separately pinned [version-7 precision mode](surface-precision.md). Milestone D remains incomplete.

## Observation contract

`seasonal_sensitivity_report` observes only successful hourly caller intervals. Rejected provisional operations never enter its summaries. The collector cannot mutate authoritative state; a directed test compares the entire summarized run against independent advancement without collection.

The existing numbered-month calendar is `floor((day % 365) * 12 / 365)`. Each interval must be contiguous and wholly inside one numbered month. A cross-month integrated flow is rejected, not divided in proportion to elapsed time. The report keeps year and month indices separately, plus accepted seconds and a completeness flag. Partial final months are not extrapolated. These are recorded histories, not monthly climatic normals.

Outputs distinguish:

- Rain, snowfall, precipitation, evaporation, and generated-runoff **totals** over an explicit period.
- Soil and snow **sampled time means**, using right endpoints of accepted hourly intervals, weighted by duration and physical region area. These are quadrature approximations, not exact continuous-time integrals or endpoint stocks.
- Final exclusive stocks and their model-owned budgets.

At the existing water-density convention, `kilograms / square meters` is millimeters water equivalent. Areas are physical, not region counts. Signed low components enter the same soil/snow sampled mass; summation corrections are not physical water. Generated runoff means `liquid_runoff + soil_drainage` at its donors. It is not gross routed departures, new water production, or instantaneous hydraulic discharge.

Each month reports global, initially dry land, positive-uplift land, and nonpositive-uplift land aggregates. The last two use the raw monthly `U · grad(h)` proxy, independent of response strength. They partition initial dry land for that month; they are not fixed annual populations, true windward/lee watersheds, or inferred desert labels. Empty groups have null averages, not invented zero rainfall. Static regional geography includes actual IDs, areas, initial land/water classification, prepared bed, air-facing elevation, and twelve raw uplift values; paired precipitation/runoff changes retain those same IDs.

Accepted precipitation, evaporation, and generated runoff are independently re-summed from returned step transfers and reconciled with cumulative native ledgers at the unchanged `1e-12` relative scale (1 kg floor). Native stock/graph checks remain authoritative. Complete native JSON round trips and another hour are compared exactly. A repeated failed continuation may be recorded as exact replay, but cannot qualify as a successful continuation.

## Matched interventions

The full suite uses three subdivision-2, 1,000 km-radius worlds: `seasonal-reference`, `moisture-coast`, and `moisture-interior`. Each starts from the identical generated finite mobile-water partition for its seed and runs 730 days. Temperature/wind, process order, routing, initial water, numerical representation, and all unvaried settings remain fixed.

All three seeds compare baseline upslope strength 1 against strengths 0 and 2. The reference seed additionally compares response heights 500/2,000 m against 1,000 m and soil capacities 75/300 mm against 150 mm. This gives thirteen runs per suite; the zero-strength control remains version 7 rather than mixing numerical models.

Before integrating any case for a seed, the report finds a common allowed coupling bound across all its variants. The normal suite requests a 900-second ceiling; `--refined` repeats the same interventions at a 450-second ceiling. Actual resolved bounds are recorded and must match within a paired comparison. Hourly sampling stays unchanged. Failed runs retain last accepted monthly data, budgets, and atomicity witnesses; incomplete or cadence-mismatched comparisons cannot qualify. All case configurations and model/module pins remain in the record.

Doubling strength and halving response height produce the same `strength / height` rate here. A test verifies equal complete seasonal summaries, budgets, and stock components despite different resolved settings. These are an identity control, not two independently identified physical effects. Calibration could not distinguish those parameters using this model's response alone.

## Measured scope

The [retained validation record](data/seasonal-sensitivity-validation.json) contains both suites, all monthly observations and spatial comparisons in a compact column-labeled representation. Raw native report JSON is reproducible with the commands below. The retained record is a diagnostic export, not a resumable checkpoint.

The 900-second suite completes all thirteen runs, each with 24 complete numbered months and exact checkpoint continuation. Maximum relative global mass residual is below `2.25e-16`; maximum regional-ledger residual is below `3.30e-15`.

Baseline global precipitation totals show substantial year-to-year change despite repeating prescribed forcing:

| Seed | First-year precipitation | Second-year precipitation |
| --- | ---: | ---: |
| seasonal-reference | 404.04 mm WE | 248.42 mm WE |
| moisture-coast | 422.95 mm WE | 253.98 mm WE |
| moisture-interior | 430.17 mm WE | 245.18 mm WE |

This is evidence that these finite-stock initial trajectories are not a periodic climatology. It does not identify a unique cause for the decline or define a sufficient spin-up criterion. The initial dry atmosphere and persistent water stores must not be silently replaced by an external moisture supply to make the annual totals match.

At 900 seconds, increasing upslope strength from 1 to 2 changes second-year global precipitation by `+0.0782%`, `+0.0409%`, and `-0.0542%` across the three seeds. A local extra deposition rate does not imply monotonically increasing long-run global rainfall: finite donors, transport, and subsequent stock histories interact. This sign reversal is retained, not rejected as a broken numerical invariant. Spatial intervention differences, not a correlation of unrelated worlds, show where the response changes.

For the reference seed, halving/doubling soil capacity changes second-year sampled mean land soil water from 53.95 mm to 26.30/107.19 mm. Corresponding land generated-runoff totals are 424.87/409.46 mm versus 419.86 mm. These are controlled observations of the uniform one-bucket law, not measurements of real soils or universal relationships.

All thirteen 450-second repeats also complete with exact replay and 24 complete months. Maximum relative global residual is below `4.48e-16`; maximum regional-ledger residual is below `4.34e-15`. The sign of each recorded nonzero second-year global precipitation intervention survives this single halving, including the negative double-strength result for `moisture-interior`.

Magnitudes are not all time-converged: disabling extra upslope response in `moisture-interior` changes second-year precipitation by `+0.06257 mm` at 900 seconds versus `+0.03700 mm` at 450 seconds. This is about a 41% change in a small effect, not a 41% change in rainfall itself. Across all interventions, the largest absolute change in the second-year global effect is `0.02557 mm`; the largest area-weighted absolute difference in two-year spatial precipitation effects is `0.05066 mm`. Neither aggregate bound guarantees a local sign or convergence at every region. Preserve the weak-effect sensitivity instead of treating successful water budgets as a physical-accuracy certificate.

Full project verification passes all native targets and 72 Node/adapter tests. The report adds six directed tests and passes Clippy with warnings denied. It does not change the desktop executable or protocol; earlier GUI evidence applies to the underlying mode, not a new parameter/history interface.

## Reproduce and next work

```sh
cargo test --locked --manifest-path native/Cargo.toml --example seasonal_sensitivity_report
cargo run --release --locked --manifest-path native/Cargo.toml --example seasonal_sensitivity_report -- --smoke
cargo run --release --locked --manifest-path native/Cargo.toml --example seasonal_sensitivity_report
cargo run --release --locked --manifest-path native/Cargo.toml --example seasonal_sensitivity_report -- --refined
```

The smoke workload is one subdivision-1 seed for 32 days; it is not the two-year qualification. Tests cover calendar boundaries, physical-area/time arithmetic, zero-area nulls, read-only complete-state parity, repeatability, cumulative-flow reconciliation, parameter-rate identity, unavailable comparison rejection, and invalid duration bounds.

The fixed worlds do not establish arbitrary parameter stability, spatial convergence, realistic climate, or an equilibrium distribution. Generated terrain and runoff still depend on resolution. The finite mobile layer excludes inactive deep water; temperature/wind and geography do not evolve. Groundwater, lake spill ownership, freezing energy, clouds, vegetation, and coherent weather remain absent.

Follow-up: [bounded seasonal preparation diagnostics](seasonal-preparation.md) now compare annual regional stocks and flows, distinguish quiet candidates from active circulation, and retain ten-year drift under both reference coupling ceilings. No generated case qualifies as stationary under the recorded criteria, and none contains always-cold cells; permanent cold storage does not explain their rainfall decline. Next inspect reference-water/terminal return and evaporation footprint, add physically paired resolution controls using shared locations, then expose qualified histories and bounded parameters through the desktop. Cosmetic climate targets or invented weather must not conceal nonperiodic baseline evolution.
