# Geometric spill connections: milestone C3c

Implemented scope: a standalone native analysis of immediate basin children, connected sill plateaus, all lower contacts, and queryable potential passages. Analysis version `spill-connections-1`. This supplies missing geometry for future nested routing; it does **not** allocate water or evolve reservoirs. Desktop/world recipes remain `basins-1`, protocol 7; C2a's `basin-analysis-1` and the C3a/C3b experiments are unchanged.

## Why the merge tree is insufficient

A C2a parent says that its children become connected at a shared threshold. It does not imply that every pair of children directly shares a sill. The existing single contact edge per child witnesses the threshold, but may not lead to another child through that particular plateau.

For example, the chain of heights `[0, 4, 1, 4, 2]` has three children merging at 4 m:

```text
left bowl — sill A — middle bowl — sill B — right bowl
```

Left can spill directly into middle. Reaching right requires crossing middle, whose current inventory cannot be ignored. Treating the three children as an all-to-all connection would invent a passage past an unfilled reservoir.

Another controlled case has a one-child, dead-end plateau and a separate connecting plateau at the same height. C2a's lexicographically first contact can legitimately point to the dead end: it was never a complete spill route. C3c retains both plateaus and finds the actual connecting one without redefining C2a's witness.

## Representation and construction

`SpillConnections::build(surface, heights)` constructs and owns a fresh C2a hierarchy from the exact supplied bed, so callers cannot accidentally pair a hierarchy with different heights. As with C2a, the library requires an already validated connected reciprocal graph; it is not a parser for arbitrary untrusted adjacency arrays.

For each parent merge, retain a **child–plateau incidence graph**, rather than materializing every possible pair of child branches:

- A sill plateau is a connected component of exactly equal-height regions at that parent's birth threshold. A plateau's regions have that parent as their exclusive C2a owner.
- Each contact stores the immediate child branch and an actual adjacent `[sill_region, lower_region]` pair. The lower region may belong to a deeper descendant inside that child's subtree.
- Every lower contact edge is retained, including multiple contacts to one child, multiple separate plateaus between the same children, and plateaus touching only one child.
- Higher regions owned by the parent are not automatically spill passages at its birth height. Minimum plateaus with no merge are not sill connections. The global root has no external receiving branch.

An iterative Euler traversal assigns subtree intervals. Binary search over each parent's ordered child intervals identifies the immediate child containing a lower endpoint, without repeatedly climbing a deep hierarchy. Equal-height components are traversed iteratively. Plateau IDs follow ascending smallest region ID; regions, contacts, candidate branches, and alternative plateau IDs are sorted. Canonical neighbor order makes results independent of input adjacency iteration order. No random stream, bed modification, epsilon height, or water-level fitting is involved.

As a cross-check, the minimum of all new contacts for every non-root child must equal that child's existing C2a witness. That check does not replace independent reachability tests.

## Queries and interpretation

`receivers(source)` returns all other immediate children sharing at least one connected sill plateau with the source, grouped with their alternative plateau IDs. It never traverses a different lower child. Empty results for the root mean no external outlet; an invalid branch is an error. Several candidates indicate an unresolved allocation choice, not automatic equal splitting or a random receiver selection.

`passage(source, receiver, plateau)` requires both distinct branches to contact the chosen plateau. It chooses each branch's lexicographically first contact on **that** plateau, then finds a shortest graph-hop path between the two sill endpoints using breadth-first search and ascending neighbor IDs. The returned sequence is:

```text
lower source contact → sill regions only → lower receiving contact
```

This is shortest only between those canonical endpoints, not the globally shortest route over all contact pairs or a physical least-resistance flow path. Region areas, hydraulic conductance, and travel time do not choose it. A passage at the threshold is limiting geometry: sill cells have zero depth there. Actual source water must reach the threshold before it can spill.

For nested receivers, retain the actual entry region instead of choosing an arbitrary leaf. In the chain `[0, 2, 1, 5, −1]`, outer overflow from the right enters the nearer child of the nested left reservoir via region sequence `[4, 3, 2]`; it does not teleport into region 0. Resolving the subsequent descent and active receiving reservoir remains future work.

