# Coupled reservoir pair: milestone C3b

Implemented scope: a standalone, closed, two-bowl experiment with one prescribed sill, independent initial stocks, conservative overflow into the neighboring bowl, and common storage after connection. Experiment version `reservoir-pair-1`. Desktop/world generation stays static, using model `basins-1`, protocol 7, and basin analysis `basin-analysis-1`.

## Geometry and boundary assumptions

The request explicitly declares three disjoint column sets: left bowl, right bowl, and connection/upper-bank columns. Every bowl bed must be below the finite sill height. Connection beds must be at or above it, with at least one column exactly at the sill. All areas must be positive and finite. The two bowls are assumed to connect through this sill and have no other barriers or outlets. Column data alone does not establish geographic adjacency; arbitrary real terrain cannot be passed off as a verified pair by matching heights.

The pair reuses [C3a](reservoir-experiment.md)'s indexed prism-storage queries for each side and their combined geometry. Internal curve objects are geometry-query helpers, not three independently updated inventories. Side capacity is `V_side(sill)`. Common capacity at connection is the sum of these two capacities; the connection columns have zero depth there. Above the sill, those columns must participate in storage. Upper-bank columns enter when their own heights are reached.

This is a closed experiment: there is **no external collector or outflow**. The C3a collector boundary is not attached to either actual pair inventory. Initial stocks must be within each bowl's capacity; overfilled, unequilibrated starting states are rejected, not silently redistributed. Arbitrary connected initial stocks can be restored from a validated pair checkpoint after an earlier run.

## State and pulse rules

Storage has exactly one authoritative representation:

- `separate`: a stock for each side. Levels can differ, and a dry side has no level (`null`).
- `connected`: one total stock, with no duplicated child inventories. The reported phase is `atSill` at the combined threshold, or `merged` above it.

At `atSill`, both sides reach the same height but the sill still has zero depth. This is a limiting connection state, not a claim of positive-depth flow across a dry cell. Above the threshold, both sides share a level obtained by inverting the combined curve. A positive surplus that cannot produce a representable level above the sill is rejected.

Each pulse specifies simultaneous, nonnegative input volumes for left and right:

1. Each input fills its own side up to its capacity.
2. Excess on one side fills the other side's remaining deficit. The source retains its threshold stock; the recipient need not immediately reach the sill.
3. Only when both sides are full does storage switch to one combined reservoir. Remaining surplus raises its common level, including storage over the connection.
4. Subsequent inputs add to common storage regardless of the side of entry.

The implementation reports left-to-right and right-to-left transfers **while filling separate bowls**. `inputToCommonStorageCubicMeters` is surplus entering shared storage, not a directional edge flux. If both sides receive more than their own deficits simultaneously, each fills locally and both surpluses join common storage; the report does not invent opposing cross-sill exchanges. Transfer totals may depend on pulse partitioning even when the resulting equilibrium stocks agree.

Pulse index is not time. Equilibration is instantaneous; no hydraulic rate law, travel time, flood-wave dynamics, evaporation, withdrawal, separation after drying, downstream external spill, or multiway/nested routing is implemented. This is a conservation experiment, not a replacement for the eventual water-cycle solver.

## Accounting, numerical behavior, and persistence

```text
initial_left + initial_right + cumulative_input_left + cumulative_input_right
    = total_stored
```

Internal transfers appear nowhere as new external input. Each pulse also checks `new_total − old_total − pulse_input`. Budget tolerances remain `max(1e-9 m³, scale × 1e-12)`; output-level reconstruction uses `max(1e-9 m³, stock × 1e-10)`, as in C3a. Recorded residuals are never used to replace stock or reset coverage. Exact threshold comparisons are deterministic; no epsilon height is added. Unresolvable additions, overflows, invalid geometry, invalid checkpoints, or unrepresentable output levels return errors before any state is committed.

A checkpoint contains the experiment version, all ordered geometry columns and sill, initial stocks, cumulative input per side, the exclusive storage variant, and pulse count. No transfer queue, RNG, or dynamic rate state exists in this approximation. Derived curves are rebuilt on restore. Numeric checkpoint fields use C3a's scoped exact decimal reader, without changing existing world-recipe parsing. Restore rejects incompatible versions, inconsistent budgets, premature common storage, and noncanonical separate storage when both sides are exactly full. It validates state consistency, not historical authenticity.

