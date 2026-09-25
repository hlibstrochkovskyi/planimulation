# Controlled nested filling: milestone C3d

Implemented scope: a standalone closed, initially dry experiment on a validated graph of at most 128 regional columns. Version `nested-reservoir-1`. It combines C2a storage topology, C3c spill passages, and C1 dry-bed descent with exclusive active inventories. It is **not** a general planetary solver: desktop/world water, `basins-1` recipes, protocol 7, and the existing analysis versions remain unchanged.

## Supported geometry and allocation boundary

Each column specifies bed height in meters and positive physical area in m². Each undirected edge specifies two region indices and positive distance in meters. Preparation rejects non-finite fields, self/duplicate/out-of-range edges, disconnected graphs, unresolved numeric intervals, and the size limit. This is a physical column graph, not a triangulated display mesh or a spherical-world checkpoint.

Every merge must have exactly two children. Each child must have exactly one direct candidate receiver through exactly one connecting plateau, with exactly one lower contact on each side of that plateau. Multiple sill cells along a connected plateau are allowed. One-child dead-end plateaus do not become outlets. Multiway merges, alternative connecting plateaus, and ambiguous lower contacts are rejected **before accepting any input**, rather than assigned an implicit equal split or arbitrary sibling preference. C3c still supports inspecting that broader geometry; C3d deliberately supports a smaller subset.

Unique sill contacts determine the receiving entry region. Dry-bed descent uses C1 with every region initially dry: steepest positive bed slope, stable region-ID ties, and deterministic flat routing over sorted adjacency. The resulting terminal must be a leaf inside the receiving subtree. This is an explicit single-receiver approximation, not a measured hydraulic flow distribution. A dry ridge with tied slopes can route differently after region relabeling; physical symmetry is tested only where routing is unique.

Routes report the actual adjacent sill passage, the dry-bed descent path, and its terminal leaf. During filling, an already active ancestor of that leaf receives the input as a single lake. The full static descent path is therefore a routing guide, not a claim that water continues flowing down the bed beneath an existing lake.

## Inventory and filling rules

The authoritative state is a sorted frontier of active branches with volumes in m³. Initially all leaves are active with zero stock; dry surfaces have no reported water level. Every leaf is covered by exactly one active branch. An active parent replaces its children; parent and child stocks must never coexist.

Each pulse has one entry region and a nonnegative volume. Pulses are **ordered**, not simultaneous forcings:

1. Descend toward the entry's dry-bed leaf until reaching its active reservoir.
2. Fill that reservoir to its next sill, retaining its threshold stock. A closed root accepts all representable additional volume.
3. Route only the excess through the source's unique C3c passage into its sibling. If that sibling contains separate children, descend through its actual entry leaf, filling nested deficits before allowing onward spill.
4. When both immediate children reach their capacities, replace them by the parent's birth stock, exactly the sum of those capacities. Only then may surplus raise the parent's common level.
5. Repeat up the bounded hierarchy. The root has no external collector or loss.

The shared curve includes all descendant and exclusive parent columns, including sills and upper banks as they become submerged. An exact merge threshold has zero water depth over the sill; this is a limiting connection state, not positive-depth discharge. Adding water to an already merged reservoir does not recreate old child inventories.

The experiment is quasi-static. Pulse index is not time; there is no discharge law, travel delay, rain field, evaporation, withdrawal, drying/splitting, or erosion. Initially unequal stocks can be obtained through earlier pulses, not arbitrary unvalidated initial lakes. Simultaneous-input scheduling and multiway allocation remain separate design questions.

## Budgets, precision, and checkpoints

```text
cumulative prescribed input = sum of exclusive active stocks
new total − previous total = pulse input
```

Each pulse records accepted transfers between immediate sibling subtrees. A parcel can appear in a transfer at more than one nested level: these records must **not** be summed as independent external input. Transfers are emitted after their nested receiver updates; report order is not physical event time. Geometry and entry details are available in the route table. Common storage growth is not labeled as a directional edge flux.

Reuse C3a's indexed prism curves as read-only geometry helpers, not as extra water ledgers. Budget tolerances are `max(1e-9 m³, scale × 1e-12)`; output-level reconstruction permits `max(1e-9 m³, total stock × 1e-10)`. Threshold states use their exact threshold levels. Unresolvable positive additions, levels rounded onto the wrong threshold, numeric overflow, and counter overflow return errors. Residuals never replace the stock, and global ocean coverage is never refitted.

Updates operate on candidate inventories, audit the frontier, levels and both budgets, then commit. Failure leaves the prior checkpoint unchanged. Zero-volume pulses preserve stocks and increment the pulse counter.

