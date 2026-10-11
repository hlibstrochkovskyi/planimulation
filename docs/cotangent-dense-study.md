# Dense cotangent seasonal controls

October 10, 2026: the headless dense-report tool can explicitly select the [cotangent seasonal family v2](cotangent-seasonal-cycle.md). This increment changes the qualification tool and its tests, not production equations, tolerances, default settings, desktop protocol or checkpoint interpretation. The original [model-1 dense study](soil-dense-study.md) remains a separate regression witness.

## Declared physical input and controls

The cohort uses `first-light` and `receiver-2` at subdivisions 3 and 4 (642 and 2,562 regions), the same `basins-1` recipe and 30% initial coverage target as the original dense study. Each world starts at second 900 with finite main-ocean funding of 50,000 kg/m² into the land contact at its first canonical main-ocean/land adjacency. Actual contact evaporation, atmospheric crossing and rainfall histories fund the same terrestrial liquid owner; no external water is added. This supplied 50 m stress input is not naturally reached precipitation or a weather-frequency claim.

On each fixed world, five branches share the complete funded initial state and forcing: 900/450/225 s coupled ceilings at a 1e6 m²/s mobility cap, and 900 s controls at 1e5 and zero cap. The numerical policy remains `retainDonor`. The measured cohort requests 90 daily caller intervals, reaching second 7,776,900 if successful. Unlike the original 30-day experiment, it crosses the prescribed forcing transitions at days 31 and 61; it is still not a dense seasonal year or long-term drift study.

The first canonical contact, bed, wet mask, physical areas, temperature and funding footprint can change across meshes. Shared-direction geographic diagnostics remain explicit. Comparing those generated worlds is not isolated spatial convergence, a Richardson/GCI estimate, or an exact wetting-front study. The [isolated smooth-sphere evidence](surface-cotan-candidate.md) qualifies a different bounded capped regime.

## Comparison and ownership contract

Every branch retains its complete initial checkpoint internally. Same-mesh comparison requires identical final time and successful completion, normalizes only the named cadence or cap setting, then checks the entire initial checkpoint: owned high/low pairs, histories, pins, forcing, recipe and absolute clock. Different funding or initial history cannot silently become a solver-error estimate. Construction errors abort the report; runtime refusals retain the first refusal, atomic rollback and last accepted complete state. Failed or unequal endpoints are explicitly non-comparable.

All five regional owners are compared using compensated signed-pair differences, per-region absolute mass differences, finer/second-stock normalization and physical column areas. The cotangent report additionally compares the **counted-once finite body inventories** in kilograms and relative L1. Bodies receive no invented regional area or depth metric. A zero comparison stock reports null relative difference, not zero error. Full native JSON restore and one-hour continuation are checked for both accepted and refused outcomes.

Surface histories are conservative weak-form numerical adjacency exchanges, not literal continuum barycentric-face discharge, instantaneous river flow or extra inventory. Gross crossings can count water repeatedly. Numerical-retention diagnostics are retained separately and can count repeated requests against the same water; they are neither lost stock nor a physical-error bound. Changing the mobility cap changes the model's response, not just its runtime.

## Test fixtures and isolation

Nine report tests cover the original funding/replay/precision/refusal/geography conditions and the new complete-initial-state guards, cotangent branch selection and body-pair differences. A directed soil assertion explicitly supplies constant warm forcing: the first main-ocean contact is not guaranteed to be warm in the first month. This test-fixture choice does not alter the declared generated-world cohort or remove its inherited cold-soil gate.

Without `--cotangent`, the tool retains its original report version, shape and comparison fields. New finite-body comparison fields and explicit weak-form scope belong to `regional-soil-cotan-dense-report-1` only. Legacy trajectories are not retuned or migrated. The owner's separate camera/style edits are outside this increment.

## Recorded ninety-day results

