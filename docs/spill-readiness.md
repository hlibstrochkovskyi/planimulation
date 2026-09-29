# Generated-world integration checkpoint: C3h

Status: implemented read-only structural screening, `spill-readiness-1`, September 28, 2026. This increment measures whether generated bed geometry satisfies **two necessary restrictions** of the bounded C3f/C3g laboratory. It does not move water, import initial inventories, relax either solver, or certify a world as dynamically supported.

## Position in the product plan

The project still follows the agreed natural-world-first sequence. The desktop has reproducible generation, geological layers, relief, water surfaces, drainage and basin inspection. C3a–C3g validate storage and routing in separate headless experiments. Milestone C is not complete: there is no advancing planetary water state, computed river discharge, climate or ecology. Milestone B also retains export/prehistory work. No calendar delivery baseline exists, so these records do not claim that development is on schedule or assign a percentage complete.

The recent work exposed genuine model errors and restrictions, but laboratory completeness is not product integration. Further extensions need evidence from generated geography. This checkpoint therefore changes the immediate priority from building a pending-input wrapper to measuring and resolving actual receiving-entry restrictions. Product principles and the broader sequence are unchanged.

## Implemented screen

`Screening::build(surface, heights)` requires the same validated reciprocal connected graph as C2a/C3c. It rebuilds sill connections from the supplied bed and canonical dry drainage with zero water-body labels. Initial oceans do not terminate this dry descent, matching C3f/C3g preparation. It neither uses nor changes the world's current wet-drainage field.

Checks:

- Is the whole graph within the existing 128-region laboratory bound?
- Do all contacts from connecting sill plateaus into each immediate child descend to one internal terminal leaf?

For the second check, one-child dead-end plateaus are excluded, as in the solver. Every relevant lower contact follows dry descent. Iterative Euler intervals verify that its terminal is a leaf inside the receiving subtree. For each conflicting branch the report retains the first canonical entry and the first entry reaching a different leaf, including plateau IDs, actual adjacent `[sill, lower region]` edges, terminal regions and parent ID. These are witnesses of the current restriction, not a claim that such geography is physically invalid. Multiple entry paths may be entirely natural.

The report also gives branch/leaf counts, maximum hierarchy depth (root depth zero), maximum immediate-child count, connecting/dead-end plateau counts and checked contact count. It estimates two **logical element counts** of the current laboratory representation:

```text
membership matrix entries = branch_count²
curve column references = sum(subtree_region_count for every branch)
```

These are not measured RAM bytes or runtime estimates. The screen calculates them using subtree counts without allocating either the matrix or per-branch reservoir curves. In addition to C3c construction and drainage, screening uses iterative traversal and contact scans with `O(N + E + K)` additional storage. The report stores at most one witness pair per branch, not all path combinations. No recursion proportional to hierarchy depth is added.

`notChecked` explicitly lists initial-water mapping, weight policy, stock/event numerics, pending-input timing/accounting, dynamic performance and drying/splitting. Passing both flags establishes only that these two structural gates did not reject the world. It does not prove the solver will construct or advance successfully.

## Reproducible generated-world results

Use [the existing recipe](scenarios/spill-connections.json), changing only the seed and subdivision as recorded in [all 26 results](data/spill-readiness.csv). Analysis/model versions are `spill-readiness-1` / `basins-1`. The experiment generated all listed worlds and retained every result; there was no failed-seed filtering.

| first-light subdivision | Regions | Branches | Maximum depth | Conflicting branches | Curve column references |
| ---: | ---: | ---: | ---: | ---: | ---: |
| 0 | 12 | 1 | 0 | 0 | 12 |
| 1 | 42 | 9 | 3 | 0 | 72 |
| 2 | 162 | 42 | 11 | 4 | 879 |
| 3 | 642 | 112 | 21 | 11 | 7,380 |
| 5 | 10,242 | 451 | 66 | 21 | 411,318 |
| 6 | 40,962 | 589 | 97 | 7 | 2,589,588 |

At subdivision 3, all twenty seeds `readiness-00` through `readiness-19` had at least one conflicting receiving branch (range 3–12). All were also above the laboratory size bound. This is a fixed engineering smoke sample, not an estimate for all possible planets. Conflict counts are not monotonic with resolution; changing resolution changes the generated physical geometry and its topology.

For default `first-light` at subdivision 5, one witness is child branch 16 under parent 87, plateau 155:

```text
sill 7321 → receiving region 1861 → dry terminal 7669 (leaf 11)
sill 7321 → receiving region 7322 → dry terminal 1872 (leaf 2)
```

