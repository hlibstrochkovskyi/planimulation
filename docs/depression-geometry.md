# Depression geometry and the next landscape decision

October 14, 2026. Implemented: read-only native analysis
`depression-geometry-1`, paired cohort maps, and an opt-in uplands incision
preview. Neither analysis nor preview changes the selected desktop world,
recipe schema, native wire layout, old fingerprints or seasonal state.
This is R3 evidence, not completion of the Earth-like landscape milestone.

## Two different heights

`native/src/depression_analysis.rs` uses the existing plateau-batched basin tree
to enumerate dry minimum leaves. For each it reports:

- First sibling-merge depth: branch spill height minus minimum bed height.
- The leaf's prism capacity at that merge, not its whole drainage catchment.
- The dry area reaching the minimum in the existing static receiver graph.
- Minimum escape height to any existing initial water, and separately to the
  declared main ocean, minus that minimum's bed height.

A first spill can lead into another depression, not the ocean. The global
closed-sphere root has no first spill or finite capacity. Missing targets give
`null` escape analyses, not zero depths or an invented exterior boundary.
Branch IDs and minimum region IDs are local to the sampled world, not stable
lake identities across resolutions or changing beds.

## Independent minimax sweep

For the unchanged bed `h`, let `z = max(h, initial_water_level)`. Compute:

```text
escape[i] = min over paths i→explicit_target of max(z along path)
additional_depth[i] = escape[i] - z[i]
additional_geometric_volume = sum(area[i] * additional_depth[i])
```

The targets are either every existing wet region or just the main ocean's
existing regions. Other inland wet regions are not targets in the ocean
control. Their starting surface is the actual initial water level rather than
their underwater bed; existing water is not charged again as empty space.
All of this relies on the current initializer's prescribed common level. It
does not implement independent evolving lake levels or ongoing global fitting.

A multi-source minimax Dijkstra sweep uses a binary min-priority queue and
region-ID ties. An accepted witness edge points to an already settled node,
so the witness graph is acyclic even on flats. No epsilon is added to a height.
Witness edges can climb the **physical** bed: they prove potential connection,
not existing hydraulic flow, and must not be installed as physical drainage or
fed directly to the downhill-only stream-power kernel.

