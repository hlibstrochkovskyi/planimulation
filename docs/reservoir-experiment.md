# Isolated reservoir experiment: milestone C3a

Implemented scope: a standalone Rust volume-pulse experiment for one reservoir, with a prepared storage curve, persistent inventory, an optional external collector, and validated JSON checkpoints. Experiment version `isolated-reservoir-1`. This is not a planetary hydrology solver or a desktop animation; world recipes remain `basins-1`, protocol 7, and basin analysis remains `basin-analysis-1`.

## Boundary and interpretation

This increment isolates the first storage problem before introducing neighboring reservoirs. `Basins::isolated_leaf_reservoir(node, initial_volume)` copies only a minimum branch's bed columns and reference areas. It cannot modify the world, initial water, hierarchy, or random streams. A leaf with a geometric spill gets an **imposed external collector** at that threshold. A root leaf remains closed and has no finite capacity. The initial experiment stock is supplied explicitly; it is not automatically copied from or substituted for the world's existing water.

Merged branches are rejected. A parent's storage curve includes several children, but that does not justify giving unequal, partly filled children a common water level. Nested and multiway fixtures verify rejection and independent leaf storage, **not** coupled fill–spill–merge behavior. A contact edge is still an adjacency witness, not a computed receiving path.

The collector has unlimited acceptance, no backpressure, and no connection to another simulated lake. Its cumulative receipt is recorded as outflow from the experiment. It is not a hidden drain added to the closed planet. A future network must replace this boundary with an explicit receiving reservoir and conserve their combined stock.

Each pulse prescribes a volume in m³, followed by instantaneous equilibrium. There is no time step, flow rate, overflow delay, river discharge, rainfall field, evaporation, infiltration, withdrawal, erosion, or automatic basin merge/split. Pulse count is not a calendar. These limitations make the experiment useful for accounting tests, not a claim about flood timing.

## Storage curve and updates

For bed columns with heights `z_i` and areas `A_i`:

```text
V(h) = Σ A_i × max(0, h − z_i)
```

Sort columns by height and original index, combine exact-height plateaus, and store the volume and active area at each distinct height. Advance volume between heights using `active_area × height_difference`, avoiding subtraction of large absolute-datum moments. This gives a piecewise-linear curve with a strictly increasing volume index. Inversion selects a segment by binary search and computes `height + (stock − segment_volume) / active_area`. Above the last column, a closed reservoir continues with its entire support area. Zero stock has no physical water level (`null`), not a fictitious surface at the lowest bed.

For a collector threshold `H`, capacity is `C = V(H)`; the threshold must exceed all columns in the detached leaf. For each nonnegative input `I`:

```text
retained = min(I, C − previous_stock)     # closed: retained = I
outflow  = I − retained
new_stock = previous_stock + retained
initial_stock + cumulative_input = new_stock + cumulative_outflow
```

An exact fill to capacity has zero outflow. Only excess spills. The authoritative quantity is the stored volume, never a stock recomputed from a rounded output height. Capacity includes all stored water, not just additional input needed. Initial stock above capacity is rejected rather than silently discarded or exported.

Preparation costs `O(M log M)` time and `O(M)` memory for `M` reservoir columns. Storage/level queries and pulse updates use binary searches in `O(log M)`; there is no recursive traversal or iterative settling loop. Checkpoint creation copies the column data; restoration rebuilds the index. This is not yet an implementation optimized for every basin on a planet.

## Numerical contract and transactions

- Pulse accounting uses `max(1e-9 m³, input × 1e-12)` tolerance. The cumulative budget uses `max(1e-9 m³, total_supplied × 1e-12)`.
- Reconstructing stock from an absolute output level uses `max(1e-9 m³, stock × 1e-10)`, matching initial-water fitting. The reported storage residual remains visible; it is never used to correct inventory.
- A generated regression case (`reservoir-world-0`, subdivision 3, branch 51) has bed −4,462.697299737766 m, area 827,658,943,104.4869 m², and quarter-capacity stock 261,139,818,745.05887 m³. Rounding its absolute water level produces approximately 0.37637 m³ reconstruction error (relative 1.44e-12). This motivated keeping level reconstruction distinct from the stricter inventory ledger.
- Non-finite/negative inputs, invalid geometry, unsupported versions, inconsistent ledgers, numerical overflow, and unresolvable levels fail. A positive addition completely lost at the accumulator's precision also fails. Extreme datums or tiny pulses may legitimately be unsupported; the code does not conceal them by adding epsilon heights or resetting volume.
- Every pulse computes and audits a candidate state before committing it. On failure, inventory and pulse count remain unchanged. Zero input is valid and increments the pulse count without altering stocks.

