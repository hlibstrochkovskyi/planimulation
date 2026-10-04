# Body-aware annual stationarity diagnostics

Implemented October 4, 2026 as `body-seasonal-preparation-analysis-1`. This read-only native observer supports **only seasonal-moisture-8**. It tests the [finite reference-water pool](reference-water-pool.md) without changing stocks, rates, conservation tolerances, clocks, checkpoints, or defaults. A diagnostic candidate is not physical climate readiness, ecological suitability, or a prepared state.

## Ownership first

Version 8 owns liquid once per connected reference body, not independently at each wet region. Dividing that stock among regions for a stationarity test would invent a distribution that the physical model does not simulate.

The observer therefore retains two distinct sets:

- The six existing regional stocks: land liquid, all local snow, soil, transit, closed-dry terminal water, and vapor. Wet regional liquid/terminal components are validated as zero.
- One high/low liquid pair per actual reference body, indexed by regenerated sorted positive body IDs.

Regional and body components are counted once in the total change. Opposite changes in different stocks, regions, or bodies cannot cancel: absolute high/low component differences are taken **before** summation. Representation-only high/low redistribution remains visible. This is a conservative component-change diagnostic in real arithmetic, not exact physical mass change or a certified outward-rounded binary64 bound.

For each body, report its ID, immutable reference area, current high/low stock, signed stock change, component-L1 change in kilograms and mm WE, and annual rain/melt/actual-terminal-delivery/liquid-evaporation. Signed change differences corresponding high and low components before summing; this preserves the directed equal-mass representation control's cancellation instead of first rounding large high+low totals.

Body mm WE divides by the **entire reference footprint**. It is not an independently spendable regional stock, a simulated lake surface, or a new water level. Local terminal mm likewise divides by the receiving region's area, not a derived lake surface.

Regional output reports changes by each of the six stocks and the maximum local change's region. Closed-dry terminal inventory is shown separately. These measurements distinguish near-constant precipitation from continued trapping/redistribution.

## Recorded criteria

The observer reuses the existing validated `preparation::Criteria` values, with an explicit additional body-owner gate:

| Condition | Default threshold |
| --- | --- |
| Total regional **plus body** component-L1 / initial mobile water | ≤ 0.001 |
| Maximum regional component-L1 / region area | ≤ 1 mm WE |
| Maximum body component-L1 / reference-body area | ≤ 1 mm WE |
| Each regional annual flow difference | ≤ 0.1 mm WE + 0.001 × larger annual total |
| Consecutive eligible comparisons | ≥ 3 |

All conditions must pass. A small global ratio cannot excuse a changing small body or local sink. Regional flow columns remain precipitation, evaporation, generated runoff, snowfall, and actual terminal delivery. The four per-body flow columns are **explanatory outputs**, not additional eligibility tests.

Thresholds are uncalibrated project policy, separate from physical budget tolerances. Reusing numerical thresholds across differently partitioned physical models does not make their ownership/spatial tests equivalent scientific benchmarks.

The first annual observation has no previous annual-flow interval: its eligibility flag is null. With default criteria the earliest candidate is year four. `positiveAtmosphericCyclingObserved` separately requires positive annual P/E; quiet, dry, and tiny-flow cases must not be labeled climate-ready. Annual endpoints do not certify within-year consistency or future weather stability.

The version-7 observer's **cold-lock proof is not inherited**. A cold region's former liquid can be available to warm atmospheric recipients in the same version-8 body. The new observer makes no cold-accessibility classification. Legacy `seasonal-preparation-analysis-1` continues to support only versions 3–7 and keeps its original results and semantics.

## Read-only lifecycle

`body_preparation::Monitor` borrows an immutable model. Initialization validates criteria, model version, complete physical state, body areas, and a whole-year anchor. Each observation requires exactly the next complete 365-day year and revalidates the state against that model. Duplicate, skipped, partial, foreign, nonfinite, and decreasing-flow observations reject before observer history is committed.

The monitor retains O(N+B) preceding regional/body stocks and flow totals, not every tick or a decade archive. Observation is O(N+B); this is a complexity description, not a largest-grid benchmark. No rendering or per-tick callback is involved.

Restoring a whole-year checkpoint and constructing a new observer reanchors **diagnostic history only**. The physical clock and stocks remain unchanged; preceding annual flows are not fabricated. Observer history is not a new simulation checkpoint field.