This independently derived sweep follows the minimax principle of
[Barnes, Lehman and Mulla's Priority-Flood](https://arxiv.org/abs/1511.04463),
adapted to explicit targets on a sphere instead of raster edges. No external
code is copied. The shared bounded geographic validator checks fields and
reciprocal adjacency; basin construction additionally checks connectivity.
The analysis uses iterative traversal, O(N+E) working storage and
O((N+E) log N) time under the bounded-degree surface contract. This does not
extend supported desktop resolution or qualify dynamic-water performance.

## Storage without nested double counting

Connected positive-depth cells necessarily share one minimax level: if a
neighbor had a lower escape level, its path would improve the higher one.
The analysis groups these into disjoint components and records their unique
cell prisms, dry/wet areas, maximum additional depth and escape height. Components
are sorted by volume, then canonical minimum region ID. Zero-depth sill cells
are excluded. Components are not the entire hierarchy of nested reservoirs.

These volumes describe potential geometry with the chosen targets held fixed.
They are **not actual water stock, material additions, simulated lake volumes,
rainfall required to fill a world, or a simultaneous global common level**.
Nested basin capacities are never summed. In the ocean control, component and
global additional volume can include isolated initial wet cells. The dry-only
depth-band volumes therefore need not sum to that global volume; the component
volumes do. Bounds are `(0,1]`, `(1,10]`, `(10,50]`, `(50,100]`, `(100,500]`,
and `(500,infinity)` meters, with zero reported separately. These are inspection
bins, not thresholds for declaring real lakes or deleting artifacts.

## Retained paired evidence

```bash
cargo run --locked --release --manifest-path native/Cargo.toml \
  --example geography_cohort -- artifacts/new-depression-cohort 6 --uplands
```

Observer `geography-cohort-3` adds baseline/candidate depression records and
four-panel potential-depth atlases. Existing physical fields, coverage pairs,
fixed-volume controls and fixed-terminal ablations remain unchanged. Output
directories must be new; a manifest is written last. Reports are diagnostic
files, not simulation checkpoints. Maps share one logarithmic depth palette
saturated at 500 m, with existing water still blue. The flat seam/pole polygon
approximation never modifies spherical adjacency.

Recorded run: `artifacts/depression-cohort-l6/manifest.json`, twenty ordinary
seeds and three directed controls. All 23 baseline and candidate initial
fingerprints match the earlier uplands-control cohort exactly. Both the
`first-light` atlas and full twenty-pair contact sheet were inspected.

Medians over twenty ordinary worlds, with equal weight per world:

| Diagnostic | Geography I | Geography II |
| --- | ---: | ---: |
| Dry minimum leaves | 223 | 220 |
| Within-world median first-merge depth | 11.63 m | 22.83 m |
| Share of leaves with first-merge depth <=10 m | 44.84% | 26.12% |
| Share of leaves with first-merge depth >100 m | 2.12% | 7.80% |
| Dry area with positive escape depth to any initial water | 9.46% | 6.04% |
| Dry area with positive escape depth to main ocean | 11.90% | 7.83% |
| Additional geometric volume to any initial water | 437,513 km³ | 537,932 km³ |
| Maximum additional depth to any initial water | 382.25 m | 450.04 m |
| Static closed-catchment share of dry area | 52.39% | 42.61% |
| Dry catchment area ending in <=10 m first-merge leaves | 16.62% | 8.11% |

All twenty candidates reduce positive-depth dry area, but fourteen have **more**
potential storage. Broad uplands reduce depression footprint while producing
deeper remaining basins. A closed catchment includes upland slopes, so its
42.61% area is not a claim that 42.61% of the land is lake floor or permanently
endorheic. Neither of these static measures establishes long-term lake state.

## Decision and bounded next process check

Do not blanket-fill or breach all minima. Nor does removing only <=10 m
first-merge barriers resolve all remaining closed catchments: escape can involve
several higher enclosing barriers. At the present approximately 112 km regional
width, even a shallow one-cell obstruction is not automatically a small physical
artifact. Finer sampling and structured relief still matter.

Extend the existing dry stream-power preview to the **same uplands family**:

```bash
cargo run --locked --release --manifest-path native/Cargo.toml \
  --example landscape_preview -- artifacts/new-uplands-incision.json \
  artifacts/new-uplands-incision.svg first-light 6 8 --uplands
```

`landscape-preview-2` records physical geography and depression metrics before
and after actual accounted bed incision, not a filled potential bed. Existing
default CLI recipes remain `continuous-plates-1`. Both previews start with zero
old smoothing, use the previously declared stream-power coefficients, rebuild
downhill drainage each step, hold original wet terminals fixed, and fit candidate
initial water once to the original resolved stock. The separate removed-material
reservoir, independent bed-loss check and all parameters remain in the report.
This is not yet a persisted production preparation model, sediment deposition,
calibrated runoff, ongoing geology or a claim of km-scale river valleys.

The first four subdivision-6, eight-pass uplands runs retain:

| Seed | Dry terminals before → after | Static closed dry area before → after | Mean dry incision |
| --- | ---: | ---: | ---: |
| `first-light` | 240 → 60 | 47.56% → 27.07% | 172.92 m |
| `4836187e` | 252 → 59 | 42.34% → 26.03% | 165.39 m |
| `geography-00` | 165 → 28 | 33.99% → 16.95% | 199.55 m |
| `geography-01` | 215 → 54 | 45.06% → 31.01% | 167.02 m |

These start with **zero** old smoothing and therefore have different initial
stocks/beds from the four-pass geography cohort. Each material-ledger relative
residual is zero in these observed runs; maximum represented-water relative
difference is `1.207e-14`. Original coverage is unchanged, as expected with
fixed wet beds and a base-level stop. Over 99% of dry area is incised, and dry
p95 elevations decrease by about 295–372 m. Storage increases in two of the
four worlds, despite fewer minima. The actual `first-light` physical atlas was
inspected: incision alone still leaves broad forms and excessive smoothing.
This setting is **not promoted** as a finished mountain/valley solution. The
next preparation choice must balance retained ridges, channel/hillslope scale,
effective runoff and closed basins, rather than optimize only sink counts.

Retained reports: `artifacts/uplands-incision-{seed}.json` with corresponding
SVGs and a PNG of `first-light`. A complete second `first-light` report matches
exactly after excluding only declared timings. The original continuous-plate
preview's physical fields, material/water values and history remain exact.

The same twenty ordinary seeds subsequently complete this zero-smoothing
incision comparison. Median dry terminals decrease from 220 to 57 and median
static closed area from 39.77% to 23.65%; all twenty reduce both measures. Median
mean dry incision is 184.60 m and median dry-p95 elevation loss is 347.47 m.
At least 99.21% of dry area is incised in every case. Eight worlds have increased
potential depression storage. Maximum relative material residual is
`1.444e-16` and maximum represented-water relative difference is `2.140e-14`.
The full physical-field contact sheet is retained at
`artifacts/uplands-incision-cohort-contact.png`. These findings reinforce the
non-promotion decision: improved coarse drainage is not sufficient evidence of
preserved mountain character or resolved fluvial valleys.

Only after inspecting that result should a preparation family and its material
ledger/persistence be integrated into the desktop. If flow-path conditioning
is subsequently needed, [Lindsay's constrained-breaching work](https://jblindsay.github.io/ghrg/pubs/Lindsay-HP-preprint.pdf)
is a useful algorithmic reference, but its DEM correction is not a physical
erosion law. Length/depth restrictions must have stated units and retained
closed-basin behavior rather than forcing a favorable drainage percentage.

## Verification scope

Eleven new native controls cover weighted nested bowls, first merge versus
eventual escape, inland-water versus ocean targets, disjoint prism accounting,
closed dry and fully wet planets, flats/reversed neighbor order, datum shifts,
area scaling, exact depth-band endpoints, represented micro-depressions,
malformed/disconnected/overflow inputs and a finest-supported iterative plateau.
Independent all-pairs minimax Floyd–Warshall checks spherical fixtures. Quantized
spherical controls compare the leaf catalog against independent dry terminals
and reconstruct their catchment area sums. Original inputs and exact repeats
are checked. The existing eight-pass generated-incision accounting test now
also covers four uplands seeds. New observer palette/family controls do not
replace full regression tests or establish geological realism.

Full `npm test` passes 703 native tests across 127 targets and 96 Node tests,
retaining one pre-existing ignored native test. Strict all-target Clippy,
typechecking, Rust formatting and whitespace checks pass. Five preview CLI
refusals preserve existing output and create no new report/atlas. The read-only
analysis has an independent code audit; no blocking defect was found and its
recommended leaf/band controls are included. Owner `map.ts`/`style.css` hashes
remain unchanged and those files are excluded from this increment. No new
desktop UI, protocol or packaged-app capability is claimed.
