# Reversible one-level common-sill lakes

Implemented October 5, 2026 as opt-in native **`seasonal-moisture-12` / schema 12**. This extends seasonal lake exchange with applied contraction and splitting. [Model 11](common-sill-lakes.md) retains its original above-birth-only contract and checkpoint output. Neither mode is the desktop default; general hydrology and milestones C/D remain incomplete.

## Physical transition and approximation

Eligible groups are the same disjoint, initially dry parents whose immediate children are all minimum leaves. No nested parent, ocean merge or new receiving-frontier policy is introduced. Parent geometry uses the existing incremental curve above its common-sill birth level.

Exactly saturated siblings remain separate when there is no positive owned excess. A parent activates only when every child is exactly full **and** there is positive pending group input. Their complete represented capacities transfer to the parent birth pair; pending excess funds its common layer. A negative low component is still below capacity. The canonical zero-depth state avoids immediate merge/split chatter without adding a hysteresis mass threshold.

During evaporation:

1. If frozen demand is below the complete surplus pair, the parent remains active and pays actual regional grants from surplus.
2. If demand covers surplus, a bounded proportional allocation consumes the entire high/low common-layer pair.
3. Complete birth contents return to the original child capacities. The parent ceases owning water.
4. Each child independently pays the remaining potential demand on its own regions, limited by its own inventory. A connecting shelf is no longer wet at birth; its unmet potential request cannot borrow child water.

Children may therefore contract to different levels. There is no fictitious common below-sill surface, collector, drain or ocean return. Full children can later merge again when actual new excess arrives.

The existing half-stage footprint and thermal/atmospheric demand remain frozen. The evaporation operator changes donors when it exhausts the common layer; it does **not** integrate continuously changing wetted area, shoreline thermodynamics or hydraulic redistribution. Below-birth child contraction updates exposure for the next stage. This is an explicit bounded operator approximation, not a calibrated drying law.

