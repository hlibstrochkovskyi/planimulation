# Event-driven receiving-frontier network: milestone C3f

Implemented scope: a standalone, initially dry, closed graph of at most 128 regional columns, combining nested reservoirs, multiway merges, and distinct or alternative sill plateaus. Experiment version `spill-network-1`; explicit policy version `frontier-weighted-events-1`. This is **ordered quasi-static volume routing**, not simultaneous rainfall or timed hydraulics. C3d/C3e remain unchanged reference experiments. World model `basins-1`, protocol 7, initial water, and desktop behavior are unchanged.

## Geometry and supported entry conditions

Use the existing bounded column/edge validator, C2a basin hierarchy, C3c child–plateau incidence, and C1 dry-bed receivers over sorted adjacency. Region beds, areas and distances are physical inputs, independent of display detail. No bed height, initial-world inventory or RNG stream is modified.

Unlike C3d, a parent may have more than two children and multiple connecting plateaus. Unlike C3e, the children need not all touch one common plateau. Repeated contacts, alternative sills and cycles do not multiply a receiver's weight.

Every contact into a particular child branch, across its multi-child spill plateaus, must descend to the **same terminal leaf** within that branch. The actual entry region remains in the route witness; C1 supplies the terminal used to find the active receiving reservoir. Contacts entering different internal leaves are rejected during preparation: choosing among those entry paths would require an additional allocation policy. One-child dead-end plateaus are not outlets and do not constrain receiving entry. This condition is intentionally conservative even when some ambiguous child might later become a common lake.

External pulses enter one explicit region and follow its dry-bed terminal toward the current active ancestor. Stored water has one authoritative frontier: either an active branch or its descendants, never both. Each branch has a prism-storage curve; a parent's curve includes sill and upper-bank columns. The root is closed and has no invented drain.

## Dynamic accessibility before allocation

For a source at its parent's spill threshold, search the child–plateau graph:

- Traverse connected sill plateaus and child branches **already full to that parent's threshold**.
- Stop at an underfilled child. It is a receiving frontier, not a transit shortcut.
- Deduplicate recipients by branch, regardless of the number of contacts or paths.
- Record a deterministic witness consisting of saturated transit branches, plateau IDs, the receiving region, and its terminal leaf.

Thus `A — sill — B — sill — C` first fills B from A. C becomes reachable only after B reaches the relevant outer threshold. If B itself contains smaller reservoirs, an inner merge does not make it eligible for outer transit.

`receivers(source)` exposes this read-only query on the current state. Invalid, underfilled, retired, or root sources fail. Reported witnesses establish accessibility, not directional discharge measurements. C3c can resolve each plateau transition into an adjacent-region passage. Transit through a full child is justified by its flooded connected subtree; the report does not trace a current through the lake bed.

## Saturation events and prescribed weights

The setup requires one finite positive dimensionless receiver weight per non-root branch in ascending analysis ID order, including internal branches. These are experimental preferences, not inferred conductances or widths. Normalization is local to the currently reachable frontier.

At an inactive parent:

1. Fill the input's child, descending into its actual receiving leaf as needed.
2. If excess remains, find the frontier reachable from the saturated source component.
3. Compute each frontier child's total remaining capacity, subtracting only its exclusive active descendant stocks.
4. Distribute proportionally **only until the first frontier child saturates**, or the available input is consumed.
5. Fill each recipient recursively. Recompute accessibility after a saturation before distributing any remaining input.
6. When all immediate children are full, replace them by one parent stock and let excess raise the parent level or spill at its next threshold.

For excess `E`, deficits `d_i`, and weights `w_i`:

```text
share_i = E × w_i / sum(w)
fraction = min(1, min_i(d_i / share_i))
grant_i = share_i × fraction
```

Limiting recipients receive their exact deficit. At a final unsaturated stage, the last canonical recipient receives the checked arithmetic remainder, as documented in C3e. Bounds, positive progress and budgets are checked before committing.

The distinction from C3e is crucial. Consider equal weights, A's capacity 4 m³, B's 1 m³, C's 2 m³ and D's 3 m³, with edges `A–B`, `B–C`, `A–D` across separate equal-height sills. Input of 7 m³ into A first gives B/D one cubic meter each. B then opens the route to C, so the last cubic meter splits between C and D. Final stocks are `[A=4, B=1, C=0.5, D=1.5]`, not `[4,1,0,2]`. Allocating the entire excess across the original frontier would miss that event.

## Input order is part of the model

The same graph gives an exact order-sensitive regression:

| Ordered inputs | Final A | Final B | Final C | Final D |
| --- | ---: | ---: | ---: | ---: |
| A receives 5 m³, then B receives 1 m³ | 4 | 1 | 0.25 | 0.75 |
| B receives 1 m³, then A receives 5 m³ | 4 | 1 | 0.5 | 0.5 |

Both retain 6 m³. The difference is when B opens access to C, not floating-point noise. C3e's shared-sill order comparisons cannot be generalized to this network. An arbitrary iteration over rainy cells must **not** be labeled simultaneous forcing. This experiment records and replays ordered pulses; a future planetary update needs separately specified simultaneous-input or finite-rate semantics.

Stage records include receiving frontiers, accepted volumes and newly saturated branches. Nested stages are emitted after their receiving updates, before the enclosing stage record; list order is not physical time. Nested transfers must not be summed as independent external inputs. Different paths to the same recipient use a deterministic accessibility witness, not a route-specific flux split.

## Precision, bounds and persistence