Checkpoints contain version, ordered column/edge geometry, active stocks, cumulative input and pulse count. Preparation rebuilds topology, routes and storage indexes. Restore rejects overlap, missing leaves, premature parents, over-capacity stocks, unsorted IDs, unmerged full siblings, invalid budgets and incompatible versions. It checks consistency, not historical authenticity. Scoped raw-decimal readers preserve serialized finite numbers in the supported environment; world-recipe parsing is untouched. The CLI uses externally tagged start variants to avoid intermediate numeric conversion during deserialization.

## Bounded implementation

This is a correctness laboratory, not a scalable full-world representation. With `N` columns and `K` hierarchy nodes, it retains subtree curves and a membership matrix: `O(NK + K² + E)` storage, with up to `O(NK log N + K² + EK)` preparation including passage queries. A conservative pulse bound is `O(K² + K log N)` including the frontier audit. No dense grid-size promise or measured performance improvement is made.

Filling recursively descends the validated binary hierarchy; a newly merged branch gets one final active-storage call. The hard 128-region limit bounds stack depth and work. There is no unbounded settling loop. C2a/C3c's existing large-world iterative analyses are unchanged; their scale tests do not imply that C3d supports those world sizes.

## Developer scenario

```sh
cargo run --release --locked --manifest-path native/Cargo.toml --example nested_reservoir -- docs/scenarios/nested-reservoir.json
```

The fixture is a closed chain of unit-area columns:

```text
region:     0     1     2     3     4
bed (m):    0 —   2 —   1 —   5 —  −1
            A   sill   B   sill    C
```

A and B merge at 2 m. Their combined reservoir meets C at 5 m. A/B inner capacities are 2/1 m³; AB's outer capacity is 12 m³; C's is 6 m³. Input enters only C:

| Pulse input | Total stock | Result |
| ---: | ---: | --- |
| 6 m³ | 6 m³ | C reaches its outer sill; A/B stay dry |
| 1 m³ | 7 m³ | Spill path `[4, 3, 2]` fills B to 2 m; A stays dry |
| 2 m³ | 9 m³ | B spills into A; A/B meet at their inner sill |
| 9 m³ | 18 m³ | AB rises to 5 m; all three meet at the outer sill |
| 5 m³ | 23 m³ | One common lake at 6 m, including both sill columns |

Requests use `{"start":{"dry":GEOMETRY},"inputs":[{"region":4,"volumeCubicMeters":6}]}`. To continue, use `{"start":{"checkpoint":CHECKPOINT},"inputs":[...]}` with the previous report's `finalCheckpoint`. Use `-` for stdin. Requests are limited to 32 KiB and 1,024 pulses, and unknown fields are rejected. Nothing is printed until the entire request succeeds. The report includes analysis, routes, initial/final checkpoints, and per-pulse transfers and snapshots. This is not a desktop command or a full-world save.

## Validation

Hand-computed tests cover near-before-far entry, reverse spill, independently supplied stocks, exact thresholds, large pulses crossing several merges, sill storage, all-region entry after complete filling, and flat/closed terrain. Forty seeded weighted geometries each receive 100 pulses and are compared against an independent three-bowl piecewise oracle using direct column sums and bisection. Another 500 mixed-entry pulses check stock accounting, area/datum scaling, relabeling where routes are unique, and repeated JSON restoration. Independent ancestor walks reconstruct regional depths without using the implementation's membership matrix.

Checks also cover a 127-region nested hierarchy, a 128-region flat, rejection above the size bound, ambiguous sills and multiway contacts, malformed graphs/frontiers, premature/duplicate parent stocks, invalid ledgers, numerical failures, and atomic counter overflow. CLI tests cover the fixture, continuation, malformed/oversized requests, and failure after an earlier valid pulse.

Validation completed September 25, 2026:

- `npm test` passed: type checking, release native compilation, 84 Rust tests across all targets, and 43 TypeScript tests. The final frontier-corruption assertions also passed in a targeted rerun.
- Rust formatting and Clippy across all targets with warnings denied passed; `git diff --check` passed.
- Release CLI file/stdin reports matched byte-for-byte. The fixture ended with 23 m³ at 6 m; checkpoint continuation with another 5 m³ produced 28 m³ at 7 m, exactly matching uninterrupted execution. Eight invalid argument/request cases failed without stdout, including an invalid pulse after valid ones.
- Production build and `npm run test:desktop` passed. Headless and desktop retained fingerprint `2e66ac09`; the default world still has 10,242 regions and 5,792,952 model-array bytes.
- The existing Vite renderer-chunk-size advisory remains. No new packaging run, visual feature, scientific calibration, or full-network performance benchmark is claimed.

## Next increment

Specify and test a separate allocation policy for simultaneous multiway/alternative outlets, with explicit comparison cases and pulse-order sensitivity checks. Do not relax C3d's rejection rules without a model-version decision. Full-world inventory initialization, a scalable active-state representation, finite rates, withdrawals/splitting, and desktop integration remain separate increments.
