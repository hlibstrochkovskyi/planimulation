# Basin connectivity and storage analysis: milestone C2a

Historical C2a baseline: native analysis module and standalone JSON report, analysis version `basin-analysis-1`. At that milestone the module was not integrated into desktop generation or the wire protocol; recipes remained `drainage-1`, protocol 6. [C2b basin inspection](basin-inspection.md) now integrates the same algorithm with `basins-1`, protocol 7. The measurements below describe C2a. Water dynamics remain unimplemented; milestone C is not complete.

## Purpose and boundary

Given the core's connected reciprocal region graph, finite bed heights, and positive reference-sphere areas, compute when separate low regions first connect as an analysis level rises. Keep the original bed, initial water, C1 drainage, and random streams unchanged. Include the entire bed, including presently underwater regions: this is geometric potential, not a classification of currently empty lakes.

This separates two questions:

1. At what height can two depressions communicate, and what storage fits below that height?
2. Given actual, possibly unequal inventories and inflows, where does water move?

Only the first is implemented. An overflowing depression does not immediately turn an unfilled neighbor into a common-level lake. The later transport model must fill that neighbor before using their combined storage branch. Existing initial water must eventually be mapped onto independent inventories rather than redistributed globally each step.

## Connectivity tree

The implementation is an elevation-sorted, plateau-batched union-find sweep over regions. It is a sublevel connectivity tree, not a downhill receiver tree.

1. Sort regions by bed height, then region ID. Process all exactly equal heights together; do not add epsilon heights.
2. Before unions at that level, record contacts with strictly lower connected components and their current tree branches.
3. Activate the entire equal-height batch and union edges to neighbors at or below that height.
4. Group new regions by the resulting component, ordered by their smallest new region ID. With no lower child, create a leaf. With one child, extend that branch. With multiple children, create one parent containing all of them.
5. Close child branches at that merge height. Record a deterministic contact edge for each child: lexicographically smallest `[sill_region, lower_neighbor]` among its contacts.

Batching includes disconnected pieces of a sill linked through lower components. Three branches meeting at one height become one three-way merger, not an arbitrary binary ladder with zero-height intermediate basins. Sorted child IDs and contact pairs make results independent of adjacency iteration order.

A recorded edge witnesses threshold adjacency; it is **not** a complete spill path, a flow direction, or proof of available water. Multiple equivalent spill routes may exist. Graph connectivity and region-center elevations determine thresholds, not interpolated display triangles.

Leaves represent connected minimum plateaus. Parents represent potential combined reservoirs. Each non-root branch has a parent at a strictly higher level. IDs follow construction order, so parents have larger IDs than children. They are analysis-local IDs, not persistent lake identities across changes of terrain or resolution.

The root represents the whole closed planet. It has no external drain, spill edge, or finite capacity; these fields are `null`, not an invented outlet or infinite JSON number. A monotone test chain or single bowl has one root branch unless another local minimum exists. Above the highest bed, additional volume covers the entire reference area. No claim is made that such levels describe a habitable planet.

## Storage and ownership

Each region is assigned exclusively to the branch active when it enters the sweep. A branch footprint is its own regions plus all descendant regions. `regionNodes` is therefore **not** C1's catchment labeling: slopes above a merger belong to its parent, not arbitrarily to one of its leaf minima.

Storage uses the same regional-column approximation as B4:

```text
V(branch, level) = Σ area[i] × max(0, level − bed[i])
                  over the branch's entire subtree
```

`volume_at_level` accepts levels from a branch's birth (minimum or merge height) through its spill threshold, inclusive; the root has no finite upper threshold. Below a parent's birth the proposed reservoir is disconnected, so querying that parent is rejected instead of silently returning the storage of unrelated lakes. Above a child's spill height the parent must be considered. At the exact threshold, sill regions have zero depth: the parent value is the limiting storage at connection, not evidence of positive-depth communication across the sill.

`supportAreaSquareMeters` is the whole subtree footprint, not wetted area at an arbitrary lower level. `capacityCubicMeters` is **total** subtree storage at its next merge, including descendants. Capacities across the hierarchy must not be summed: doing so double-counts nested storage. At a parent's birth its total volume equals the sum of its children's capacities. It is not the amount of new rain required to fill the parent.

During the sweep, retain each live branch's area, last elevation, and volume there. Before activating a higher region, add `area × elevation_difference`; new regions enter at zero depth. On merging, add the child volumes and areas. This avoids cancellation from subtracting large absolute-datum elevation moments and does not require rescanning every descendant for every capacity. Non-finite inputs, accumulated overflow, invalid query levels, and disconnected surfaces return errors. The API assumes topology already validated by the core; it is not a parser for arbitrary untrusted adjacency arrays.

