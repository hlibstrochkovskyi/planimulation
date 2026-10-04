# Finite connected-reference-water candidate

Implemented October 4, 2026 as opt-in **`seasonal-moisture-8` / schema 8**, headless only. It changes the physical ownership/availability approximation, unlike the numerical precision changes in versions 5–7. Defaults, desktop protocols, old checkpoints, and the [preceding diagnostic record](water-return-analysis.md) remain unchanged. Milestone D is incomplete.

## Motivation and selected assumption

The preceding decade diagnosis found enough total liquid within a connected reference body to fund frozen evaporation demand, while local liquid/terminal donors frequently could not. Version 8 tests **fast common availability within each already connected reference body**. It does not globally share water, replenish from inactive deep water, or tune rainfall toward a desired target.

This is an explicit coarse hypothesis: internal redistribution is taken as faster than the coupled exchange interval. It has no resolved velocity, travel time, bathymetric circulation, stratification, or energy budget. In particular, it does not establish that ocean-wide equilibration is realistic. HEC-HMS documents reservoir continuity and a level-pool assumption, but also warns that the assumption is unsuitable for very large reservoirs. Our mobile-layer pool borrows the continuity accounting principle, **not Modified Puls routing or its hydraulic validity**. [HEC-HMS: Reservoir Modeling Concepts and Equations](https://www.hec.usace.army.mil/confluence/hmsdocs/hmstrm/reservoir-modeling/reservoir-modeling-concepts-and-equations).

Geometry, reference wet mask, drainage, monthly temperatures, and winds stay fixed. The full reference footprint continues to determine atmospheric demand even as the mobile stock decreases; zero stock funds no evaporation. This is not a changing shoreline or exposed-area model.

## One authoritative liquid owner

Each positive generated reference-body ID owns one normalized `high + low` liquid stock. Initial regional active water is summed into its body and removed from the regional liquid array. The initial global mobile inventory is the same as version 7, not an additional source. Bodies are indexed by sorted positive IDs regenerated from the recipe.

For a reference-water region in version 8:

- Regional liquid and terminal-water stocks, including their low components, must remain zero.
- Rain, melted snow, and runoff arriving at the actual contact region credit the corresponding body.
- Evaporation debits that body and credits the requesting region's vapor.
- Snow remains a local compensated stock. Atmospheric temperature gates, snowfall and melt use the existing closure.

Initially dry regions retain the existing liquid, snow, soil, transit, and terminal-water owners. In particular, a closed dry terminal cannot donate to a reference body. This mode does not turn it into a lake with a derived surface area or spill level.

Actual runoff contact regions are still recorded in `terminalDelivery`. The receiver graph is validated exactly as before; a canonical body ID does not replace contact provenance. Regional `liquidEvaporation` records the actual body-to-atmosphere transfer, **not a debit of a separately spendable local liquid stock**.

## Exchange order and bounded allocation

The existing two half-local/routing stages around a full vapor-transport stage remain. Within each half-local stage:

1. Capture each wet region's demand from its pre-deposition vapor, monthly capacity, and the existing exponential response. Demand is zero at nonpositive temperature or when evaporation is disabled.
2. Perform regional precipitation and snowmelt with wet liquid evaporation suppressed. Credit resulting liquid components to the body and update local snow/vapor/transfer records.
3. After all wet inputs are credited, allocate evaporation concurrently within each body.
4. Perform the existing delayed runoff stage; wet arrivals credit the body and dry arrivals credit the local terminal store. These arrivals become available in the following exchange stage.

Land exchange keeps its existing order. Wet precipitation/melt inputs are thus available throughout their connected body before that stage's grants: this is a new operator, not a claim that version-7 trajectories should remain equal.

For positive total demand `D`, allocation uses one factor:

```text
safe_available = conservative_donor_floor × (1 − 8 × binary64_epsilon)
factor = min(1, safe_available / D)
request_i = demand_i × factor
grant_i = actual compensated donor withdrawal(request_i)
```

Zero demand requests zero grants. An abundant donor funds unscaled demand. The small arithmetic reserve guards sum/product rounding near depletion and remains **owned water**, not a discarded residual, source, physical coefficient, or widened budget tolerance. Negative low tails use the existing conservative donor floor. Subnormal multiplication/division can limit representable grants; ungranted water remains in the donor.

Every grant has one actual debit. If a withdrawal would clip a recipient's scaled request, the interval rejects rather than giving a last-index priority/remainder. There is no final stock reconstruction, mass repair, or grant correction. A shared factor removes explicit index priority; arbitrary floating-point permutations are not promised bitwise invariant. Supported recipe ordering is deterministic.

Demand overflow, malformed components, overdraw, and nonfinite operations reject. The allocator checks its represented stock change against summed actual grants at the existing local arithmetic scale of `32 × epsilon`. Global, vapor, and cumulative ownership gates remain `1e-12`. The two-component representation still has finite precision.

## Budgets, checkpoints, and compatibility

Each body independently validates:

```text
body_water = initial_body_water
           + sum(member rain + melt + actual terminal delivery)
           − sum(member liquid evaporation)
```

Only the displaced wet liquid/terminal regional identities are replaced. Snow, land, atmospheric transfers, runoff graph identities, and global mass remain checked. Balanced transfers between two bodies cannot hide behind the global total. The existing maximum-local-ledger budget field includes body identities in version 8; witness ledger 9 denotes a body identity.

Schema 8 requires the existing compensated-soil/surface/terminal settings and upslope pins, plus:

- `referenceWaterPool: "fastConnectedBody"` in resolved settings.
- `referenceBodyModelVersion: "reference-water-pool-1"`.
- `referenceBodyHighKilograms` and `referenceBodyLowKilograms`, one entry per regenerated body.

Missing fields preserve legacy formats; present null rejects. Model/schema/settings/pins must agree. Body arrays require exact shape and finite normalized nonnegative represented stocks. At day zero they must equal the generated initial partition, including its nonzero low components. Wet duplicate owners reject at any time.

`Budget.referenceBodyWaterKilograms` is a separate owned class; it is not also included in `surfaceKilograms` or `terminalWaterKilograms`. State owned-stock iteration includes body components once. Updates remain cloned/validated/committed atomically, including all body components and transfer summation corrections.

Versions 3–7 omit all new fields and never execute pooling. Old local-operation observers and the preparation/frozen-return diagnostics explicitly reject version 8 because their local-owner/cold-lock interpretations no longer apply. Existing display/checkpoint protocol writers also reject it before writing bytes. No desktop option, checkpoint migration, or derived regional display distribution is introduced.

## Validation and measured scope

The allocator controls cover dyadic independent values, signed donor tails, depletion, dyadic recipient permutation, disconnected membership, zero supply, subnormals, overflow, and up to 100,000 uneven requests. Integration tests cover generated inventories, independent body identities, actual return, nonzero low-component replay, exact hourly/daily batching, forged owners/pins, balanced cross-body forgery, dry/empty/all-water worlds, cold/disabled processes, closed dry terminals, foreign-state rollback, and unsupported observer/protocol refusal.

The matched report runs versions 7 and 8 on the same subdivision-2 reference worlds at 1,000/6,371 km radius and 1/10 m initial active depth. It retains annual flows, budgets, actual coupling, complete replay checks, and failed intervals if any. The 10 m Earth-radius control matters because its old frozen local-demand shortage was already negligible. Different radii also change generated geography; this is not a geographically paired radius intervention.

The [retained record](data/reference-water-pool-validation.json) contains eight ten-year runs at a 900-second coupling ceiling and eight two-year repeats at 450 seconds: **16/16 complete without numerical refusal**. All full JSON checkpoints round-trip exactly. All eight two-year states continue another hour exactly; continuation is explicitly not applicable at the ten-year clock limit, not a successful extra step.

Annual global precipitation at year ten, in mm/year:

| Radius | Initial active depth | Version 7 | Version 8 |
| --- | ---: | ---: | ---: |
| 1,000 km | 1 m | 98.20 | 438.58 |
| 1,000 km | 10 m | 319.23 | 438.58 |
| 6,371 km | 1 m | 51.96 | 170.85 |
| 6,371 km | 10 m | 170.85 | 170.85 |

The new availability assumption removes local-liquid limitation in these cases, not a prescribed deficit in total water. The 10 m Earth-radius control has identical annual global precipitation in both modes for all ten years, consistent with its preceding negligible local-demand shortage. This does not imply equal ownership, regional trajectories, or a realistic ocean circulation model.

For version 8, the year-two precipitation difference between 900/450-second coupling is **0.0601%** at 1,000 km and **0.0270%** at Earth radius, for both active depths. This is one global-summary refinement check, not componentwise/spatial convergence or ten-year temporal qualification. The maximum relative global mass residual across all runs is `8.95e-16`; the maximum regional/body-ledger residual is `5.57e-15`, below unchanged `1e-12` gates.

The 1 m Earth-radius pooled case still retains **6.84%** of initial mobile water in closed dry terminal stores at year ten. Stable global rainfall therefore does not establish stationary storage, and common ocean availability does not resolve closed-lake geometry/evaporation.

All four legacy decade final budgets, shared model pins/settings/recipes, maximum residuals, and replay results reproduce the preceding preparation record exactly. Annual summaries use a different floating-point reduction: this report differences global cumulative totals, while preparation sums regional differences. Their largest P/E discrepancy is `5.69e-13 mm` (`2.61e-15` relative), not a change in the physical state or a widened water-budget gate.

Annual precipitation/evaporation difference leading cumulative totals, not independently exact integrals. Numerical success and a smaller-step repeat do not establish climate realism, arbitrary-seed stability, spatial convergence, or seasonal stationarity. No preparation or clock reset is performed.

## Reproduction and next gates

```sh
cargo test --locked --manifest-path native/Cargo.toml --lib reference_pool
cargo test --locked --manifest-path native/Cargo.toml --test reference_pool
cargo run --release --locked --manifest-path native/Cargo.toml --example reference_pool_report -- --decade
cargo run --release --locked --manifest-path native/Cargo.toml --example reference_pool_report -- --refined
npm test
```

Add `--output NEW_FILE` to retain a full generated report without terminal-output truncation. The report tool refuses to overwrite an existing file. Four allocator tests, seven integration tests, the full native/72-Node regression suite, formatting, and Clippy with warnings denied pass. No GUI qualification is claimed; the new mode is deliberately not displayed.

Next: body-aware annual regional diagnostics and longer refined controls, review of fast internal availability versus finite redistribution, then explicit display ownership/inspection before desktop promotion. Closed-lake area/storage/spill, evolving geography, thermodynamic freezing, atmosphere/energy calibration, weather, and ecology remain separate gates. Do not infer readiness from sustained rainfall alone.