Potential passages do not establish that source water exists, identify a current lake, assign discharge, fill a receiver, or authorize transit through a lower child. A dynamic solver must combine this incidence graph with active inventories and distinguish filled transit reservoirs from still-unfilled recipients. C3c is not a general fill–spill–merge implementation.

## Complexity and reproducibility

In addition to the existing C2a construction, the analysis uses iterative graph traversal, sorted adjacency/contact lists, and logarithmic immediate-child lookup. A conservative build bound is `O((N + E) log(N + K))`, with `O(N + E + K)` retained storage for regions, edges, and hierarchy nodes. Each region belongs to at most one stored sill plateau; lower contact storage is bounded by the input adjacency size. A multiway merge does not create a quadratic all-pairs table.

Candidate lookup scans incident plateau contacts and groups results. A passage query allocates `O(N)` predecessor scratch space and scans the chosen plateau's adjacency plus endpoint contacts. These are inspection APIs, not per-tick routing-performance promises. Copied sorted adjacency and lookup indexes are private and omitted from JSON; the recipe regenerates them. The report is static analysis, not a dynamic checkpoint.

## Developer report

```sh
cargo run --release --locked --manifest-path native/Cargo.toml --example spill_connections -- docs/scenarios/spill-connections.json
```

The checked-in recipe uses `first-light` at subdivision 2. Any current desktop-exported recipe can be supplied instead. Output includes the resolved recipe, both analysis versions, C2a hierarchy/ownership, and the full plateau/contact graph. To query a passage, append three IDs taken from the report:

```sh
cargo run --release --locked --manifest-path native/Cargo.toml --example spill_connections -- path/to/recipe.json SOURCE_BRANCH RECEIVER_BRANCH PLATEAU_ID
```

The optional query adds candidate receivers and the selected passage; the graph itself stays unchanged. IDs are analysis-local, not persistent river or lake IDs. Use `-` for stdin. Recipe input is limited to 32 KiB; malformed/legacy recipes, invalid arguments, and invalid passages fail before any JSON is printed. The CLI constructs a separate analysis alongside the generated world and does not modify its water, drainage, or wire arrays. This is not a desktop layer or a new protocol command.

## Validation

Controlled tests cover a three-way chain that cannot skip its middle child, a shared multiway plateau with several direct receivers, nested entry regions, alternative sills, dead-end witnesses, multi-region plateau paths, equal-hop ties, multiple contacts, flat/closed worlds, and invalid queries. An independent raw-height flood checks direct receiving branches and every returned passage's adjacency, endpoint ancestry, and sill-only interior over 72 continuous/quantized spherical cases; reversed neighbor order reproduces the complete analysis.

Stress checks include a 12,001-node deep hierarchy, a 12,001-region long plateau path, and a 2,000-child shared plateau without a quadratic connection table. Generated worlds at subdivisions 0, 2, 5, and 6 check agreement with their existing basin analysis, unchanged wire state, sampled passages, and independence from initial water coverage and diagnostic playback. The largest generated case has 40,962 regions. These are correctness checks, not scientific calibration or a performance benchmark.

Validation completed September 25, 2026:

- `npm test`: type checking, release native build, 72 Rust tests across all targets, and 43 TypeScript tests passed.
- Rust formatting and Clippy across all targets with warnings denied passed; `git diff --check` passed.
- Release CLI file/stdin reports were byte-identical. The checked-in recipe produced 20 plateaus and 48 contacts; a selected passage was `[50, 13, 46]`. Eight invalid-input/argument cases failed without partial stdout.
- `npm run test:desktop` passed with fingerprint `2e66ac09`. A separate headless default-world check confirmed the same fingerprint, 10,242 regions, and 5,792,952 array bytes.
- The production build retained its existing renderer chunk-size advisory. No new packaging run, visual feature, or performance benchmark is claimed for this standalone analysis.

## Next increment

Use the explicit entry regions and child–plateau graph to define a small nested filling experiment. Specify active receiver descent and the policy for ambiguous simultaneous outlets before allocating any water. Keep combined-stock accounting, source saturation, no transit through unfilled children, and checkpoint continuation as acceptance conditions. Planetary inventory assignment, flow rates, drying/splitting, and desktop integration remain separate work.