Both enter the same outer child but different inner reservoirs. The arrows after the receiving region summarize dry descent, not direct adjacency. A canonical single entry would discard a real alternative; counting every mesh edge as a separate equal-weight receiver would introduce resolution-dependent weighting. Neither is silently adopted here.

## Decision on sub-precision inputs

No pending-water implementation is added in C3h. C3g retains atomic rejection. The following requirements are fixed for any future accumulator:

1. Pending volume is an explicit owned stock, associated with its source region and forcing interval/profile; it is not already lake storage or a change to visible water level.
2. Accepted external input must balance resolved storage plus pending storage and any declared losses. Acceptance, release and failure must be distinguishable; catching every solver error and calling it pending input is invalid.
3. Saving must preserve pending amounts, ownership, timing, counters and any numerical compensation. A single rounded global scalar cannot establish retention of increments below its precision.
4. Delayed release or mixing different intervals changes forcing history. It requires a named approximation and a separate version; it is not an exact numerical fix for C3g.
5. Input that is tiny relative to an existing stock and a tiny arithmetic remainder created inside a threshold event are different problems. A source queue alone does not solve both.

Compensated arithmetic and temporally batched pending storage remain candidate approaches, not working features. Before selecting one, retain a concrete failed numerical case and measure its relevance at intended world/forcing scales. A model that rejects small inputs cannot yet claim a robust continuous water cycle.

## Next integration gates

[C3i multiple-entry allocation](multi-entry-network.md) now supplies a separately versioned bounded policy using explicit branch and internal-entry weights. Thirteen conflicting subtrees extracted from four generated worlds were accepted and advanced in that experiment. This screen still describes C3f/C3g restrictions and must not be treated as proof that C3i is ready for planetary dynamics. Preserve the previous experiments as references; neither the size bound nor initial-world inventory mapping is resolved.

After that:

1. [C3j](initial-water-inventory.md) maps existing initial water into read-only exclusive active inventories without refitting coverage or double-counting nested storage, and validates regional reconstruction and total volume. [C3k](seeded-network.md) connects it to a separately versioned bounded dynamic checkpoint; full generated-world integration remains open.
2. Replace or bound the duplicated storage representation and measure headless forcing on generated worlds. Resolve relevant numerical failures explicitly; passing a structural screen is not enough.
3. Integrate a deliberately limited desktop water demonstration: prescribed input, run/pause, visible lake-level change on flat map/globe, budget and event inspection, and reproducible replay. Climate-derived rain, erosion and the full water cycle remain later work.

The next visible acceptance target is that water demonstration, not a growing count of isolated experiments. Before widening scope, require one documented generated recipe to preserve initial inventory and advance prescribed inputs headlessly; unsupported cases must remain explicit.

## Developer command

```sh
cargo run --release --locked --manifest-path native/Cargo.toml --example spill_readiness -- docs/scenarios/spill-connections.json
```

Supply any current exported recipe or `-` for stdin. For example, with `jq` available:

```sh
jq '.seed = "readiness-00" | .subdivision = 3' docs/scenarios/spill-connections.json | native/target/release/examples/spill_readiness -
```

The CLI accepts one recipe, limited to 32 KiB. Malformed/legacy recipes and analysis errors fail before stdout. Output includes the recipe, scope and screening report. Structural restriction failures are reported data, not CLI errors. No new desktop operation, wire field or model version is introduced.

## Validation

Tests check real conflicting paths, dead-end exclusion, flat roots, logical storage counts, a 2,001-node deep hierarchy, invalid fields, and canonical adjacency. Sixty small quantized surfaces compare the screen against actual C3g construction and independent receiver/ancestor walks. Generated worlds at subdivisions 0, 2, 5 and 6 preserve their complete wire arrays; dry/wet initialization and diagnostic advancement do not change the screen.

Validation on September 28, 2026:

- `npm test` passed: type checking, release native build, 131 Rust tests across all targets and 43 TypeScript tests.
- Rust formatting, Clippy across all targets with warnings denied and `git diff --check` passed.
- Release CLI file and stdin reports were byte-identical. All 26 CSV rows were regenerated and checked field-for-field. Eight malformed argument/recipe cases failed without stdout.
- `npm run test:desktop` passed. Desktop and separate headless checks retained fingerprint `2e66ac09`; the default world still has 10,242 regions and 5,792,952 array bytes.

The existing Vite chunk-size advisory remains. No new packaging run, visual feature, full solver execution on generated worlds or dynamic performance claim is included.
