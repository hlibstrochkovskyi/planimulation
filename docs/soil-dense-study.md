# Dense unified soil-water controls

October 9, 2026: this study extends checks of the existing `regional-seasonal-water-1` family to finite-funded active flow on 642- and 2,562-region generated worlds. It adds a headless report and report tests, not a new physical model, altered tolerance, desktop default or save migration. Earlier [annual numerical evidence](regional-soil-cycle.md) and [desktop integration](soil-water-desktop.md) remain separate.

## Experiment and comparison contract

The report runs two declared seeds (`first-light`, `receiver-2`) at subdivisions 3 and 4, with a 30% initial water-coverage target and the resolved `basins-1` recipe from `docs/scenarios/spill-connections.json`. It does not silently substitute the desktop's terrain-preparation recipe. For each generated world, one shared checkpoint starts at second 900. Its finite main-ocean stock funds 50,000 kg/m² of supplied rain into the land region at the first canonical ocean/land face. Matching contact evaporation, one real atmospheric crossing and regional rain histories are recorded. This 50 m stress input is artificial, not a claim about naturally reached precipitation or extreme-weather realism.

Five branches share that complete initial stock/history, geometry, forcing and clock:

| Control | Coupled ceiling | Maximum diffusivity |
| --- | ---: | ---: |
| Baseline | 900 s | 1e6 m²/s |
| Time refinement | 450 s | 1e6 m²/s |
| Time refinement | 225 s | 1e6 m²/s |
| Lower mobility cap | 900 s | 1e5 m²/s |
| No surface mobility | 900 s | 0 m²/s |

The resolved `retainDonor` policy, roughness, vertical exchange and all other settings stay fixed. The coupling ceiling is a requested upper bound; actual coupled/atmospheric/surface work counts are recorded. One-day caller requests advance every branch to the same absolute endpoint unless a numerical refusal occurs. Each refusal keeps its first day/message, whole-state rollback status, final accepted checkpoint and continuation result. A failed or unequal-time endpoint produces an explicit non-comparable record, not a difference between unequal trajectories. Construction failures abort report creation instead of dropping a world from the cohort.

The initial prototype picked the first contact of any wet body, as the coarse annual report did. On the denser first-light world, that small lake could not fund the fixed stress depth. The run stopped before seasonal advancement, and its log is retained. The final fixture explicitly selects the main-ocean contact while retaining the finite donor check; it neither scales down the input nor creates extra water.

## Measurements and interpretation

Each complete native JSON checkpoint is round-tripped and continued for one hour against the uninterrupted state. Diagnostics distinguish actual surface crossings from retained numerical requests. Global/local residual maxima include the starting funded budget. Active directed face counts, positive-soil land area, maximum liquid depth, total owned stocks and actual substep counts accompany complete final native stock/history pairs.

Same-mesh comparisons normalize all settings except the named control and reject changed recipes, thermal/wind settings or a second changed parameter. For each of the five regional owners, the per-region mass difference uses compensated summation of both signed pairs. L1 is the sum of absolute regional differences; relative L1 divides by the second trajectory's total stock. A zero denominator is reported as null, not an invented zero error. Maximum column differences divide by physical area (1 kg/m² = 1 mm WE). Gross surface-crossing differences are differences of flow integrals, not differences of inventory or observed river discharge. Reduction between successive cadence differences is measured, never forced to pass by a tuned coefficient.