The topology ordering follows the idea of [Barnes, Callaghan and Wickert's Fill–Spill–Merge](https://esurf.copernicus.org/articles/9/105/2021/), not that paper's complete solver or a claimed splitting implementation. [HEC-HMS reservoir evaporation](https://www.hec.usace.army.mil/confluence/hmsdocs/hmstrm/reservoir-modeling/reservoir-modeling-concepts-and-equations/evaporation) supports converting evaporation depth to volume through surface area; it does not validate our frozen-demand endpoint approximation.

## Endpoint arithmetic

The usual finite allocator intentionally leaves a small owned reserve and cannot implement exact layer exhaustion. The endpoint operator instead emits actual scalar debit packets, including the represented low tail. Requests are summed and debited in numeric demand order, not by region ID.

An exactly reconciling nominal proportional allocation is accepted, including exact tied shares. Otherwise only a **unique largest physical request** may carry a correction bounded by `32 * epsilon` of its nominal share. Equal-largest unresolved ties refuse instead of selecting a region ID for the remainder. Insufficient local demand, unrepresentable positive debits, overflow or out-of-bound correction also refuse the whole caller interval. This policy is a numerical reconciliation, not physical priority for an arbitrarily named region.

Each receiver's complete evaporation transfer is first accumulated as a compensated pair. It is then combined with that region's scalar atmospheric vapor stock and rounded once. This avoids treating pair decomposition as separate physical arrivals. Scalar vapor is still an approximation: the receiver-rounding residual must remain within its local `32 * epsilon` arithmetic bound and the unchanged global/vapor/local `1e-12` budget gates. A complete transfer that cannot change its vapor receiver refuses. No claim of bit-exact atmospheric water conservation is made.

## Persistence and historical ownership

`mergedLakeState.frontier` adds per-eligible-group merge/split counts and four compensated gross ledgers:

| Ledger | Meaning |
| --- | --- |
| `pendingToParent` | Previously leaf-owned pending excess handed off at activation |
| `captureByParent` | Actual regional liquid capture while the parent owns it |
| `deliveryToParent` | Actual delayed terminal delivery while the parent owns it |
| `evaporationFromParent` | Actual common-layer regional evaporation |

These are provenance, not spendable water. Child-to-parent and parent-to-child birth transfers are derived from represented child capacity times the corresponding count, using a two-component product. Counts obey the clock bound and `merges - splits = activeParent`. They are not a complete chronological or causal event log.

Every eligible group's lifetime parent identity remains checked after splitting, not just while it is active. Every child identity includes the historical birth transfers and separates parent-owned capture/delivery/evaporation from leaf-owned flows. Captured rain on a now-dry parent-only column remains attributable to the parent that actually owned it. Historical accounting eligibility does not make that column currently wet or intercept later land water.

Active parents must have positive surplus, the exact inherited birth pair, and zero child liquid. Wrong pins, shapes, counters, owners, duplicate/ineligible parents, malformed pairs and missing required lifecycle state reject before use. Day-zero counts and flows are zero. All mutations remain provisional until the complete seasonal budget accepts the caller interval.

Pins are `frozenCommonSillFrontier`, `closed-leaf-exchange-4`, `common-sill-frontier-2`, and nested `common-sill-lifecycle-1`. Model 11 omits the optional lifecycle field and refuses one supplied under its old pin. Present null and unknown fields reject. No automatic migration or desktop wire-format broadening occurs.

Storage is `O(N + K)`: four new high/low flow pairs per region and two counts per eligible group. Geometry remains disjoint one-level storage; splitting uses an indexed child lookup rather than rescanning every leaf for each parent. Runtime/memory at the largest grid is not yet qualified.

## Validation and remaining gates

Directed unit controls cover exact three-child ownership, parent-only capture, a merge/split/remerge cycle, independent child contraction, integer-product birth transfers, signed tails, exact tied shares, unresolved-tie refusal, request permutation and unrepresentable debits.

Generated-world integration controls cover seasonal splitting of parent 37 into children 8/156, preservation of its inherited −16 kg component, another explicitly funded merge, parent-only rain provenance after drying, exact checkpoint continuation before/after splitting, malformed lifecycle state, next-parent spill rollback, obsolete observer/wire refusal, and aligned hourly/daily caller partitioning. Alignment matters: different partial endpoints are different integration schedules, not a determinism failure.

Four forty-day controls compare **complete physical checkpoints** with model 11 before any natural merge: 162 regions at 900/450-second ceilings with 71% reference coverage, a 30%-coverage case, and 642 regions at 71% coverage. Removing only new state/pins gives exact equality. These are fixed numerical regressions, not seed-ensemble reliability or spatial convergence. Directed transitions use explicitly ledger-funded water from finite reference bodies, not spontaneous climate evolution.

The [reproducible report](data/common-sill-frontier-validation.json) separately retains ordinary runs, an applied merge/split on unmodified generated geography, their budgets and exact continuation, and next-parent spill refusal. All four ordinary runs complete forty days; the largest measured global relative residual is `1.81e-16` and local identity residual is `2.08e-15`. The directed common-layer exhaustion returns both child owners with retained low components and zero final pending input. Atmospheric vapor's directed ledger residual is `15.203125 kg`, within its recorded relative gate, not exact zero. The report refuses file overwrites and records ordinary failures rather than dropping them.

```sh
cargo test --locked --manifest-path native/Cargo.toml --lib seasonal_moisture::merged_lake
cargo test --release --locked --manifest-path native/Cargo.toml --test common_sill_frontier
cargo run --release --locked --manifest-path native/Cargo.toml --example common_sill_frontier_report -- --output artifacts/new-frontier-report.json
npm test
```

Next-parent overflow, unrelated spill into an active parent, nested ownership and ambiguous/concurrent spill remain explicit atomic refusals. Those receiving-frontier transitions are the next implementation gate, followed by broader repeated forcing and qualified dynamic display. Weather, groundwater, ecology, climate calibration and general planet-wide water dynamics are not promoted by this increment.

Final verification: full `npm test` passes native all-target tests and all 72 Node tests. The seven parent/operator unit tests and all nine new integration tests pass, with a separate release-mode integration run. Warnings-denied all-target Clippy, formatting and diff checks pass. The newly generated model-11 report exactly matches its preceding retained report object; the model-12 report exactly matches the new retained evidence. The pre-existing ignored long release qualification is unchanged; no new tests are ignored.