The [retained validation summary](data/cotangent-dense-validation.json) records all twenty branches, resolved forcing/settings/pins, funding, budgets, numerical diagnostics, work counts, six-owner comparisons and source hashes. All twenty complete 90 daily requests and finish at exactly second 7,776,900, including both monthly forcing transitions. Every complete native checkpoint round-trips and continues for an hour exactly. All sixteen nonzero-cap branches have actual surface and soil exchange; the four zero-cap branches have no surface exchange. Nonzero branches activate 86–600 numerical adjacency directions.

| Seed | Regions | Liquid L1, 450 → 225 s / finer stock | Soil L1, 450 → 225 s / finer stock | Maximum liquid-column difference, cap 1e6 → 1e5 |
| --- | ---: | ---: | ---: | ---: |
| first-light | 642 | 0.006963% | 0.014634% | 2.334 m |
| first-light | 2,562 | 0.129789% | 0.021019% | 0.320 m |
| receiver-2 | 642 | 0.010107% | 0.013078% | 1.290 m |
| receiver-2 | 2,562 | 0.099609% | 0.015193% | 0.183 m |

Liquid and soil L1 differences reduce by factors of 1.9962–1.9984 between the two successive cadence comparisons in all four fixed-world groups. The largest finer-pair liquid-column difference is 0.6511 mm. This supports time refinement for these supplied states, not a universal convergence order or a spatial-error estimate. Finite-body finer-pair relative L1 differences are 5.6474e-7–6.6863e-7; no body-depth comparison is invented. Snow, vapor and delayed drainage differences also remain in the retained record.

Lowering the cap changes liquid distribution by 4.984%–41.417% **of the lower-cap branch's remaining liquid stock**, much more than the cadence differences. This confirms that the cap changes the modeled response; it does not select a calibrated replacement. Neither this ninety-day cohort nor the earlier thirty-day cohort is a universal bound on cap sensitivity.

Maximum measured global/local relative residuals are `1.7125892711894628e-16` and `4.1151778710956274e-16`, with unchanged native checks. Deferred-request counts are 7,705–12,989,227 per case; cumulative requested mass is 0.004936–1,400.354 kg and the largest single request is 0.483259 kg. These diagnostics cover the retaining cycle, can count repeated attempts, and are not missing water or measured physical error. Smaller steps need not reduce their cumulative count or mass.

The complete report repeats byte-for-byte (97,333,291 bytes), including all native stock/history pairs and diagnostics. The default twenty-case thirty-day report also remains byte-identical before/after this tool change and to the historic buffered report (95,360,949 bytes). These are separate exact-reproduction controls, not cross-operator physical agreement.

Validation: full `npm test` passes 530 native tests and all 18 Node test files; the same one pre-existing ignored annual-snow test remains. The nine dense-report tests pass in debug and release, and all twelve regional-soil integration tests pass in release. Warnings-denied all-target Clippy, formatting and whitespace checks pass. Invalid/duplicate CLI arguments and overwrite controls refuse without altering existing output. No GUI scenario is rerun for this headless-only increment; owner renderer edits stay outside it.

## Reproduction and remaining gates

```bash
cargo run --release --locked --manifest-path native/Cargo.toml --example regional_soil_dense_report -- 90 artifacts/new-cotan-dense-90d.json --cotangent
cargo test --locked --manifest-path native/Cargo.toml --example regional_soil_dense_report
cargo test --release --locked --manifest-path native/Cargo.toml --example regional_soil_dense_report --test regional_soil
```

The CLI accepts 1–365 days, refuses unknown/duplicate flags and existing paths, and publishes buffered complete JSON to an exclusively created output file. Progress messages are not part of reproducible output. Full checkpoints belong in ignored `artifacts/`; scalar evidence and source hashes are retained separately.

Nonlinear sharp-head/front accuracy against a fixed continuum problem, broader dense annual/long-term ensembles, empirical mobility/soil calibration and a history-aware desktop adapter remain subsequent gates. Finite reference-body heads/coasts are still prescribed. No ecology readiness, calibrated discharge or milestone C/D completion is implied.
