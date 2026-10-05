# Bounded outgoing spill from common-sill lakes

October 5, 2026 applied continuation. Opt-in headless model/schema 14 extends [bounded parent receivers](common-sill-receiving.md) with an actual outgoing transition. A full eligible one-level parent can send its owned excess through one unique geographic outlet when the receiver accepts the complete arrival. This is native functionality, not general nested hydrology or a desktop/default promotion.

## Physical scope

The [Fill–Spill–Merge paper](https://esurf.copernicus.org/articles/9/105/2021/) motivates depression storage and explicit filling/merging relationships. This increment reuses existing hierarchy and storage geometry; it does not implement or validate that paper's full solver. There are no new random events, calibration coefficients, or synthetic collectors.

Eligible groups remain disjoint, initially dry, one-level parents with immediate minimum-leaf children. Their inherited birth high/low contents and incremental common-layer curve are unchanged. The actual parent's next sill supplies the outgoing head. A generated passage crosses that sill and follows checked static downhill/flat routing to a reference-body contact or closed terminal; it cannot re-enter the parent or its children. Multiple alternatives remain ambiguous rather than being chosen by index.

Local filling and eligible sibling activation happen before overflow classification. Once the parent's surplus pair is exactly its represented next-sill capacity, any remaining input queues belong to that parent, not to its inactive children. Several such queues are one source owner. The operator debits their complete high/low excess pairs, credits actual receiver stocks and records typed parent events. Inherited contents and saturated surplus remain at the source; the child liquid stores remain zero. Subsequent actual evaporation can contract that parent as before.

Accepted destinations are:

- A finite reference body at the actual wet contact, under the existing fixed-level backpressure check.
- An independent leaf whose whole arrival fits below its first sill and whose represented capacity is not above the supplying head.
- An already active eligible peer parent whose whole arrival fits both the supplying-head and next-sill ceilings. Its child is not revived; the existing parent-owned incoming marker records this portion.

Exactly one spilling owner may exist at a resolver stage. A second parent or unrelated overflowing leaf, ambiguous outlet, over-capacity receiver, unsupported higher-level activation, backpressure, or unresolvable arithmetic refuses the complete seasonal caller interval. Clock, prior stocks and histories remain unchanged; no partially applied prefix or event list is published. Combined leaf/parent output is bounded by 4,096 transfer packets per caller interval.

This mode does **not** send unhandled external leaf arrivals onward through a full parent: those incoming arrivals still require the complete-arrival fit inherited from model 13. Nor does it apply downstream receiver-overflow chains, activate a higher parent, allocate simultaneous sources, or integrate discharge/travel time. A closed parent root retains its existing storage behavior and gets no fabricated outlet. Exposure remains frozen within half-local stages; bed/drainage and reference-body levels remain fixed. Displayed heights and geometric prism capacities remain floating-point approximations, not stock ownership.

## Independent identities and persistence

`mergedLakeState.frontier.outgoingSpill` adds two compensated gross flow arrays: group-indexed `cumulativeOutgoing` and region-indexed `cumulativeIncoming`. These are provenance, **not water stocks**. Group order is the deterministic eligible-candidate order; destination indices identify actual terminal/body contacts.

The independent geographic identity rebuilds each nonzero departure's unique parent outlet and aggregates its complete pair at that actual destination. Departures require historical activation. Parent lifetime identities include gross outflow; leaf and reference-body identities include gross received water. The parent-owned portion of arrivals is still excluded from the deactivated child's history and included in the actual receiving parent's history. These checks remain meaningful after contraction/splitting. They do not reconstruct the chronological order of every historical packet or constitute a complete causal event log.

| Contract | Pin |
| --- | --- |
| Seasonal checkpoint | `seasonal-moisture-14`, schema 14 |
| Settings mode | `frozenCommonSillOutlets` |
| Closed-lake exchange | `closed-leaf-exchange-6` |
| Parent state | `common-sill-outlet-4` |
| Nested lifecycle | `common-sill-lifecycle-3` |
| Outgoing provenance | `common-sill-outgoing-transfer-1` |

Exact pins, both incoming-parent and outgoing markers, normalized high/low pairs and exact shapes are required. Missing/null fields, wrong pins/shapes, false owners and geographically inconsistent paired transfers reject. Models 12/13 omit outgoing provenance and retain their preceding behavior, with no automatic migration. Native `parent_spill_connections` observes alternatives only; `closed_lake_frontier` continues exposing current owners without treating gross flow as liquid. Old desktop/observer contracts refuse this mode before writing or mutating state.

Numeric storage adds `16 * (G + N)` bytes to model 13 for outgoing/incoming arrays, excluding containers and generated route storage. Dense validation remains `O(N + G)` plus checked route reconstruction during model construction. Large-grid runtime/memory and broad repeated forcing are not qualified here.

## Verification

Four added synthetic operator controls cover a full parent's known 500 kg excess entering a finite body, a 4,000 kg-capacity independent leaf, or an active peer parent; neither parent's deactivated children regain liquid. They also check multiple parent sources, ambiguous sills, receiver overflow, fixed-level backpressure and signed low tails of `±2^-50 kg` in actual receiver stocks and both flow pairs. These controlled chains are not generated-world restore fixtures.

Seven generated integration controls use unmodified seed `first-light`, subdivision 2, initial coverage `0.3`. Parent basin 37 has children 8/156 and the unique passage `8 → 143 → 41`, ending at reference body 2, contact 41. Directed rain/runoff histories are debited from real finite reference stocks with matching histories; they are **not spontaneous weather or seed-ensemble qualification**. An independent `i128` dyadic audit checks all owned stock components before/after transfer, separately from floating-point budget tolerances, and integer capacity arithmetic checks the expected outflow.

Controls cover initial and preexisting full parents, several queues under one owner, actual delayed terminal delivery, later evaporation, geographic/shape/pin/null corruption, current-owner observation, unsupported observers/wire formats, exact full-save continuation, unchanged model-13 refusal and complete caller rollback for a concurrent unrelated leaf. No new test is ignored.

The [retained report](data/parent-outlet-validation.json) completes four forty-day comparisons with model 13: 162 regions with 71% coverage at 900/450-second ceilings, 162 regions with 30% coverage, and 642 regions with 71% coverage. Every accepted daily physical checkpoint matches exactly after removing only new pins and zero outgoing provenance; continuation is exact. These cases have no parent outflow and test regression, not nested hydrology or convergence. Maximum ordinary relative global/local residuals are `1.81e-16` / `2.08e-15`.

The separate directed transfer delivers `26,003,678,340 kg` to the actual contact. Source inherited contents retain their `-16 kg` low component, and source surplus stays exactly at its `26,003,678,350,734,924 kg` capacity. After an evaporating hour it contracts and retains its outgoing history. The scalar global diagnostic residual is `256 kg` against `1.42e18 kg` initial mobile water; the independent transfer stock audit is exact. Post-contraction scalar-vapor residual is `3.7890625 kg`, not exact zero. Legacy/concurrent refusals preserve the complete state. No tolerance is widened.

Full `npm test` passes native all-target tests (including all 49 library units and seven outgoing integration controls), TypeScript checking and all 72 Node tests. The twelve leaf-spill operator units and seven outgoing integration controls also pass in release mode. The generated-transfer test's final added observer/wire assertions pass in separate debug and release integration reruns. Warnings-denied all-target Clippy, formatting and diff checks pass. The new report repeats exactly, and rebuilding the preceding model-13 report reproduces its complete retained object. The existing ignored long annual-snow release qualification is unchanged and was not rerun. No GUI validation or new ignored tests are claimed.

```sh
cargo test --release --locked --manifest-path native/Cargo.toml --lib seasonal_moisture::leaf_spill
cargo test --release --locked --manifest-path native/Cargo.toml --test parent_outlet
cargo run --release --locked --manifest-path native/Cargo.toml --example parent_outlet_report -- --output artifacts/new-parent-outlet-report.json
npm test
```

Next gates are higher/nested parent ownership, downstream overflow/backpressure and explicit simultaneous-source policy, followed by broader forcing qualification and dynamic display. Weather, groundwater, ecology and climate calibration remain separate work.
