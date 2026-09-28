# Shared-sill allocation experiment: milestone C3e

Implemented scope: a closed, initially dry junction of two or more leaf reservoirs connected through one shared sill plateau. Experiment version `spill-junction-1`; explicit policy version `receiver-weighted-capped-1`. This is a new standalone experiment, **not** a silent extension of C3d. World recipes remain `basins-1`, protocol 7; desktop water, C2a/C3c analyses, and C3d's binary/unique-contact rejection rules are unchanged.

## Geometric eligibility

Use C3d's bounded column/edge input validation (1–128 regions), then build C3c's exact child–plateau graph. The root must merge only leaves, and exactly one plateau may contact multiple children. Every root child must contact that same connected plateau. One-child dead-end plateaus are allowed but are not outlets. Nested children, separate connecting plateaus, and alternative sills are rejected before accepting any water.

This distinguishes a true shared junction from the equal-height chain `bowl → sill → bowl → sill → bowl`: the latter cannot be treated as an all-to-all connection past its unfilled middle bowl. The geometry check is stronger than merely finding the same spill height.

Multiple contacts to one leaf are allowed because they enter the same communicating reservoir. They do not multiply its allocation weight. C3c still reports all contacts and can provide adjacent-region witness paths through the shared plateau; C3e does not assign per-contact discharge or build a quadratic table of flows between every source and receiver.

Inputs enter explicitly named leaf inventories, not arbitrary surface regions. There is no additional dry-bed routing approximation in this experiment. The size limit is a correctness-laboratory boundary, not a claim of planetary scalability.

## Explicit experimental policy

The caller must supply a finite positive dimensionless weight for every leaf, in canonical analysis child order. Equal weights implement equal sharing among still-unfilled receivers; unequal weights prescribe a preference. There is no inferred conductance, sill width, roughness, travel time, pressure difference, or scientific calibration. A geometry with more mesh contacts does not gain more weight. Scaling all weights equally should preserve allocations within floating-point precision.

A pulse is a **simultaneous batch**, containing a nonnegative volume for every leaf, including explicit zeros. Missing, duplicate, or reordered IDs fail rather than changing processing priority.

1. Each input fills its own leaf first, up to its sill capacity.
2. Only exactly saturated sources contribute excess to a common spill pool.
3. Allocate that pool among remaining deficits in proportion to receiver weights. A receiver gets no more than its deficit; redistribute its excess share among the still-unfilled receivers.
4. Once every leaf reaches the sill, replace all leaf stocks with one root stock. Additional input raises its common level, accounting for sill and upper-bank columns.
5. Later batches add to root storage. There is no external loss or collector.

For deficits `d_i`, weights `w_i`, and available spill `E`, the allocation is:

```text
allocated_i = min(d_i, lambda × w_i)
sum(allocated_i) = min(E, sum(d_i))
common_surplus = max(0, E − sum(d_i))
```

The implementation removes receivers whose proposed share reaches their cap, then recomputes shares. Every pass either saturates at least one receiver or finishes; at most one pass per receiver is needed. Weights are normalized by the largest active weight to avoid summing unscaled extreme values. Unrepresentable allocations are rejected, not rounded into a hidden drain.

At the final unsaturated pass, the largest branch ID receives the arithmetic remainder after the other shares. That remainder must remain positive, fit its deficit, and differ from its proportional share by no more than the budget tolerance. This is documented rounding treatment, not a physical priority rule; relabeling comparisons use numerical tolerances rather than promising bitwise symmetry.

## What order comparisons mean

For fixed weights, initially dry leaves, one shared plateau, and nonnegative inputs only, the separate-state result can also be characterized by cumulative direct inputs `D_i`:

```text
stock_i = min(capacity_i, D_i + lambda × w_i)
sum(stock_i) = sum(D_i)                 # before common storage
```

Tests solve this independently by bisection and compare with the incremental allocator. They compare simultaneous batches, single-entry sequences, reversed batches, and one combined batch. Partial-state cases are checked explicitly so a later common lake cannot hide allocation bias. This characterization does not justify order independence for nested receivers, distinct sills, withdrawals, changing weights, or a timed flow model.

Per-pulse transfer reports need not be partition-independent. With capacities `[4, 3, 2]`, weights `[1, 1, 3]`, and inputs `[6, 3, 0]`, one batch records 2 m³ accepted from the spill pool. Applying `[6, 0, 0]` and then `[0, 3, 0]` records 2.5 m³ across the two batches, since some water first occupies the second leaf. Both end with 9 m³ at the sill. Equal final state is not equal transfer history.

## Storage, accounting, and persistence

Separate storage contains exactly one stock per leaf. Merged storage contains only the root; both representations cannot coexist. A dry leaf has no level. Exact connection has zero sill depth and is reported as `atSill`; positive common depth is `merged`. C3a indexed prism curves supply geometry queries only; no external-collector inventory is advanced.

Each pulse reports locally retained input, spill supplied per source, spill accepted per receiver, and input into common storage. It does **not** invent a source-to-receiver flow matrix for a pooled allocation.

