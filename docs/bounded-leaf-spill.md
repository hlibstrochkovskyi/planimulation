# Applied bounded leaf spill

Implemented October 5, 2026 as opt-in headless **`seasonal-moisture-10` / schema 10**. This extends the applied model-9 lake exchange with actual capacity-bounded transfers, not another read-only spill certificate. Defaults, desktop protocols and successful legacy trajectories remain unchanged. Milestones C and D remain incomplete.

## Selected physical approximation

Inputs first fill their own exclusive minimum leaf. Overflow enters the first geographic recipient and fills its available storage before continuing. A full receiving leaf can pass remaining input only when its outlet is strictly lower than the supplying sill. Equal-sill full siblings need a parent merge and refuse; they are not treated as a drain to an ocean.

This borrows the depression-aware ordering described by [Barnes, Callaghan and Wickert: Fill–Spill–Merge](https://esurf.copernicus.org/articles/9/105/2021/), **not its complete algorithm or implementation**. The paper's merging stage is not implemented here. A static downhill certificate alone never establishes that a receiving lake is ready to pass water.

Redistribution is instantaneous within the exchange stage. There is no resolved hydraulic discharge, spillway coefficient, flow velocity or travel time. This is a coarse fast-redistribution hypothesis, not a prediction of flood hydrographs. Reference bodies retain model 8's fixed geometry and common mobile availability; their generated reference level must be below the supplying sill. Their mobile inventory does not determine a new ocean level, and no inactive deep-water stock is borrowed.

The supported resolution has one overflow source after all local queues have filled their own leaves, one unique certified route at each visited source, and resolved sill levels. A closed receiving leaf's first capacity must not be above the supplying head. Multiple geometric alternatives, concurrent overflowing source queues, equal/higher-sill backpressure, unresolved levels and parent merging reject the entire caller interval. A closed root stores its own input and gains no external outlet.

## Ownership and exchange order

Schema 10 introduces three separate compensated regional pairs in `leafSpillState`:

| Account | Meaning | Included in owned inventory? |
| --- | --- | --- |
| `pendingInput` | Incoming liquid owned at a closed terminal, waiting for bounded application | Yes, once |
| `cumulativeOutgoing` | Gross flow across each closed leaf's certified edge | No |
| `cumulativeIncoming` | Gross arrivals at the actual closed terminal or wet contact | No |

Settled lake liquid still belongs to the existing terminal high/low pair. Pending liquid is **not** above-capacity lake storage and does not determine exposed area. Accepted ordinary steps settle their queues to zero. A restored ledger-balanced queue is an explicit input state, not an inferred past climate history.

Each half-local stage freezes the exposed-area mask before deposition, as in model 9. Captured local liquid/rain/melt now credits its terminal's queue; all local queues fill before overflow selection. Spill settles before finite lake and reference-body evaporation grants. The delayed runoff stage then credits actual dry-terminal queues or wet-body contacts and settles those queues separately. Newly changed exposure affects the next stage's requests. Soil, snow and delayed transit keep their separately owned model-9 semantics. Monthly thermal/wind normals and static drainage remain unchanged.

For the generated reference, region 91's first recipient is lake 73. Lake 73's lower outlet reaches reference body 1 at actual contact 66:

```text
91 full → fill 73's remaining capacity → only then return excess to body 1 at 66
```

Water is debited from the original pending owner until it reaches its actual final inventory. Full intermediate leaves record matching incoming/outgoing gross flows, not a duplicate stock. Events retain the source terminal, original input terminal, exact static passage/descent certificate and actual scalar grant. They are returned only for accepted caller intervals; this is not a persistent history/timeline system.

## Numerical and checkpoint contract

Transfers reuse the normalized compensated stock and conservative available/remaining-capacity floors. Up to four scalar grants decompose an owned pair or a capacity deficit. Thresholds require `high == capacity` and `low == 0`: a negative low component remains below the sill even when the derived display level rounds to it. No snapping, stock reconstruction, global repair, arbitrary last-recipient remainder or discarded input tail is used.

Zero input moves nothing. Representable subnormal inputs can move. Nonfinite/overflowing components, loss of an entire positive credit **or debit**, stalled progress or exceeding the four-grant bound refuse. In particular, a tiny receiving deficit cannot gain a grant whose subtraction leaves a much larger donor pair unchanged. Each debit/credit is checked against the unchanged local `32 × epsilon` arithmetic guard. The pair representation is finite precision, not exact arithmetic for arbitrary scales. A resolution caps visited edges and direct edge-grant events at 4,096, with linear visited-owner checks. The whole caller interval also caps edge-grant events at 4,096; exceeding the caller bound requires a shorter interval, not truncated diagnostics.

Each lake independently checks:

```text
settled liquid + pending input
  = actual delayed terminal delivery + captured local liquid + spill arrivals
    − spill departures − actual regional lake evaporation
```

Each reference-body identity adds only spill arrivals at its actual member contacts. A separate graph identity reconstructs arrivals from the outgoing ledgers on regenerated unique routes. Wrong owners, unsupported outgoing routes and arrivals without a geographic source reject. Thus a globally balanced teleport cannot pass merely because total water is unchanged. Pending components are included once in state iteration and the global budget; capture and spill flows are excluded from owned stock.

Model/schema/settings must agree, including:

- `closedLakeExchange: "frozenLeafExposureWithSpill"`;
- `closedLakeModelVersion: "closed-leaf-exchange-2"`;
- `leafSpillState.modelVersion: "closed-leaf-spill-transfer-1"`;
- all six nested component arrays, with exact regional shapes and normalization.

The compensated soil/surface/terminal and connected-reference-body prerequisites remain. All new queues/flows are zero at initialization. Missing fields preserve legacy formats; present null, missing nested arrays, wrong versions or foreign owners reject. Models 3–9 omit the new state rather than silently migrating it. Complete provisional state is cloned, validated and committed atomically, including queues, gross-flow pairs, existing transfer corrections and clock. Global, atmospheric and local cumulative identity gates remain `1e-12`; the ten-year clock bound is unchanged.

Old regional observers, model-8 preparation monitoring and desktop checkpoint/display writers refuse model 10 before callbacks, state changes or output bytes. Native lake surface observation accepts the matching model-10 recipe without mutating it. The routing-disabled check now distinguishes intercepted lake liquid/evaporation from river transfers; this also fixes that previously refused model-9 combination. Successful legacy trajectories are not changed.

## Validation and scope

Six unit controls cover an exhaustive integral-unit dyadic fill oracle, positive/negative low tails, capacity headroom below one ULP, subnormal/lost-input behavior, real synthetic reciprocal chain certificates with same-sill merge refusal, ambiguous routes/reference backpressure, a closed root with no invented outlet, and transactional caller-event bounds.

Eight integration controls cover the real generated two-lake chain, stopping at the unfilled recipient, passing through a full lower-sill recipient to the actual wet contact, independent integral-`i128` summation of all owned components in the quiet planetary-scale fixture, full JSON/next-hour replay, signed near-threshold tails, concurrent-source rollback, strict schema/owner/graph forgery, actual warm-rain and delayed-runoff threshold crossings, forty-day generated exchange, aligned hourly/daily batching, dry/all-water limits, model-9 all-water physical equality, and obsolete observer/protocol refusal. The directed funded inputs come from finite mobile reference water and are explicitly **not** claimed naturally generated climate histories.

The [retained matched report](data/bounded-leaf-spill-validation.json) contains 900/450-second forty-day ordinary runs, two directed real-recipe transfers per cadence, complete native replay checks, first-spill certificates, final spill accounts and retained concurrent-source refusals. The ordinary runs do not reach a first sill; their zero spill-event counts are retained, not presented as natural overflow evidence. Directed controls exercise actual applied overflow, independently of the ordinary rainfall trajectory.

Both ordinary runs complete without refusal and with zero final pending lake input. Their maximum relative global residual is `1.76e-16`; maximum local identity residual across both runs is `2.81e-15`. All six successful states round-trip and continue another hour exactly. The directed controls stop in lake 73 when it is still unfilled, or fill both leaves before returning excess to contact 66. Both concurrent-source controls refuse with the complete checkpoint unchanged. These are bounded numerical checks, not a calibrated hydrological model, ensemble reliability or spatial convergence proof. The rounded global budget can show a 512 kg residual at a planetary inventory of approximately `3.63e18 kg`; the independent integral-component fixture verifies unchanged represented ownership without repairing that display reduction.

The complete first-year annual record of legacy model 9, its recipe and resolved settings exactly reproduce the preceding retained decade's first year. Its full checkpoint round-trip and actual next-hour continuation pass. No legacy runoff-enabled trajectory or threshold rule is silently changed to make the new mode pass.

Full `npm test` passes, including native all-targets and all 72 Node tests. The final lost-debit/visited-edge guards were followed by six new unit controls, eight new integration controls, all eight legacy lake controls, warnings-denied all-target Clippy, format/diff checks, and both complete report reruns. Both report objects exactly match the retained evidence after those guards. The existing ignored long release qualification is unchanged; no new test skips are introduced. No GUI or largest-grid performance qualification is claimed.

```sh
cargo test --locked --manifest-path native/Cargo.toml --lib leaf_spill
cargo test --locked --manifest-path native/Cargo.toml --test leaf_spill
cargo run --release --locked --manifest-path native/Cargo.toml --example leaf_spill_report -- --output artifacts/new-leaf-spill-report.json
cargo run --release --locked --manifest-path native/Cargo.toml --example leaf_spill_report -- --refined --output artifacts/new-leaf-spill-refined.json
npm test
```

The report refuses to overwrite evidence files. The separately pinned [model-11 common-sill mode](common-sill-lakes.md) now applies exclusive storage and merging for one-level all-dry parents; model 10 retains its original same-sill refusal. Contraction/splitting, general concurrent receiving frontiers and qualified dynamic display remain open. Do not grow this bounded leaf adapter into an unversioned collection of implicit overflow exceptions. No default, desktop, stationarity, ecology or calibration promotion is implied.