Annual flow values difference cumulative binary64 leading totals. Body flows difference sums of member cumulative totals; regional flows difference each region before summing. These reductions can differ by roundoff and are not independently exact annual integrals. Transfer-summation corrections are never counted as water.

## Report and retained controls

`reference_pool_report --preparation` adds annual assessments and their diagnostic pins/criteria. Version 7 controls use the unchanged legacy observer; version 8 uses the body-aware observer. The flag selects report version 2. Without it, report version 1 and all existing output fields remain unchanged.

The [retained record](data/body-preparation-validation.json) includes the same reference worlds at 1,000/6,371 km radius, 1/10 m initial active depth, and matched legacy controls. Standard and refined reports use daily callers and explicit coupling ceilings. Failed physical intervals and failed diagnostic observations are retained as failures, not stationarity outcomes. At the existing ten-year bound, next-hour continuation is not applicable, not an extra successful step.

At 900-second coupling, both pooled 1,000 km cases first become candidates in **year seven**. Their annual flow comparison first passes in year five and stays within the recorded criteria through year ten. They have no closed-dry terminal stock in this generated geography.

The pooled Earth-radius cases do **not** qualify by year ten. At 1 m initial depth, year-ten total component-L1 is about **1.44%** of initial water; at 10 m it is **0.144%**. Both have approximately **858.28 mm** maximum regional change at region 73 and **7.20 mm** body change. Their largest remaining regional change is closed-dry terminal inventory, not snow. The 1 m case ends with **6.84%** of initial mobile water in closed dry terminals despite nearly constant annual rainfall.

The full ten-year 450-second controls preserve this classification: first candidate year seven for both small-radius cases, no candidate for either Earth-radius case. Earth-radius year-ten changes remain approximately **858.26 mm** locally and **7.20 mm** per body. Refining the time step changes year-ten pooled global precipitation by **0.0600%** at 1,000 km and **0.0263%** at 6,371 km (absolute difference divided by the larger total). This is one temporal refinement comparison, not demonstrated convergence.

All **16 numerical cases** and native JSON checkpoint round trips pass. Across both suites the maximum recorded relative global mass residual is `1.453251716080013e-15`; the maximum local ledger residual is `5.5667492212634965e-15`, without changing the existing `1e-12` budget gates. A round trip at the ten-year limit does not establish continuation beyond it.

Removing only newly added diagnostic fields reproduces all eight previous standard physical reports exactly. The refined suite's first two annual physical entries exactly reproduce all 16 corresponding entries in the prior short refined record. All **60** comparable legacy annual assessments (four standard and two refined controls, matched by complete recipe and settings) reproduce the original preparation record exactly; other seeds are not matching controls. Full `npm test` passes the native/TypeScript/72 adapter checks. The final body module has three passing unit tests and four passing integration tests; all-target warnings-denied clippy, formatting and diff checks pass. No desktop or GUI validation was performed for this headless addition.

These are fixed generated cases on a coarse grid, not arbitrary-world qualification, spatial convergence, calibration, or justification for increasing the clock limit. Different radii also change geography, so they are not geographically paired radius interventions.

## Reproduction and next gate

```sh
cargo test --locked --manifest-path native/Cargo.toml --lib body_preparation
cargo test --locked --manifest-path native/Cargo.toml --test body_preparation
cargo run --release --locked --manifest-path native/Cargo.toml --example reference_pool_report -- --preparation --decade --output artifacts/body-preparation-new-report.json
cargo run --release --locked --manifest-path native/Cargo.toml --example reference_pool_report -- --preparation --decade --refined --output artifacts/body-preparation-new-refined-report.json
npm test
```

Output files must be new; the report refuses to overwrite existing evidence. Unit controls cover opposite body changes, low components, equal-mass representation redistribution, invalid arithmetic, and every eligibility gate. Generated integration tests cover exact observed/unobserved complete state, body areas/flows, replay, quiet/empty/dry candidates, invalid/foreign observations, restored anchors, and explicit diagnostic-version isolation.

The measured remaining gate is **closed-water surface/storage representation**, not a demand for more ocean water or a longer clock. Review how a closed sink's collected water acquires an exposed area and a conservative return/spill path. This mechanism is not implemented here. Physical validation of fast body availability, paired spatial-resolution controls, explicit desktop ownership displays, and any eventual prepared-state export remain separate work.
