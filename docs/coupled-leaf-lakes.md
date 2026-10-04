# Bounded coupled leaf-lake exchange

Opt-in **`seasonal-moisture-9` / schema 9**, headless only. This is an applied physical ownership/exchange candidate, not another frozen probe. It connects the [exclusive closed-leaf geometry](closed-leaf-lakes.md) to seasonal precipitation, melt, finite evaporation and delayed runoff. Defaults, models 3–8, old checkpoints, and desktop protocols remain unchanged. Milestone D is incomplete.

## Selected assumptions, not established lake physics

Each initially dry minimum leaf has one finite liquid owner at its existing terminal. The immutable bed and exclusive whole-region columns determine its positive-depth exposed footprint. Fast internal availability is assumed within that leaf only; other leaves and reference bodies cannot fund its demand. No parent storage is borrowed.

Multiplying an interval evaporation depth by surface area is consistent with [HEC-HMS's evaporation accounting](https://www.hec.usace.army.mil/confluence/hmsdocs/hmstrm/reservoir-modeling/reservoir-modeling-concepts-and-equations/evaporation). That reference does **not** validate our vapor-deficit response, whole-column exposure, split order, common-fraction allocation, fixed land-temperature normals, or inactive submerged soil. No external solver/code is copied and no calibration is claimed.

The new choice is deliberately bounded: fixed bed/thermal/wind forcing, growing/contracting surfaces within a single minimum leaf, and **no crossing of its first connection**. A closed root has no fictitious external drain. There is no lakebed leakage, groundwater, bathymetric circulation, heat/latent-energy budget, thermodynamic ice, floating snow or evolving rainfall-temperature feedback. Static runoff can still travel under a newly flooded column; this retains the old delayed receiver network, not a lake hydraulic model.

## Half-stage order and exclusive transfers

The existing two half-local/routing stages around one full atmospheric transport stage remain. For each half-local stage:

1. Derive the exposed mask from each lake's authoritative high/low terminal stock, before local inputs or evaporation. Zero liquid exposes no columns; exact zero-depth shelves are dry. Capture each exposed region's potential from pre-deposition vapor and its existing monthly capacity, with the positive-temperature/enable gates.
2. At exposed regions, perform the existing precise rain/snow deposition and snowmelt closure, with local liquid/soil evaporation, infiltration, soil drainage and liquid-runoff generation suppressed. Soil high/low components remain separately owned and unchanged. Existing transit is also kept separately; it is not an evaporation donor.
3. Transfer the resulting **entire local liquid high/low pair** into that leaf's terminal lake, then clear the local pair. This includes existing surface liquid, warm rain and actual snowmelt exactly once. Snow remains regional. Persist a compensated gross capture ledger at the actual source region.
4. Once all local inputs are credited, check every lake's first-connection bound before any evaporation. Allocate finite grants concurrently over that lake's **pre-stage exposed mask** with the existing common-fraction allocator. Debit its terminal owner once and credit each actual requesting region's vapor once. Record the grant as `terminalEvaporation` at the atmospheric contact region, which can be a nonterminal leaf member in version 9.
5. Apply the old delayed runoff pass. Actual terminal deliveries credit the corresponding finite reference body or closed lake, preserving contact provenance. Check lake bounds again. Arrivals become available in the next half-stage.

Nonexposed leaf members retain the old land closure. The old point-terminal evaporation path is suppressed throughout coupled leaves, including when dry. Exposure growth or contraction affects the **next half-stage**, not already chosen requests or land transfers. A newly flooded endpoint may therefore still retain separately owned local liquid until the next capture; it is not already counted in the lake level. This is an explicit split lag requiring refinement, not continuous shoreline interception. Re-exposed land resumes the land closure with its preserved soil and snow.

The stand-alone `Lake::evaporate` probe freezes exposure at its own operator input. Coupled version 9 instead freezes exposure before the entire local stage and can spend subsequently captured inputs on that footprint. Both reuse the same surface index and finite allocator; they are not claimed to be the same coupled operator.

## Bounds and transactional refusal

The existing represented-mass capacity check runs before mass-to-volume rounding can hide a positive low tail. An exact mass-capacity pair at the first connection refuses, as does any represented mass above it. A negative low tail below capacity is not declared full merely because its derived level rounds to the sill. The component's geometric capacity, not another rounded hierarchy total, governs this bound.

All-input-before-evaporation means an unsupported crossing cannot be hidden by immediately evaporating back below it. Neither overflow nor a tiny residual is discarded into an external collector, returned to the ocean, or reassigned to another basin. Invalid geometry, overflowing arithmetic, unresolvable levels and failed ledgers reject the complete caller interval, preserving all stocks, low parts, transfer totals/corrections and the clock. The unchanged ten-year clock bound is not reset or extended.

Construction rebuilds and checks the basin hierarchy and static drainage against the supplied bed/body IDs, rejecting stale caches. Geometry preparation adds these existing analyses once. The retained exclusive columns and region-owner index remain O(N + K); half-stage mask/allocation work is linear in regions/leaf members apart from storage-index lookup. This is not a planet-scale runtime qualification.

## Accounting and persistence

Lake liquid remains in the existing `terminalWaterKilograms` / `terminalLowKilograms` at its unique terminal. It is **not** also a regional surface stock, reference-body stock, new lake-stock array, or cumulative capture total. `owned_stock_components` still counts physical components once.

For each region, the local liquid identity gains its actual captured high/low transfer as an output. For each lake:

```text
lake liquid = actual terminal delivery
            + sum(member liquid capture)
            − sum(member actual lake evaporation)
```

The lake starts empty. Its independent identity replaces only the old endpoint-only terminal identity; snow, soil, local liquid, vapor, delayed receiver-graph, reference-body and global identities remain checked. Witness ledger 10 denotes a coupled lake. A globally balanced fabricated transfer between two lakes must still fail. Existing local capture/allocation arithmetic bounds use 32 machine epsilons; cumulative/global/vapor gates remain `1e-12`. No balance repair or tolerance increase is introduced; two-component arithmetic is still finite precision, not mathematical exactness.

Schema 9 requires every model-8 precision/body pin and additionally:

- `closedLakeExchange: "frozenLeafExposure"` in resolved settings.
- `closedLakeModelVersion: "closed-leaf-exchange-1"`.
- `cumulativeLakeCaptureKilograms` and `cumulativeLakeCaptureLowKilograms`, N normalized compensated **gross transfer** components. They are zero outside lake leaves and at day zero.

These capture components are not spare liquid and are excluded from inventory iteration/totals. The optional budget field `cumulativeLakeCaptureKilograms` also describes a gross transfer, not remaining stock. In version 9, `terminalEvaporation` describes lake-to-vapor provenance over all leaf members; it is not necessarily a local terminal debit. Old observers and model-8 stationarity diagnostics explicitly refuse these new semantics. Existing desktop writers refuse before writing bytes. The surface/probe inspection API accepts matching native models 8/9, but its independent probes do not advance either model.

Absent new fields preserve old JSON shapes; present null rejects. Schema/model/settings/pins must agree, array shapes and pair normalization are validated, and geometry is regenerated from the recipe on restore. No implicit migration is introduced. Exact replay means supported same-build continuation, not cross-platform bitwise promises or proof that any ledger-balanced checkpoint describes a real historical trajectory.

## Validation and next gate

Eight integration controls cover actual generated exchange and independent lake identities, aligned hourly/daily batching, complete JSON continuation, submerged nonterminal evaporation without a duplicate owner, preserved soil and delayed transit, cold snowfall/warm snowmelt with disabled evaporation, full-interval first-connection rollback, missing/null/shape/pin refusal, independent balanced-lake/capture forgery, stale geometry, dry/all-water limits, legacy all-water state equality, and obsolete observer/desktop/diagnostic refusal. Directed funded states are explicitly ledger-balanced inputs, not claimed generated histories.

The [retained matched decade record](data/coupled-leaf-lake-validation.json) reports model 9 at 900/450-second coupling, native checkpoints, per-lake actual capture/delivery/evaporation, geometry, soil, budgets and failures. The coupled report omits frozen endpoint probes; they would not be the applied annual evaporation. Stand-alone model-8 reports retain their old shape and independent probe semantics.

Both ten-year runs complete: 20 annual budgets and 40 lake surface observations, no numerical/geometric refusal. Maximum relative global mass residual is `1.06e-15`; maximum regional/body/lake identity residual is `5.57e-15`, below unchanged gates. Independent report-level annual liquid differences reconcile capture/delivery/evaporation within `6.85e-16` relative; this is another binary64 audit, not an exact arithmetic oracle. Full JSON checkpoints round-trip exactly with nonzero signed capture low components in both cases. Further-hour continuation at the ten-year bound is explicitly not applicable; shorter generated test states verify exact actual continuation.

| Terminal | Year-ten depth / 900 s | Year-ten depth / 450 s | Absolute refinement difference |
| --- | ---: | ---: | ---: |
| 73 | 8.261609 m | 8.261460 m | 0.149283 mm |
| 91 | 0.028082 m | 0.028125 m | 0.043262 mm |

All recorded surfaces still expose only their terminal column. Region 73 remains at approximately **23.855%** of first-connection capacity, not overflowing. Its year-ten 900-second actual flows, divided by terminal area, are approximately **256.565 mm capture + 647.344 mm delayed delivery − 45.637 mm lake evaporation**. Captured rain/melt is no longer mistaken for newly generated routed runoff. Continued filling remains visible, not classified as a solved stationarity problem. Region 91's corresponding capture/delivery/evaporation nearly balance.

Year-ten global precipitation is `170.860938 / 170.816011 mm/year` for 900/450 seconds. Smaller-step differences in this one recipe are not spatial/componentwise convergence or an ensemble qualification. The complete legacy 900-second model-8 decade report exactly reproduces the preceding retained component report, including its independent probes. Full `npm test` passes (72 Node tests); the final eight new controls, all-target compilation, warnings-denied Clippy, format and diff checks also pass after additional stale-geometry/local-capture guards. Those guards leave both complete new decade reports unchanged. The ordinary native suite retains its pre-existing ignored long release qualification; no new skips were added.

Numerical completion, replay and a smaller-step repeat do not establish calibrated evaporation, stationary climate, arbitrary-seed/resolution reliability or desktop readiness. Continued filling below the first connection is not erased to force preparedness. Next: explicit active spill recipient/backpressure and transfer ownership, including a lake that first reaches another still-unfilled lake; then spill/merge integration and qualified dynamic display. The existing static certificates alone cannot authorize those transfers.

```sh
cargo test --locked --manifest-path native/Cargo.toml --test lake_exchange
cargo run --release --locked --manifest-path native/Cargo.toml --example closed_lake_report -- --coupled --decade --output artifacts/new-coupled-lake-report.json
cargo run --release --locked --manifest-path native/Cargo.toml --example closed_lake_report -- --coupled --decade --refined --output artifacts/new-coupled-lake-refined-report.json
npm test
```

The report refuses to overwrite existing evidence files. No GUI qualification or default promotion is claimed.

Subsequent follow-up: [model-10 bounded leaf spill](bounded-leaf-spill.md) now applies unique recipient transfers with explicit pending ownership and full lower-sill passage. Model 9 retains its refusal boundary and checkpoint semantics; general parent merge and concurrent spill policy remain open.
