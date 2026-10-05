# Bounded external spill into active common-sill lakes

October 5, 2026 applied continuation. Opt-in headless model/schema 13 extends [reversible one-level lakes](common-sill-frontier.md) with one receiving transition: an unrelated full minimum leaf can send its actual unique-route excess to an **already active** eligible parent. This is native functionality, not a desktop/default promotion or completion of milestone C.

## Physical scope

The [Fill–Spill–Merge paper](https://esurf.copernicus.org/articles/9/105/2021/) motivates retaining depression storage and explicit filling/merging relationships. This increment is a bounded policy on the existing hierarchy, not an implementation or validation of the paper's complete solver. It adds no new random events or adjustable calibration coefficient.

Eligible parents remain disjoint, initially dry, one-level groups whose immediate children are minimum leaves. Their inherited contents and incremental common-layer curve are unchanged. A child of an active parent no longer owns separate liquid; a route ending at that child's static terminal must therefore credit the parent, not revive the child or teleport the input to a canonical sibling.

The existing leaf resolver still requires a unique geographic passage and decreasing static downhill path, at most one overflowing queued source, and bounded acyclic traversal. Initial local filling and sibling activation happen before overflow routing. Consequently a parent can be active from a previous caller interval or be activated by that local filling before an unrelated overflow is processed. The arrival itself does not activate an inactive parent.

Let `F` be the supplying leaf's absolute first-connection sill, `B` the receiving parent's birth sill, `V_g(h)` its incremental storage curve and `H_next` its next-sill height relative to birth. Require finite `F - B > 0`. The receiving ceiling is

```text
C = min(parent's represented next-sill mass capacity,
        1000 * V_g(min(F - B, H_next)))
```

For a closed root there is no `H_next` ceiling; finite representable storage remains required. The existing curve's boundary is a capacity marker, **not** a collector. The current surplus high/low pair must fit `C`. Actual transfer packets debit the complete donor pair and credit the surplus without modifying inherited birth contents. A positive low component beyond the ceiling is not hidden by a rounded display level.

The complete incoming excess must fit. If it would exceed supplying-head or next-sill capacity, the complete seasonal caller interval refuses without advancing the clock or changing any prior stock, event or ledger. No remainder is discarded, released to a fake sink, or automatically routed into a higher parent. This deliberately excludes general blocked queues and nested backpressure resolution.

Redistribution is still fast within a local stage, not a discharge law or travel-time integration. Exposure and thermal demand remain frozen for each half-local stage; static drainage and fixed reference-body levels retain their previous limitations. Mathematical prism capacity and derived displayed levels remain floating-point approximations.

## Independent ownership and persistence

The actual edge keeps its source, sill passage and destination terminal in the existing `leafSpillState` incoming/outgoing ledgers and accepted events. `mergedLakeState.frontier.spillToParent` adds one compensated gross pair per region for the subset of those arrivals credited while a parent owns the receiver. It is **provenance, not another liquid stock**.

- Each nonzero marker needs an eligible child terminal and historical parent activation. Its gross amount cannot exceed that terminal's actual geographic incoming amount under the unchanged local ledger gate; no positive marker is allowed when physical incoming is exactly zero.
- The parent lifetime identity includes this inflow, even after the parent splits. The child lifetime identity excludes this parent-owned portion from its own incoming liquid. Independently validated geographic edge provenance still rejects moving both markers and incoming flow to an unrelated sibling.
- Existing merge/split counts, represented inherited transfers, capture, delayed delivery and evaporation identities remain checked. Aggregate history does not reconstruct the chronological order of every transfer and is not a complete causal event log.

| Contract | Pin |
| --- | --- |
| Seasonal checkpoint | `seasonal-moisture-13`, schema 13 |
| Settings mode | `frozenCommonSillReceiving` |
| Closed-lake exchange | `closed-leaf-exchange-5` |
| Parent state | `common-sill-receiver-3` |
| Nested lifecycle | `common-sill-lifecycle-2` |

Model 13 requires the new marker pair and exact pins. Missing/null, malformed shape or normalization, wrong owners and inconsistent provenance reject. Model 12 omits the marker and continues refusing external spill into an active parent. There is no automatic save migration or old desktop wire-format broadening. The unchanged `seasonal-closed-lake-frontier-1` observation derives current model-13 owners and surfaces without consuming the marker as water.

Storage adds `16 * N` bytes of numeric marker arrays to model 12, excluding container overhead. Dense accounting remains `O(N + K)`. Largest-grid memory/runtime and broad repeated forcing remain unqualified.

## Verification

Independent synthetic unit controls use a five-region chain with unit areas and known integer capacities: the external source fills to 4,000 kg, its 500 kg excess increases the active parent's surplus from 1,000 to 1,500 kg, the inherited 4,000 kg stays unchanged, and deactivated children remain empty. Additional controls cover a lower supplying head, a receiver already above that head, an exact ceiling, heads above the next sill, nonfinite input, and positive/negative low tails at the ceiling.

Generated integration controls use resolved seed `receiver-2`, subdivision 2, relief scale `0.01`, detail amplitude `3 m`, and initial reference coverage `0.1`. Parent basin 15 has child terminals 22/33; external terminal 15 has a unique real passage to terminal 22. All directed queues are debited from actual finite reference-body stocks with matching evaporation/rain histories. The larger refusal control uses several independent reference donors. These are synthetic histories on unmodified generated geography, **not spontaneous climate transitions** or seed-ensemble qualification.

Seven integration tests check applied transfer and geographic/owner identity, a preexisting receiving parent, full checkpoint replay, evaporation/splitting after external inflow, forged geographic/owner provenance, strict save pins/shapes/nulls, unchanged model-12 refusal, over-capacity caller rollback and obsolete observer/wire refusal. No new test is ignored.

The report separately runs four forty-day comparisons against model 12 (162 regions with 71% coverage at 900/450-second ceilings, 162 regions with 30% coverage, and 642 regions with 71% coverage). It compares complete physical checkpoints at every accepted day after removing only new pins and the zero receiving marker. This checks unchanged calculations without receiving events, not convergence or general reliability. Directed acceptance, subsequent splitting, exact continuation and atomic refusals are retained separately.

The [retained evidence](data/common-sill-receiving-validation.json) completes all four ordinary forty-day runs with exact daily physical-state equality and exact resumed continuation. Largest ordinary relative global/local residuals are `1.81e-16` / `2.08e-15`. The directed route delivers `4,210,064,096.25 kg` to terminal 22's parent-owned surplus while inherited birth contents retain their `+0.03125 kg` low component. After one evaporating hour the parent splits, both children retain their own high/low liquid, and the historical receiving marker remains. Global residual is `4 kg` against `1.90e16 kg` initial mobile water; the post-split scalar-vapor residual is `0.0234375 kg`, not exact zero. The legacy and over-capacity refusals preserve the complete state. No numerical tolerance is widened.

Verification: full `npm test` passes native all-target tests (including all 45 library units and seven receiving integration controls) and all 72 Node tests. The eight leaf-spill operator units and seven receiving integration controls also pass separately in release mode. Warnings-denied all-target Clippy, formatting and diff checks pass. Repeating the new report reproduces the complete retained object exactly; rebuilding the model-12 report reproduces its preceding retained object exactly. The pre-existing ignored long annual-snow release qualification is unchanged and was not rerun; no GUI validation or new ignored tests are claimed.

```sh
cargo test --locked --manifest-path native/Cargo.toml --lib seasonal_moisture::leaf_spill
cargo test --release --locked --manifest-path native/Cargo.toml --test receiving_frontier
cargo run --release --locked --manifest-path native/Cargo.toml --example receiving_frontier_report -- --output artifacts/new-receiving-report.json
npm test
```

Next gates are nested/higher-parent ownership and outgoing spill, explicit ambiguous/concurrent-source policy, broader repeated forcing and qualified dynamic display. Weather, groundwater, ecology and climate calibration remain separate future work.
