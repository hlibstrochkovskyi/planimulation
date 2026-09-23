# Drainage structure: milestone C1

Implemented model: `drainage-1`, binary protocol 6. This is static bed-based drainage analysis downstream of geology and initial water, not flowing rivers, rainfall, lake filling, erosion, or a depression hierarchy. Milestone C is not complete.

## Contract

Every region has a single receiver, terminal catchment label, equal-height routing rank, and accumulated contributing **dry-land reference area**. Native Rust owns the arrays. No random stream or adjustable drainage coefficient is added. The bed and initial water are read-only inputs; no depression is raised, breached, or filled.

Self-receivers are terminals. A native wet region is always terminal, regardless of underwater bed slope. A dry terminal is a local minimum or the canonical representative of a closed equal-height component. Existing inland water bodies are not automatically connected to the ocean.

This is a bed-connectivity proxy. Descent into a wet neighbor uses its bed height, not its water-surface level; it is not a hydraulic surface-gradient calculation. Travel time, infiltration, runoff efficiency, rain, and flow capacity are absent.

## Downhill receivers

For each dry region `i`, consider strictly lower neighbors `j` and maximize:

```text
gradient(i,j) = (bed[i] - bed[j]) / reference_neighbor_distance(i,j)
```

Exact positive-gradient ties prefer the smaller neighbor ID. The lowest neighbor alone is insufficient: physical distance matters. Distances are spherical center-to-center distances, not paths over display relief.

## Flat routing

Partition dry regions into connected components of exactly equal stored bed height. Equality is exact, not a tolerance that could turn shallow slopes into flats. For each component:

1. Collect regions already assigned a strictly downhill receiver; these are exits, with `flatSteps = 0`.
2. Seed breadth-first traversal with exits sorted by region ID. Unassigned equal-height neighbors receive the current region as receiver and one greater `flatSteps`.
3. If there are no exits, use the component's smallest region ID as a self-receiving closed sink and traverse from it.

The native surface's stable neighbor order resolves equal-hop alternatives by first discovery. These are graph hops, not least-distance paths. Flat routing is a deterministic analysis tie-break, not a measured hydraulic gradient. No epsilon height is added to the bed.

Every nonterminal edge either decreases height or preserves height while decreasing `flatSteps`, so no route can cycle. A completely dry, uniform planet has one canonical sink, not an invented ocean outlet.

## Area accumulation and terminal labels

Initialize dry regions with their own reference-sphere area and wet regions with zero. In topological order, add each region's accumulated area to its receiver; terminals retain their totals:

```text
sum(contributing_area[terminal regions]) = total dry-land area
```

Summing over all regions instead would count upstream land repeatedly. Direct rainfall over water is excluded; no precipitation is modeled at all. Units are m², not m³ or m³/s.

Dry regions reaching a closed sink use that sink's region ID as their catchment label. Regions reaching any cell of one initial water body share that body's minimum region ID. Wet cells themselves remain independent self-receivers: common labeling does not invent underwater transport. Per-body contributing land is obtained by aggregating dry-region areas by terminal label, not by assigning an entire ocean budget to each shoreline cell.

Each existing water body is a terminal domain even if it receives no dry-land contribution. Closed-sink counts exclude wet terminals. The closed-drainage fraction uses **dry land** as denominator and is zero when there is none. Labels reproduce within one generated world, not across future basin mergers or resolution changes.

## Complexity and limitations

Adjacency scans, component traversal, topological sorting, and accumulation are linear in regions and edges. Sorting flat exits adds at most `O(N log N)` work; storage is linear. Production uses no recursive route tracing or unbounded settling loop. Tests trace routes independently to audit termination.

Single-receiver routing does not split flow across downhill directions. Resolution, neighbor orientation, terrain noise, and flat tie-breaks affect boundaries. Tiny generated minima remain sinks. A sink alone does not establish depression extent, spill height, capacity, or parent basin: these require the next depression/spill stage.

## Versions and transport

Recipe fields are unchanged, but `modelVersion` is now `drainage-1`. Earlier recipes, including `water-1`, are rejected; preserve old files and create new recipes to explore their parameters. Upstream geology/water algorithms are unchanged, but new arrays and the version change the fingerprint.

Protocol 6 preserves earlier array ordering and appends:

