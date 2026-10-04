# Bounded common-sill lake parents

Implemented October 5, 2026 as opt-in native **`seasonal-moisture-11` / schema 11**. This applies a bounded parent transition inside seasonal exchange, not just a storage experiment. Defaults and desktop models remain unchanged. General hydrology and milestones C/D remain incomplete.

## Selected scope and physical approximation

An eligible parent has at least two immediate children, every child is an exclusive minimum leaf with a dry terminal, and the entire parent footprint is initially outside reference water bodies. These one-level groups are disjoint. Nested parent activation and merging with initial oceans/lakes are unsupported.

All children must reach their own represented common-sill capacities before the parent activates. A negative low component remains below capacity even if its leading component or displayed level equals the sill. A partly empty sibling cannot be skipped. Model 10's unique geographic route can fill that sibling; when all children become full, model 11 transfers their contents to one parent and applies remaining group-owned input above the common sill.

This borrows the topology-aware ordering of [Barnes, Callaghan and Wickert's Fill–Spill–Merge](https://esurf.copernicus.org/articles/9/105/2021/), not its complete solver or source code. In particular, the paper's general depression hierarchy is not implemented as a seasonal receiving frontier here. Existing fast within-stage redistribution, fixed reference-body geometry, static downhill routing, and monthly thermal/wind forcing remain uncalibrated approximations.

The accepted parent can receive captured rain/melt/liquid and delayed runoff to its child terminals. Its surface can rise and evaporate **above the birth level**. The complete caller interval refuses atomically if:

- evaporation demand would consume inherited water below the common sill, requiring splitting;
- input exceeds the parent's next-sill incremental capacity;
- an unrelated spilling leaf needs an already active parent as its recipient;
- a newly merged recipient has remaining externally owned overflow without a supported receiving-frontier policy;
- existing model-10 route, concurrency, backpressure or numerical bounds fail.

This is not a permanently locked lower layer: a below-birth drying request is rejected, not silently capped to make the parent survive. A shorter interval can postpone that unsupported transition, but does not implement it. The next physical gate is explicit contraction/splitting and ownership below the common sill, followed by nested and external receiving frontiers. No artificial drain, invented ocean return or stationarity repair is introduced.

## Exclusive ownership and incremental geometry

Schema 11 adds `mergedLakeState` with a strict model pin and sorted active parents. A parent owns two compensated pairs:

| Pair | Meaning |
| --- | --- |
| `birthHighKilograms`, `birthLowKilograms` | Complete inherited contents of the saturated children |
| `surplusHighKilograms`, `surplusLowKilograms` | Additional liquid above the common-sill level |

These are layers of **one exclusive parent**, not copies of child inventories. At activation every child's terminal high/low stock becomes zero. Gross capture/spill ledgers remain provenance, not new stocks. Pending input remains explicitly owned until settled. The state component iterator and global budget count both parent pairs exactly once; `mergedLakeWaterKilograms` is a rounded diagnostic total.

Birth contents are accumulated from represented child mass capacities, not replaced by a rounded parent geometric volume. In the retained generated fixture the inherited pair has a **−16 kg low component**. Discarding that component would change represented ownership despite an apparently unchanged planetary-scale total.

Geometry is anchored at the parent's common-sill level `b`. For incremental height `h >= 0`, the additional storage is

```text
deltaVolume(h) = sum(area[r] * max(h - max(bed[r] - b, 0), 0))
```

The inverse uses only surplus mass divided by water density. At zero surplus the surface is at `b`; below-sill columns are already wet. A connecting column at exactly `b` has zero depth until surplus is positive. Above birth, that connecting column contributes to incremental wetted area. Inherited mass stays authoritative even when independently rounded geometry would disagree slightly with its sum.

Relative height is authoritative for exposure. Absolute elevation is optional when adding the datum loses level resolution. Positive mass that underflows the derived volume/height, over-capacity pairs or excessive local reconstruction error refuse; no stock is snapped to a display level. Capacity markers reuse the isolated curve's boundary representation, but no external collector receives water.

Only disjoint one-level footprints are stored; every region appears in at most one candidate parent. This avoids copying every ancestor subtree and uses `O(N + K)` geometry/index storage, alongside the existing leaf layout. Activation scans groups once during input settlement, while a visited spill edge checks only its touched candidate groups. This is not a largest-grid runtime qualification.

## Seasonal coupling and validation

Half-stage exposure is frozen as before. Active children are omitted from leaf exposure/evaporation. New parent wet regions intercept local liquid/rain/melt into the parent's input queue; unexposed high banks retain land behavior. Soil, snow and delayed transit keep their separate owners. Newly expanded wetness affects the next half-stage, not already computed requests.

Parent evaporation allocates actual regional grants from surplus only after checking that the total frozen demand remains above birth. Every positive grant must produce a represented donor debit. The existing small allocation reserve remains owned; it is not a sink. The parent birth pair remains unchanged in every supported step.

One aggregate parent identity replaces the child identities after activation:

```text
birth contents + surplus + pending child-terminal input
  = group captured liquid + actual group terminal deliveries + group spill arrivals
    - group spill departures - actual group regional evaporation
```

Historical internal child-to-child spill cancels in that group identity, while the separate geographic edge ledger still checks its actual route. Reference-body and regional surface identities remain independent. Wrong/duplicate parent nodes, altered birth contents, residual child liquid, invalid surplus pairs and outside-footprint capture reject. Shape/version checks precede indexed access. Global, vapor and local identity gates remain `1e-12`; the ten-year clock bound is unchanged.

Pins must agree:

- `closedLakeExchange: "frozenLeafExposureWithSpillAndMerge"`;
- `closedLakeModelVersion: "closed-leaf-exchange-3"`;
- `mergedLakeState.modelVersion: "common-sill-parent-1"`;
- the existing `leafSpillState.modelVersion: "closed-leaf-spill-transfer-1"`.

Missing new state is valid only under an older model, not schema 11. Present null, unknown fields, missing required nested fields and inconsistent pins reject. Parents start empty and are validated against regenerated recipe geometry. Complete caller state, including all inherited/surplus low components, queues, flow corrections and clock, is provisional until the final budget accepts it. No silent checkpoint migration is performed.

`Model::merge_candidates` exposes eligible geometry, not active lakes. `Model::merged_lake_surfaces` checks model/state compatibility before deriving active surfaces. The old closed-leaf observer rejects model 11 rather than showing deactivated children as dry lakes. This is native inspection, not a new desktop view or persistent merge-history system.

## Evidence and limits

Two new directed unit tests cover a three-child common sill, zero-depth shelves, independent integral ownership before/after merging and evaporation, negative low tails preventing activation, a closed root with no outlet, below-birth refusal and unrepresentable tiny donor debits.

Eight integration tests cover:

- real generated siblings `8` and `156` becoming parent `37` on the unmodified `first-light` world with 30% initial water coverage;
- preservation of the inherited −16 kg component, with an independent `i128` dyadic-unit sum of every owned component;
- one actual unique geographic source filling its sibling, then wetting the connecting region `141`;
- rain on newly wet parent-only regions, evaporation on the whole parent surface and exact JSON continuation;
- underfilled siblings, split/next-spill rollback and malformed/exclusive-owner checkpoints;
- three days of aligned hourly/daily evaporation from an active parent;
- five forty-day physical-state comparisons with model 10: dry, all-water, ordinary, refined-cadence and eligible-parent geography, before any natural merge.

Directed histories are explicitly ledger-funded from finite mobile reference water. They demonstrate applied transitions, not spontaneous climate histories or initial-world equilibration.

The [retained report](data/common-sill-merge-validation.json) records four forty-day runs: 162 regions at 900/450-second cadence with 71% initial coverage, a 162-region 30%-coverage control, and a 642-region 71%-coverage control. All complete without refusal, with no naturally activated parent or spill event and zero final pending lake input. The largest measured global relative residual is `1.81e-16`; the largest local identity residual is `2.08e-15`. Every final state round-trips and continues another hour exactly. These are numerical regression checks, not ensemble reliability or spatial convergence.

The same report records birth-level merging, above-birth storage/exposure and evaporation, complete replay, and two retained refusals: below-birth splitting and next-parent spill, both with unchanged complete state. The directed parent grows from wet regions `[8,156]` to `[8,141,156]`. No GUI/default promotion, calibrated discharge, groundwater, weather, ecology or largest-grid qualification follows from these results.

Full `npm test` passes, including native all-target tests and all 72 Node tests. The final local birth-addition guard was followed by both parent unit tests, all eight integration tests in release mode, warnings-denied all-target Clippy, format/diff checks and another complete report run. The final report object exactly matches the retained evidence. Both coarse/refined legacy model-10 report objects also exactly reproduce their preceding retained results. The pre-existing ignored long release qualification remains unchanged; no new tests are skipped. Obsolete surface/terminal observers, body monitoring and desktop wire writers explicitly refuse model 11 without callbacks, state changes or bytes.

```sh
cargo test --locked --manifest-path native/Cargo.toml --lib seasonal_moisture::merged_lake
cargo test --locked --manifest-path native/Cargo.toml --test merged_lake
cargo run --release --locked --manifest-path native/Cargo.toml --example merged_lake_report -- --output artifacts/new-common-sill-report.json
npm test
```

The report refuses to overwrite files. Ordinary-run refusals and failed continuation checks remain reportable instead of being dropped. Previous models and desktop protocol/default behavior remain separately pinned; broader active-frontier work must receive a new version rather than silently broadening this contract.