Reuse the previous stock contract: cumulative input equals exclusive stored volume, and each pulse's storage change equals its input within `max(1e-9 m³, scale × 1e-12)`. Output-level reconstruction uses `max(1e-9 m³, stock × 1e-10)`. Exact threshold states use exact threshold levels; no epsilon height or coverage refit is applied. Failure leaves the entire prior inventory, ledger and pulse count unchanged.

An analytical threshold expressed through arbitrary floating-point areas and weights can leave a sub-precision positive remainder after a saturation. A seeded regression reaches this at the twentieth target pulse. The remainder cannot be added reliably to the next recipient, so the **whole pulse is rejected**, not discarded or rounded into a fictitious transfer. A later resolvable pulse succeeds. This is a supported failure mode and a limitation for eventual time integration; there is no pending tiny-volume accumulator in this model. Ensemble oracle samples avoid demanding exact floating analytical thresholds, while this boundary is retained as an explicit atomic-failure test.

Every nonfinal allocation event must saturate a new immediate child; each invocation is bounded by that parent's child count. Recursion descends the bounded hierarchy, with one active-parent call after a merge. Cyclic geographic connections use visited branch/plateau sets, not iterative settling. This is not an unbounded-grid implementation.

For `N` regions and `K` hierarchy nodes, subtree curves and a membership matrix retain `O(NK + K² + E)` data. Frontier queries traverse incidence lists rather than materializing all sibling pairs; stored path witnesses can be longer than direct contacts. Pulse cost depends on nested visits, frontier events, deficit sums and witness construction, and is not claimed to be linear or benchmarked at planetary scale.

Checkpoints include experiment/policy versions, ordered geometry, every weight, active stocks, cumulative input and pulse count. Derived curves, graph indexes and descent terminals are rebuilt. Existing exact-decimal readers preserve finite serialized values. Restore rejects overlapping/missing inventories, incompatible versions, noncanonical full siblings and invalid budgets; it checks consistency, not historical authenticity. Inputs and policy must remain unchanged for replay; old C3d/C3e checkpoints are not silently migrated.

## Developer scenario

```sh
cargo run --release --locked --manifest-path native/Cargo.toml --example spill_network -- docs/scenarios/spill-network.json
```

The fixture has three distinct outer sills at 5 m and one inner sill at 2 m:

```text
A (r0) — sill r1 — B-near (r2) — inner sill r3 — B-far (r4)
  |                     |
sill r7              sill r5
  |                     |
D (r8)                C (r6)
```

All areas and weights are one. Outer capacities are A=5, B=11.5, C=3 and D=2 m³. Both outer contacts into B enter its near leaf. Pulses `[5, 1, 1, 2, 9.5, 3, 9]` m³ at A first fill A, then share with B/D, fill B's inner deficits, bring B to its outer threshold, open C, and finally create one common lake at 6 m with 30.5 m³. The inner and all outer sill columns participate in common storage.

Requests use `{"start":{"dry":SETUP},"inputs":[{"region":0,"volumeCubicMeters":5},...]}` or `{"start":{"checkpoint":CHECKPOINT},"inputs":[...]}`. Use `-` for stdin. The 32 KiB request and 1,024-pulse bounds remain; unknown fields fail and stdout is published only after the whole request succeeds. Reports contain the unchanged geometric analysis, ordered stages, snapshots, and initial/final checkpoints. This is not a desktop control or world-state import.

## Validation

Tests cover blocked transit, opening a new frontier mid-pulse, weighted allocation, actual nested entry, inner versus outer saturation, common sill storage, cycles, alternative paths, explicit order dependence, atomic errors, ambiguous entry rejection, and exact JSON continuation. Binary and shared-sill limits are compared with C3d and C3e. A 127-region chain checks long saturated transit; a closed flat checks the root boundary.

Forty weighted/area-scaled forks receive 30 pulses each (1,200 updates) and match an independent two-stage analytical oracle plus direct regional prism sums. Sixty cyclic/branched graphs with partially filled inventories compare every saturated source's receiving frontier against an independent raw-region flood that stops at unfilled bowls. Every reported plateau transition is checked for physical adjacency; queries leave the checkpoint unchanged. CLI tests cover the complete scenario, continuation, malformed requests and late failures.

Validation on 2026-09-28:

- `npm test`: 110 Rust tests and 43 TypeScript tests passed, including type checking and the release core build.
- `cargo fmt --check` and `cargo clippy --all-targets -- -D warnings` passed.
- The release example produced byte-identical reports from file and stdin. The fixture retained 30.5 m³ at 6 m; checkpoint continuation with another 9 m³ matched uninterrupted execution exactly, retaining 39.5 m³ at 7 m.
- Eight release CLI failure cases (arguments, missing file, malformed/oversized input, incompatible policy, invalid weight, and a late invalid pulse) returned errors without partial stdout.
- `npm run test:desktop` passed. A separate headless baseline retained fingerprint `2e66ac09`, 10,242 regions and 5,792,952 array bytes. The existing Vite chunk-size advisory remains.

No packaging run, new visual feature, physical calibration or planetary network performance claim is included in this increment.

## Next increment

Specify simultaneous network forcing before mapping rainfall or initial planetary inventories onto this model. Preserve the exact order-sensitive case as a comparison, and decide how sub-precision residuals would be retained if small time-step inputs are required. A scalable storage representation, ambiguous nested entry allocation, finite discharge, drying/splitting, and desktop visualization remain separate work.