Preparation uses `O(M log M)` time and `O(M)` memory for `M` columns. A pulse uses a fixed number of indexed level queries, `O(log M)`, without copying geometry or running an iterative settling loop. Checkpoint creation copies geometry. No performance measurements for a full network are claimed.

## Reproducible developer scenario

```sh
cargo run --release --locked --manifest-path native/Cargo.toml --example reservoir_pair -- docs/scenarios/reservoir-pair.json
```

Left: 2 m² at 0 m and 3 m² at 1 m. Right: 4 m² at −1 m. Sill: 3 m. Connection: 1 m² at 3 m plus 2 m² of upper bank at 5 m. Side capacities are 12 and 16 m³. All input in this scenario enters on the left:

| Pulse input | Total stock | Left level | Right level | Result |
| ---: | ---: | ---: | ---: | --- |
| 12 m³ | 12 m³ | 3 m | Dry | Left reaches sill; no transfer |
| 4 m³ | 16 m³ | 3 m | 0 m | 4 m³ transferred right |
| 12 m³ | 28 m³ | 3 m | 3 m | Both at sill; zero connection depth |
| 10 m³ | 38 m³ | 4 m | 4 m | Merged; 1 m³ over connection |

An additional 22 m³ raises the common level to 6 m and gives 60 m³ total, including 5 m³ over connection/upper-bank columns. Omitting those columns would yield an incorrect level.

Use `-` for stdin. Requests contain a `checkpoint` and `inputsCubicMeters` list of `{ "left": ..., "right": ... }` objects. Reports include initial state, capacities, per-pulse transfers/levels/residuals, and the final checkpoint. Reuse `finalCheckpoint` as a later request's `checkpoint` to continue. Input is bounded to 32 KiB and 1,024 pulses; unknown fields fail. The complete request must succeed before any JSON is printed. This example is not a desktop control, world recipe, or full-world save format.

## Validation

Tests cover hand-computed weighted fills, receiver deficits, reverse spill, unequal initial stocks, simultaneous inputs, exact thresholds, oversized pulses crossing multiple stages, connection-area accounting, side-label symmetry, pulse partitioning, atomic errors, checkpoint continuation before/at/after connection, malformed states and geometry, and direct independent column-volume reconstruction. Thirty seeded sequences check 100 pulses and area/datum scaling; another 60 weighted geometries check 100 pulses each, with plateaus and unequal starting stocks. A five-region chain independently matches C2a's capacities and root storage without modifying the analysis. These checks do not establish a general nested or multiway routing algorithm.

Validation completed September 24; record finalized September 25, 2026:

- `npm test` passed: 61 Rust tests (10 new pair integration tests and 3 new example tests) and 43 TypeScript tests, including type checking and release native compilation.
- Rust formatting and Clippy with warnings denied passed for all targets.
- The release CLI produced matching file/stdin reports with phases `separate → separate → atSill → merged`. The scenario retained all 38 m³ at 4 m; restoring its checkpoint and adding 22 m³ produced 60 m³ at 6 m. Missing arguments/files, malformed requests, and oversized input failed without a partial stdout report.
- Production build and the development desktop suite passed. Default headless/desktop fingerprint remains `2e66ac09`, with 10,242 regions and 5,792,952 model-array bytes. Vite's existing renderer-chunk-size advisory remains.
- No new packaging, visual feature, or performance benchmark is claimed. The previous packaged C2b application is unchanged; this experiment runs through the developer CLI.

## Next step

[C3c](spill-connections.md) now provides geometric sill connections, all lower contacts, and potential passages without treating arbitrary siblings as adjacent. Next define active receiver descent and allocation at simultaneous outlets, then test conservative nested filling. Independent planetary inventory assignment, withdrawals/splitting, finite flow rates, and desktop integration remain separate increments; existing world water must never be replaced by repeated global coverage fitting.