| Field | Encoding |
| --- | --- |
| Receiver region IDs | N × Uint32 |
| Canonical terminal region IDs | N × Uint32 |
| Flat-routing steps | N × Uint32 |
| Contributing dry-land area | N × Float64 |

Additional size is `20N` bytes. Zero is a valid region ID; self-receivers denote terminals. The adapter checks bounds, adjacency, downhill/flat-rank constraints, wet terminal behavior, acyclicity, terminal labels, and reconstructed areas within `max(1e-6 m², expected_area * 1e-10)`. This is structural validation, not a second optimal-routing implementation. Corruption tests exercise the new fields.

Diagnostic frames still update only the unrelated diagnostic field. Headless output includes each terminal catchment's dry-land area and aggregate drainage statistics.

## Desktop views

**Catchments** colors regions by terminal label, not states, water volume, or active rivers. **Contributing area** displays `log(1 + area_in_km²)` normalized to the world's maximum, explicitly labeled logarithmic. Both show the bed on the globe and the same native fields on the flat atlas.

The inspector reports the immediate receiver, terminal label/type, contributing area, and applicable rule: bed descent, flat hops, water terminal, or closed dry sink. Global statistics report terminal domains, closed sinks, and the fraction of dry land terminating in them. No river paths, active-flow animation, spill contours, or new water surfaces are claimed by these layers.

## Validation

- Descending chain and closed bowl without bed modification or forced outlets.
- Open/closed/two-exit flats, deterministic ties, and a completely flat dry domain.
- A uniform dry sphere at the finest supported resolution (40,962 regions): one canonical sink, decreasing hop ranks, conserved contributing area, no recursive traversal.
- Distance-sensitive descent, exact gradient ties, unequal areas, and area/distance scaling.
- Several receiving cells of one water body, canonical body labels, exclusion of water area; fully wet worlds have zero contributing land.
- 20 seeds × levels 0, 2, 4 × water targets 0%, 71%, 100% = 180 combinations, each generated twice. Independent route walks check termination, adjacency, downhill/flat order, terminal labels, and land accounting. Diagnostic playback preserves routes, bed, and water.
- TypeScript summaries, corrupt routes/ranks/labels/areas, rejection of old versions, and immutability during display preparation.

### Recorded validation: September 23, 2026

Type checking, Rust formatting, Clippy with warnings denied, production build, Linux x64 packaging, development desktop checks, and packaged desktop checks passed. The automated suites contain 28 passing Rust tests and 40 passing TypeScript tests. Desktop checks cover the new layers and legends, downhill explanation, identical selected-region data between views, unchanged model fingerprint, and uniform dry/fully wet extremes. The packaged catchment screenshot was also visually inspected.

For the default `first-light` recipe at subdivision 5:

- Initial fingerprint: `2e752e9b`; model arrays: 5,732,140 bytes.
- 64 terminal catchments: 19 initial water bodies and 45 closed dry sinks.
- Dry-land area: approximately 147.915 million km²; 35.79% terminates in closed dry sinks.
- Maximum contributing area at one region: approximately 5.386 million km².

These are reproducibility observations, not targets or evidence of realistic river networks. Closed minima in un-eroded initial terrain are deliberately preserved.

Desktop timing on the development ThinkPad (Ryzen 5 PRO 4650U, Radeon Vega, 14.84 GiB usable RAM), with WebGL/GPU compositing enabled, Land and water selected, and 10× display exaggeration:

| Regions | Generation + both views | Flat RAF p95 | Globe RAF p95 |
| --- | ---: | ---: | ---: |
| 10,242 | 1,250 ms | 16.7 ms | 16.7 ms |
| 40,962 | 3,986 ms | 16.7 ms | 16.7 ms |

These are single-run end-to-end observations, not native drainage-only timings. The benchmark samples 180 animation-frame intervals per view during zoom changes and diagnostic transport; RAF timing does not measure GPU completion or guarantee future hydrology performance. No other project test/build was run concurrently with this benchmark. Generated screenshots and raw timing JSON are local artifacts, not committed assets.

## Next increment

[C2a basin analysis](basins.md) now supplies a standalone native connectivity hierarchy and level–storage queries, tested on controlled bowls, nested basins, and sills. It is not yet part of desktop generation or the protocol. Next integrate that analysis for inspection, keeping thresholds separate from actual water levels; only then introduce prescribed water input, storage, and overflow with a budget. Repeated global initial filling cannot replace independent basin dynamics.