The module snapshots heights and areas for read-only queries. A single volume query visits the requested subtree iteratively in `O(subtree regions + nodes)`. This inspection API is not suitable for querying every nested basin every simulation tick; a dynamic solver needs incremental reservoirs or indexed storage curves. Building the hierarchy takes `O((N + E) log N)` time with ordered grouping and sorting, and `O(N + E)` auxiliary storage. There are at most `2N − 1` nodes. Construction, queries, and tree representation avoid recursive traversal, including deeply nested terrain.

## Standalone report

Export a recipe from the desktop, then run:

```sh
cargo run --release --locked --manifest-path native/Cargo.toml --example basins -- path/to/recipe.json
```

Use `-` instead of a path to read JSON from stdin. Input is bounded to 32 KiB; malformed or incompatible recipes fail using the existing strict recipe validation. Output contains a report version, full generation recipe, analysis version, region/leaf counts, root, branch records, and exclusive region ownership. Fields use meters, m², and m³. This report is not a saved dynamic state; it omits copied bed/area arrays, which can be regenerated from the recipe.

The example is a developer/headless entry point, not a new desktop control or a protocol command. It does not reuse initial water-body IDs as basin IDs. No dependency was added and no external implementation was copied.

## Validation

- Weighted nested bowls: known threshold heights, contact edges, capacities, and queries between thresholds.
- Simultaneous multiway sills, including disconnected sill pieces joined through a lower basin; reversed neighbor order gives identical analysis.
- Flat terrain, slopes, a single closed bowl, and signed-zero heights without fictitious drains.
- 20 seeds × 3 spherical resolutions × continuous/quantized heights = 120 combinations. An independent positive-depth BFS at levels between bed heights checks connected components, branch membership, and direct prism sums. Repeated analysis is exact; all branches also have their capacities independently reconstructed.
- 32 generated worlds at levels 0, 2, 4, and 6, including two at 40,962 regions. Eight probe levels per world check connectivity and volume; all non-root capacities are reconstructed. Analysis leaves original serialized world arrays unchanged and is unchanged by diagnostic playback or switching initial water coverage to zero.
- Physical area scaling, large common datum shifts, rejected invalid inputs/query levels, and arithmetic overflow.
- A 40,962-region flat sphere and a 12,001-node deeply nested chain check iterative behavior without stack recursion.

### Recorded results: September 23, 2026

`npm test` passed: 36 Rust tests (8 new basin tests) and 40 TypeScript tests. Type checking, Rust formatting, Clippy with warnings denied, release example compilation, production desktop build, and the development desktop suite passed. The default desktop fingerprint remains `2e752e9b`.

Standalone stdin reports were generated twice at levels 5 and 6 and compared byte-for-byte. Malformed, oversized, legacy-version, and missing-argument inputs returned failure without a partial JSON report. For default `first-light` parameters:

| Resolution | Regions | Minimum branches | Total tree nodes |
| --- | ---: | ---: | ---: |
| 5 | 10,242 | 226 | 451 |
| 6 | 40,962 | 295 | 589 |

These are reproducibility observations, not calibration targets. Minimum branches include underwater depressions and must not be compared directly with C1's dry-sink count. No performance benchmark or new packaged-app validation is claimed for this increment.

The regression run also exposed two existing test issues: stale references after a protocol-test variable rename, and desktop clicks reusing startup canvas dimensions after a window resize (observed width changed from about 645 to 1,181 px). References were corrected; `npm test` now begins with type checking. Desktop center clicks now use the locator's current center, atlas coordinates are recomputed per sample, and explicit 1280×900 / 1600×1000 viewport changes exercise picking across resizes. The corrected desktop suite passed without changing renderer or simulation behavior.

## Next integration

[C2b](basin-inspection.md) supplies versioned native transport and shared flat/globe inspection, keeping spill thresholds distinct from actual water levels. [C3a](reservoir-experiment.md) and [C3b](reservoir-pair.md) add reservoir experiments. [C3c](spill-connections.md) derives the geometric child–plateau graph and potential passages while preserving this analysis and its canonical witnesses. Active receiver descent, general nested/multiway allocation, splitting, and initial planetary reservoir assignment remain future work. Geometry alone is not that solver.

## Research context

[Barnes, Callaghan, and Wickert (2021), Fill–Spill–Merge](https://esurf.copernicus.org/articles/9/105/2021/) distinguishes depression hierarchy from runoff routing and describes filling neighbors before merging reservoirs. It informs that separation of responsibilities. The plateau-batched sublevel sweep here is a separately implemented, tested graph analysis, not an implementation or performance reproduction of their full algorithm. Scientific realism and discharge prediction are not claimed.