## Checkpoints and developer entry point

A checkpoint contains the experiment version, ordered columns, explicit boundary condition, initial stock, cumulative input/outflow, current stock, and pulse count. Curve indexes and water level are derived, not independent sources of truth. There are no RNG states or pending flows in this instantaneous experiment. Restore validates fields, bounds, stock representability, and budget consistency; it does not authenticate history or establish adjacency for manually supplied columns. A column-only request assumes one communicating reservoir geometry.

Checkpoint numbers use a scoped raw-JSON decimal reader and Rust's `f64` parser so restoration preserves serialized finite values. The existing `serde_json` dependency enables `raw_value`, adding no crate dependency. Its global floating-point parser is unchanged: enabling `float_roundtrip` globally would risk changing existing world recipes under the same model version. Read checkpoints directly from their serialized JSON, not through an intermediate parser that might round numbers first. Save/load continuation is tested against uninterrupted execution in the supported build; cross-platform exactness is not claimed. These checkpoints cannot resume a generated world or the unrelated diagnostic diffusion.

Run the checked-in, hand-verifiable scenarios:

```sh
cargo run --release --locked --manifest-path native/Cargo.toml --example reservoir -- docs/scenarios/reservoir-sill.json
cargo run --release --locked --manifest-path native/Cargo.toml --example reservoir -- docs/scenarios/reservoir-closed.json
```

Both use a 2 m² column at 0 m and a 3 m² column at 1 m, with input pulses `[1, 1, 5, 5, 4, 0]` m³. The first has a collector at 3 m; the second has no outlet.

| Cumulative input | Collector-case level | Stored | Cumulative collector receipt |
| ---: | ---: | ---: | ---: |
| 1 m³ | 0.5 m | 1 m³ | 0 m³ |
| 2 m³ | 1 m | 2 m³ | 0 m³ |
| 7 m³ | 2 m | 7 m³ | 0 m³ |
| 12 m³ | 3 m | 12 m³ | 0 m³ |
| 16 m³ | 3 m | 12 m³ | 4 m³ |

The closed case instead retains all 16 m³ at level 3.8 m. The report records its initial checkpoint, each pulse and residual, capacity (`null` when closed), and final checkpoint. To continue, use `finalCheckpoint` as the next request's `checkpoint` and supply new `inputsCubicMeters`. Use `-` to read a request from stdin. Input is limited to 32 KiB and 1,024 pulses; unknown fields are rejected. The complete request must succeed before any JSON report is written, so a later invalid pulse cannot yield a misleading partial success report.

## Validation

Tests cover independently known weighted capacities and levels; area transitions; exact fill versus overspill; initial nonzero stock; closed bowls, slopes, and flats; minimum plateaus; isolated nested/multiway leaves and rejected parents; pulse partitioning; state independence; checkpoint corruption and exact JSON continuation; transactional failures; 30 seeded sequences of 100 pulses; datum/area scaling; detached leaves from six generated worlds; unchanged world arrays; and a 40,962-region closed flat. Direct column sums independently check the indexed curve. The CLI's two fixtures, replay, malformed requests, and input bounds run in the standard `npm test` suite through Cargo's `--all-targets` option.

Recorded September 24, 2026:

- `npm test` passed: 48 Rust tests (including 9 reservoir integration tests and 3 example tests) and 43 TypeScript tests; type checking and release native compilation passed.
- Rust formatting and Clippy with warnings denied passed for all targets.
- The release example produced the expected 12 m³ stored / 4 m³ collected result, and the closed scenario retained 16 m³ at 3.8 m. File and stdin reports matched byte-for-byte. Missing arguments/files, malformed JSON, and oversized input returned failure with no stdout report.
- Production build and the development desktop suite passed. Headless and desktop retained default fingerprint `2e66ac09`, 10,242 regions, and 5,792,952 model-array bytes. Vite's existing renderer-chunk-size advisory remains.
- No new packaging run, visual feature, or performance benchmark is claimed. The packaged C2b build remains a previous artifact; this developer experiment is run from Cargo, not a new desktop control.

## Next increment

[C3b](reservoir-pair.md) implements a closed-pair experiment; [C3d](nested-reservoir.md) extends experimentation to bounded binary nested filling with unique sill contacts and checkpoints. C3a remains the isolated external-collector case. General multiway/alternative-outlet allocation and assignment of existing planetary inventories remain future work; repeatedly refitting a global coverage target is not a dynamics model.