```text
cumulative input = sum(exclusive stored volumes)
pulse input = new total − previous total
spill supplied = spill accepted + input into common storage
```

Budgets use `max(1e-9 m³, scale × 1e-12)` tolerance; level reconstruction uses `max(1e-9 m³, total stock × 1e-10)`. Errors leave the previous state untouched. Positive additions lost at stock precision, invalid levels, numeric/counter overflow, and noncanonical checkpoints fail. Budget residuals are recorded, never used to reset coverage or overwrite stocks.

Checkpoints save both versions, ordered geometry, explicit weights, exclusive storage, cumulative input, and pulse count. Derived topology and curves are rebuilt; existing scoped raw-decimal readers preserve finite serialized numbers. Restore checks state consistency, not historical authenticity or an external policy signature. To compare different policies, start separate experiments from the same dry geometry; silently changing saved weights does not reproduce the original experiment.

No time integration, hydraulic rates, rainfall, evaporation, drying/splitting, planetary inventory assignment, or desktop animation is implemented. A finite shared-sill allocation is not a full fill–spill–merge solver.

## Developer scenario

```sh
cargo run --release --locked --manifest-path native/Cargo.toml --example spill_junction -- docs/scenarios/spill-junction.json
```

Three unit-area beds at 0, 1, and 2 m meet a unit-area sill at 4 m. Capacities are `[4, 3, 2]` m³; weights are `[1, 1, 3]`. All fixture input enters branch 0:

| Pulse input | Separate stocks / common stock | Result |
| ---: | --- | --- |
| 4 m³ | `[4, 0, 0]` | Source reaches sill |
| 2 m³ | `[4, 0.5, 1.5]` | Weighted split |
| 2 m³ | `[4, 2, 2]` | Last receiver caps; remainder goes to middle |
| 1 m³ | `9` | All meet at 4 m; sill still has zero depth |
| 4 m³ | `13` | Common level 5 m, including sill storage |

With equal weights, the second row instead becomes `[4, 1, 1]`. Both configurations conserve water; choosing between them requires an explicit modeling decision, not more randomness.

Requests use `{"start":{"dry":SETUP},"inputs":[BATCH,...]}` or `{"start":{"checkpoint":CHECKPOINT},"inputs":[BATCH,...]}`. A batch is an array of `{ "branch": ID, "volumeCubicMeters": V }` entries in the reported child order. IDs are local to the analysis, not persistent lake IDs. Use `-` for stdin. Input is limited to 32 KiB and 1,024 batches; unknown fields fail, and the entire request must succeed before stdout is published. The report includes capacities, plateau ID, full geometric analysis, pulse budgets, and initial/final checkpoints.

For `N` regions, `E` edges, and `K` leaves, retained storage beyond input is `O(N + E + K)`: each bed column belongs to at most one leaf curve and the common curve. Preparation includes existing C3c analysis and indexed curves; allocation is at most `O(K²)` per pulse plus `O(K log N)` level checks. The bounded example is not a full-network performance benchmark.

## Validation

Tests cover weighted/equal splitting, receiver saturation, simultaneous local-first input, merging and common sill storage, exact checkpoint continuation, weight scaling, area/datum changes, relabeling, repeated contacts, actual adjacent passage witnesses, atomic errors, invalid topology/policies/checkpoints, and a 127-leaf junction within the 128-region bound. The two-leaf limit is compared against the existing C3b pair using multicolumn bowls and upper banks.

Forty seeded weighted junctions receive 60 batches each (2,400 updates) and are compared against an independent cumulative-input/bisection oracle and direct bed-column volume sums. Reversed, combined and partial single-entry sequences check order/partition behavior within tolerance. CLI tests cover the fixture, exact continuation, versions, malformed inputs, and request bounds.

Validation completed September 28, 2026:

- `npm test` passed: type checking, release native compilation, 96 Rust tests across all targets, and 43 TypeScript tests. Final transfer-history and CLI count-limit assertions also passed in targeted reruns.
- Rust formatting, Clippy across all targets with warnings denied, and `git diff --check` passed.
- Release CLI file/stdin reports matched byte-for-byte. The fixture allocated `[0, 0.5, 1.5]` m³ in its first spill and ended at 13 m³ / 5 m. Restoring and adding 4 m³ produced 17 m³ / 6 m, exactly matching uninterrupted execution. Eight invalid argument/request cases failed without stdout, including a late invalid input and a count-limit request below the byte bound.
- Production build and `npm run test:desktop` passed. Headless and desktop fingerprints remain `2e66ac09`; default world data remains 10,242 regions and 5,792,952 model-array bytes.
- The existing Vite renderer-chunk-size advisory remains. No new packaging run, visual feature, scientific calibration, or full-network performance benchmark is claimed.

## Next increment

Define a controlled network combining this explicit allocation policy with nested receiver entry and more than one sill plateau. Receiver eligibility and transit through saturated bowls must be derived from actual connections, not copied from this all-to-all shared-plateau case. Preserve the existing experiment versions as references; planetary initialization and scalable state storage remain separate integration tasks.
