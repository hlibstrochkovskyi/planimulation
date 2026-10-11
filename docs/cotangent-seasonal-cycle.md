# Cotangent seasonal-cycle integration

October 9, 2026: the separately selected headless `regional-seasonal-water-2` family integrates the [cotangent conductance candidate](surface-cotan-candidate.md) into the existing five-owner [regional soil cycle](regional-soil-cycle.md). This is an implementation choice, not a new product principle or a claim of calibrated hydrology. Existing defaults, model-1 trajectories, desktop protocol 15 and old saves remain unchanged.

## Explicit selection and persistence

Native callers select `Settings { surface_operator: SurfaceOperator::CotangentWeakForm, ..settings }`. The default remains `BarycentricTwoPoint`. The setting does not change the generated world, radius, physical areas, bed, coast, reference-body partition, climate or renderer geometry.

| Selection | Complete schema | Family | Surface pin | Observation |
| --- | ---: | --- | --- | --- |
| Old, reject | 1 | `regional-seasonal-water-1` | `regional-surface-flow-paired-1` | `regional-soil-observation-1` |
| Old, retain donor | 1 | `regional-seasonal-water-1` | `regional-surface-flow-paired-2` | `regional-soil-observation-1` |
| Cotangent, reject | 2 | `regional-seasonal-water-2` | `regional-surface-cotan-paired-1` | `regional-soil-observation-2` |
| Cotangent, retain donor | 2 | `regional-seasonal-water-2` | `regional-surface-cotan-paired-2` | `regional-soil-observation-2` |

Other subsystem pins remain determined by the unchanged numerical policy and forcing preparation. The fixed-input report's `regional-surface-cotan-candidate-1` pin is not accepted as a seasonal surface pin.

Old JSON omits `surfaceOperator` and resolves to the old operator only. New JSON explicitly records `"surfaceOperator": "cotangentWeakForm"`. Missing selection under new family/schema pins, null/unknown selection, wrong subsystem pins, policy/diagnostic mismatches and malformed stock/history arrays reject. Restore is not a migration: it regenerates the resolved world and prepared operator, then validates the entire state against the selected family's origin and independent stock/history identities.

Cotangent weights are prepared once into a private immutable layout. It clones the old physical layout, replaces only the numerical conductances and recomputes their explicit stability bound; the original forcing layout is not mutated. Checkpoints cannot supply arbitrary weights. Positive finite conductance and native adjacency checks are the same as the isolated candidate's. The default branch does not allocate this extra layout.

## Applied cycle and meaning

The unchanged cycle applies vertical exchange, delayed soil drainage and surface exchange in two half-stages around paired atmospheric transport. It retains absolute cadence boundaries, finite body inventories, five paired regional stocks, gross directed histories, soil capacity and complete caller rollback. Surface substeps use the selected prepared operator's bound, not the old operator's bound. No tolerance is widened, hidden queue added or deferred grant counted as completed flow.

For the new family, `surfaceTransfers` and the observation's rounded cumulative incoming/outgoing fields are **conservative weak-form numerical adjacency exchanges**. They are not measured instantaneous river discharge or exact flux through the old barycentric dual-face boundaries. Atmospheric histories continue to use their original physical-face meaning. Their canonical directed indices are shared, but their interpretations are not interchangeable.

Reference-body mobile water remains finite and counted once per connected body. Its geometric head and initial wet contacts remain prescribed; it receives surface arrivals but does not supply surface outflow. The nonlinear capped Manning-inspired mobility, crest barrier and frozen leading-depth requests are inherited approximations. Smooth capped diffusion convergence does not prove continuum convergence of nonlinear fronts or coupled seasonal behavior.

The new headless observation has its own version. The existing desktop adapter requires exact old product settings, so both display and checkpoint writers refuse the cotangent selection before writing any frame. No GUI selector, protocol extension, default promotion or renderer edit is included here.

## Directed checks

Tests exercise both numerical policies and reject selection/schema/model/surface/policy corruption. A finite-funded generated active-flow state checks actual soil infiltration and drainage, paired low components, operator-dependent completed surface histories, read-only observation, daily/hourly absolute-clock partition equality and exact original JSON restore/continuation. Funding follows a real adjacent atmospheric contact with matching body evaporation and rainfall histories; it is an explicitly supplied stress state, not naturally generated rainfall.

A 642-region generated-world control exercises donor-retention diagnostics; the corresponding strict first-day precision refusal remains atomic rather than silently selecting retention. A 10,242-region small-radius high-mobility case refuses its surface work bound after vertical preparation and rolls back the complete caller. Closed-dry, fully wet and disabled-flow controls retain unique ownership without creating terrestrial water.