The [HEC-RAS mesh/time-step guidance](https://www.hec.usace.army.mil/confluence/rasdocs/r2dum/6.5/running-a-model-with-2d-flow-areas/selecting-an-appropriate-grid-size-and-time-step) motivates checking several time steps on each mesh, rather than relying on numerical stability alone. Its solver and stability recommendations are not copied into this different, capped regional-column model.

Across subdivisions, existing region-center directions are preserved but the generated bed, fitted initial wet classification, temperature forcing and selected funding footprint can change. The report measures area-weighted bed, wet-mask and annual-temperature differences at shared directions, and records the actual input location, area and mass separately on each mesh. No cross-mesh stock difference is advertised as isolated solver convergence. [NASA's spatial-convergence tutorial](https://www.grc.nasa.gov/www/wind/valid/tutorial/spatconv.html) discusses successive-grid discretization studies; our generated-world comparison does not hold the underlying physical problem fixed and is not a Richardson/GCI estimate.

## Reproduction

```bash
cargo run --release --locked --manifest-path native/Cargo.toml --example regional_soil_dense_report -- 30 artifacts/new-soil-dense-30d.json
cargo test --locked --manifest-path native/Cargo.toml --example regional_soil_dense_report
cargo test --release --locked --manifest-path native/Cargo.toml --example regional_soil_dense_report --test regional_soil
```

The report accepts 1–365 days and refuses an existing output path, including at final exclusive file creation. Its complete checkpoints belong in ignored `artifacts/`. Progress goes to stderr, not into the reproducible JSON. Six report tests cover finite contact funding, signed-tail differences and zero denominators, immutable comparison conditions, unequal/incomplete endpoints, a real work-bound refusal with exact rollback/replay, and shared-direction geography differences. Production simulation code is unchanged.

## Measured thirty-day results

The [retained validation summary](data/soil-dense-validation.json) records all twenty branches, complete resolved settings, pins, funding, native budgets, numerical diagnostics and same-mesh comparisons. All twenty complete 30 one-day requests, ending at exactly second 2,592,900. Every checkpoint round-trips and continues for one hour with identical complete native JSON. All sixteen nonzero-cap cases have actual surface flow and active soil exchange; all four zero-cap cases have no surface-face transfer. Nonzero branches activate 8–57 directed faces, so the result is not merely successful accounting on motionless water.

| Seed | Regions | Liquid L1, 450 → 225 s / finer stock | Soil L1, 450 → 225 s / finer stock | Maximum liquid-column difference, cap 1e6 → 1e5 |
| --- | ---: | ---: | ---: | ---: |
| first-light | 642 | 0.002324% | 0.026365% | 10.784 m |
| first-light | 2,562 | 0.021435% | 0.042319% | 3.248 m |
| receiver-2 | 642 | 0.009422% | 0.027930% | 13.116 m |
| receiver-2 | 2,562 | 0.021502% | 0.039275% | 2.473 m |

In all four same-mesh groups, liquid and soil L1 differences approximately halve between 900 → 450 and 450 → 225 s. This supports time refinement for these four supplied states, not a universal convergence order. The measured maximum finer-pair liquid-column difference is only 0.271 mm. Lowering the mobility cap changes liquid distribution much more: its L1 difference is 37.94%–73.57% **of the lower-cap trajectory's remaining liquid stock**. Thus the cap cannot be described as a performance-only safeguard. These results do not select or calibrate a replacement value.

Maximum measured global and local relative residuals are respectively `1.7125892711894628e-16` and `3.710264971468256e-16`, with unchanged native checks. Persistent deferred-request counts range from 14 to 307,314 per case. Summed requested masses range from about `2.637e-7` to `3.675 kg`; the largest single request is `0.10965849705751918 kg`. These diagnostic totals can count the same retained water repeatedly and do not provide a physical-error bound.

The shared-direction control confirms substantial upstream differences between the two generated resolutions: area-weighted mean absolute bed differences are 256.291/258.854 m for the two seeds; sampled wet-classification disagreement is 25.53%/24.21% of coarse area weights. Annual-temperature differences average 0.790/0.757°C. Funding locations, footprint and total kilograms also change. Comparing cross-mesh water trajectories without first fixing these inputs would conflate different physical worlds with discretization error.

The complete twenty-case report repeats byte-for-byte, including all native checkpoint components and policy diagnostics. Buffered output preserves exactly the earlier unbuffered report's 95,360,949 bytes. Direct CLI checks confirm that existing output is refused without changing its bytes and invalid day counts create no output. Six report tests pass in debug and release; all seven regional-cycle integration tests and three paired-atmosphere tests pass in release. Warnings-denied all-target Clippy, formatting and diff checks pass. No GUI scenario is rerun for this headless-only increment.

Full `npm test` also passes: type checking, native build, 508 native tests and all 18 Node test files. The one pre-existing ignored annual-snow qualification test is unchanged. The final additional signed-tail comparison assertions were separately rerun in debug/release after the full-suite build began. Pre-existing owner camera/dragging and style changes stay outside this commit.

## Scope limits

Thirty caller days are a dense active-flow stress check, not a new qualified seasonal year. These trajectories remain within the first prescribed forcing month; monthly transitions remain covered by the earlier annual cohort, not this dense study. They do not establish long-term stationarity, bound the physical error of donor retention, validate wetting-front arrival, calibrate roughness/mobility, or support ecology/population promotion. Two seeds are not a statistical ensemble proving general reliability. Finite reference-body inventory still does not change prescribed heads/coasts. An isolated spatial study needs fixed continuous physical inputs and compatible funding footprints on a refinement family; that remains distinct from this generated-world coverage.

The subsequent [fixed-input surface verification](surface-spatial-verification.md) supplies that isolation for a fully wet flat-bed capped-diffusion control, not for the full soil/climate cycle. It records a spatial/face-flux discrepancy rather than claiming convergence; a consistent physical-mesh flux is the next gate. This does not invalidate the exact generated trajectories or turn them into spatial qualification.

The later [cotangent dense continuation](cotangent-dense-study.md) adds an explicit `--cotangent` selection for separately pinned seasonal family v2. The default report and this model-1 evidence remain separate; the new report checks full initial-state equality and includes finite-body stock comparisons without inventing body areas.
