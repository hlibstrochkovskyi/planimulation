# Explicit multiple receiving entries: milestone C3i

Implemented scope: a separately versioned, closed, initially dry network of at most 128 regional columns. Experiment `multi-entry-network-1`; policy `branch-then-entry-weights-1`. This extends the [C3g constant-forcing experiment](simultaneous-network.md) to allow one receiving child branch to be entered through several internal leaves. C3f and C3g retain their own restrictions and checkpoint versions. The generated world, desktop water, and wire protocol are unchanged.

## The rule

C3h found real sill contacts entering different internal leaves of the same outer basin. One branch-level weight cannot choose which leaf receives overflow. The new setup therefore requires two explicit sets of positive, finite, dimensionless weights:

1. One weight for every non-root branch, in ascending branch ID order, as in C3g.
2. One weight for every distinct `(receiving branch, dry terminal leaf)` pair reached from a **connecting** sill plateau, in ascending pair order. A one-child dead-end plateau is not an inlet. `entry_targets(geometry)` reports the required pairs without supplying default weights.

For each saturated source and fixed state, traverse actual sill plateaus and only already saturated siblings. Stop at underfilled receiving branches. Keep every reachable terminal leaf of such a branch, but deduplicate repeated contacts and alternative routes to the same pair. A canonical contact, plateau and saturated-transit path remain in the report as a reachability witness.

The source's rate is first shared among distinct reachable **branches** according to branch weights. Each branch's share is then distributed among only its **currently reachable internal leaves** according to entry weights. Weights are renormalized separately at both levels. Multiple contacts to one leaf do not increase either weight. A leaf on another, still inaccessible plateau receives nothing. Distinct full sources route simultaneously under C3g's global saturation-event clock, and their rates combine before stock changes.

For source input rate `Q`, branch `b` and entry leaf `l`:

```text
Q_b   = Q × branch_weight_b / sum(reachable_branch_weights)
Q_b,l = Q_b × entry_weight_b,l / sum(reachable_entry_weights_of_b)
```

The implementation uses scaled weights and checked arithmetic remainders. An unrepresentable positive share rejects the whole interval. Entry weights prescribe an experimental preference; they are not inferred from mesh-contact count, sill width or hydraulic conductance. Selecting physically meaningful preferences is a future modeling and calibration decision. The report does not claim a measured source-to-destination discharge.

Within a receiving child, each share follows its own dry terminal to the current active inner reservoir. If that inner leaf reaches its threshold, C3g's event loop merges it with full siblings or routes its ongoing rate through the newly available inner frontier. The outer child becomes transit only when its entire subtree reaches the outer threshold. This avoids equalizing different internal lakes prematurely.

## Controlled example

The [scenario](scenarios/multi-entry-network.json) has A at region 0, inner B leaves at regions 2 and 4, D at region 6, and a connected outer sill plateau at 4 m. A's outer overflow can reach both B leaves and D. B's inner merger is at 2 m. Branch weights are equal; B's far/near entry weights are 3:1.

After A first stores 4 m³, another 1 m³ at A sends 0.5 m³ to B and 0.5 m³ to D. B's share splits 0.375 m³ to its far leaf and 0.125 m³ to its near leaf. A repeated contact to the near leaf leaves those values unchanged. Further inputs eventually fill the inner B reservoirs, merge their stocks, fill the outer branches and form one common lake. The fixture ends with 20.5 m³ at 5 m; adding 7 m³ after restoring its checkpoint ends with 27.5 m³ at 6 m, exactly like uninterrupted execution.

On a chain with separate outer sills, A initially reaches only B's near leaf, while D reaches only B's far leaf. One source never teleports to the inaccessible entry merely because that entry belongs to the same outer branch. Distinct sources can supply the two leaves in the same interval. Existing one-entry cases reproduce C3g's interval reports exactly in the tested fixture.

## State, limits and persistence

The model reuses C3g's constant concurrent inputs, global saturation events, exclusive active stocks, volume budgets and atomic interval commit. Its checkpoint contains the new experiment/policy versions, ordered geometry, both weight lists, active inventory, cumulative input and completed-interval count. Derived geometry and target catalogs are rebuilt and checked. A C3g checkpoint cannot silently become a C3i checkpoint. There is no pending tiny-input stock or fractional-interval state.

The model is still bounded by 128 regions and retains C3f's duplicated subtree curves and membership matrix. Entry-target enumeration does not remove the `O(NK + K² + E)` laboratory representation. It does not import initial planetary water, handle withdrawal or splitting, infer realistic flow rates, or advance desktop water. Positive inputs below stock precision still fail atomically. The C3h structural screen therefore continues to describe what C3f/C3g reject; it is not reinterpreted as a pass/fail screen for this new policy.

## Developer scenario

```sh
cargo run --release --locked --manifest-path native/Cargo.toml --example multi_entry_network -- docs/scenarios/multi-entry-network.json
```

Requests use `{"start":{"dry":SETUP},"intervals":[[INPUT,...],...]}` or `{"start":{"checkpoint":CHECKPOINT},"intervals":[...]}`. Input regions are unique within each interval, and their order has no temporal priority. Use `-` for stdin. The CLI bounds requests to 32 KiB and 1,024 intervals; it prints no partial report if a late interval fails.

## Validation

Tests cover branch-before-entry weights, same-leaf contact deduplication, separate-sill gating, two simultaneous sources entering different inner leaves, inner and outer saturation, checkpoint continuation, unique-entry agreement with C3g, area and weight scaling, geographic relabeling, a 127-region chain, cycles and atomic validation/precision failure. One hundred seeded weighted cases match independent two-stage share equations.

Four generated subdivision-2 worlds supplied thirteen bounded conflicting subtrees. Each was rejected by C3g, accepted by C3i with explicit unit test weights, and advanced under small distributed input while the original generated world arrays remained unchanged. Cutting out a subtree removes its external contacts: this verifies bounded geometry and state behavior, not a full planetary run or a calibrated policy for that world.

Validation on September 29, 2026:

- `npm test` passed: type checking, release native build, 143 Rust tests across all targets and 43 TypeScript tests.
- `cargo fmt --check`, Clippy across all targets with warnings denied, and `git diff --check` passed.
- Release CLI file/stdin reports matched byte-for-byte. The fixture retained 20.5 m³ at 5 m; checkpoint continuation with 7 m³ retained 27.5 m³ at 6 m and exactly matched uninterrupted execution.
- Twelve malformed argument/request cases failed without stdout, including a late invalid interval, incompatible version, duplicate input region and the 1,025-interval bound below the byte limit.
- `npm run test:desktop` passed. Desktop and separate headless checks retained fingerprint `2e66ac09`, 10,242 default regions and 5,792,952 model-array bytes.

The existing Vite chunk-size advisory remains. No packaging run, visual feature, planetary inventory integration or full-world performance claim is included.

## Next gate

[C3j](initial-water-inventory.md) maps the existing initial water to read-only exclusive active inventories without refitting the global water level and compares reconstructed depth, connected water bodies and total volume with the generated world. [C3k](seeded-network.md) supplies a separate seeded bounded checkpoint and headless continuation; it does not change this experiment's dry-start checkpoint. Next replace or bound duplicated storage and measure prescribed input on full generated-world scales. Only after these gates should the model drive a limited run/pause and water-level display in the desktop app.