The isolated dry-front test starts with one wet column on a flat native sphere. Both certified capped and uncapped initial mobilities keep all second-ring columns dry on the first frozen substep and permit arrival on the next, while preserving normalized nonnegative owners and mass. This is a causality/positivity regression, not an exact continuum dry-front solution or a temporal/spatial convergence study.

## Recorded seasonal evidence

The [retained validation summary](data/cotangent-seasonal-validation.json) fingerprints the complete reports and numerical sources. All twelve donor-retaining 365-day cases complete: eight 162-region generated worlds (four seeds and two coverage targets), one 642-region generated world, and three finite-funded 162-region cadence controls. All complete checkpoints round-trip exactly and continue for an hour exactly. Repeating the full annual report, with the optional flags reversed, gives byte-identical output (5,779,398 bytes).

Maximum recorded global and local relative residuals are 1.9543e-16 and 3.8543e-16. Ordinary cases have 67–736 active surface directions and 1.0933e12–5.7487e13 kg of gross numerical exchange. Funded cases have 134–136 active directions and approximately 1.8198e17 kg of gross exchange. These are cumulative crossings that can count water repeatedly, not stocks or instantaneous physical discharge.

Annual deferred-request counts range from 999,746 to 23,198,352 per case. Summed deferred requests range from 42.7204 to 2,251.8820 kg; the largest single request is 3.6996 kg. These remain visible numerical-limiter diagnostics, can count repeated attempts, and are neither lost mass nor a bound on physical error. Every case retains the complete diagnostics and paired histories.

The three funded cases start at second 900 and finish at the same absolute second 31,536,900. Other settings, geography, forcing and supplied input are identical. Signed high/low column differences and finer-stock sums use compensated summation in the retained-summary analysis:

| Coupled ceilings (s) | Liquid L1 / finer liquid | Soil L1 / finer soil | Relative gross surface-exchange difference |
| --- | ---: | ---: | ---: |
| 900 → 450 | 9.423719e-5 | 9.833848e-5 | 9.810306e-7 |
| 450 → 225 | 4.718131e-5 | 4.911541e-5 | 4.879827e-7 |

These differences approximately halve; snow, vapor and delayed drainage comparisons are also retained. This supports cadence refinement in this bounded supplied case, not nonlinear spatial convergence or general climate accuracy. The twelve 30-day smoke cases also complete and replay exactly, but ordinary surface flow is still zero at that early endpoint; the annual run is the active-flow evidence.

A twelve-case one-day old-operator cohort, including three actively flowing funded controls, was captured before the change and rerun after it with byte-identical complete output. This is a direct old-seasonal trajectory/serialization regression, not a claim that every historical ensemble was rerun. The original isolated smooth-sphere accuracy report remains a separate artifact.

Validation: full `npm test` passes 527 native tests and all 18 Node test files, with the same one pre-existing ignored annual-snow test. Release checks pass 75 library tests, twelve regional-soil tests, five legacy regional-surface tests and two soil-transport tests. The latest strict-refusal assertions also pass in separate debug/release reruns. All-target Clippy with warnings denied, formatting and whitespace checks pass. CLI invalid/duplicate arguments and overwrite controls refuse without changing output. No GUI rerun is claimed for this headless-only integration; the owner's renderer edits remain untouched and excluded.

## Reproduction and remaining gates

```bash
cargo test --locked --manifest-path native/Cargo.toml --test regional_soil
cargo test --locked --manifest-path native/Cargo.toml --lib surface_cotan
cargo run --release --locked --manifest-path native/Cargo.toml --example regional_soil_report -- 365 artifacts/new-cotan-year.json --retain-donor --cotangent
```

The report records every refusal and its atomic rollback, last accepted complete checkpoint and exact accepted/refused continuation. It accepts the optional flags in either order and refuses invalid/duplicate flags or existing output paths. Buffered JSON output does not change numerical state or serialization bytes. Runs with different accepted endpoints must not be compared as equal-time solutions.

The next qualification gates are broader dense active-flow/cadence evidence for this family, sharp-head/front refinement under an explicitly defined continuum problem, longer-term drift, and physical review/calibration. A later desktop adapter must make the new history meaning explicit. Dynamic reference-body heads/coasts, ocean circulation, groundwater, empirical soil/cold closures and ecology remain separate work. No milestone C/D completion is claimed.

The subsequent [dense cotangent study](cotangent-dense-study.md) selects this same family in the dense tool, checks complete initial-state comparability and includes counted-once body differences. It does not change these seasonal equations or relabel generated-world coverage as isolated spatial convergence.
