# Target-independent geological source geometry

October 14, 2026. Native initial-condition component
`fixed-geological-sources-1`, not a supported World/recipe family, desktop default
or completed Earth-like landscape milestone.

## Purpose and explicit approximation

The [direct-distance review](geodesic-belts.md) removes edge-only attenuation,
but source origins still depended on the target terrain grid. This component
constructs origins on a **separate pinned geological reference mesh**:
subdivision 5, 10242 spherical regions. Subsequent terrain sampling never changes
that reference geometry. This is a discretized structural approximation with a
fixed error scale, not exact continuous tectonics or integration of plate history.

At Earth radius the mean reference footprint is about 223 km. It is not a claim
that small faults or kilometer-scale tectonic detail are represented. Changing
the geological reference resolution requires a new source pin and identity;
it is not a camera setting or an implicit side effect of terrain refinement.

## Construction and identity

`structural_sources::build(&Recipe)` accepts validated, unprepared v2 reference
parameters. It builds the existing continuous seeded plate roots, connected
weighted partition with seeded resistance, motion, and v2 crust together on
the reference mesh. It retains actual half-dual boundary arcs and existing
closing/ridge/rift amplitude laws. An unweighted spherical Voronoi partition is
not substituted. No target grid, supplied terrain, erosion, water or display
buffer enters source construction.

The construction identity records:

- Source, envelope, plate-root, crust and random pins;
- Reference subdivision;
- Seed, physical radius, plate count and maximum speed;
- Continental fraction and scale.

Target terrain subdivision, width, relief scale, local detail and water are
excluded: they do not affect raw source geometry or raw amplitudes. Width and
relief scale affect subsequent sampling/composition. Changing motion changes
weights, not origin locations. Stopped/zero-amplitude arcs remain in the owned
source record so record-local IDs/order remain stable; the envelope index filters
them from evaluation. Identity is construction metadata, not authentication of
arbitrary caller-mutated public arrays or a resumable checkpoint.

Old root/crust/motion/resistance implementations remain dependencies of this
pin. Do not change those frozen dependencies in place and retain this identity.
No new subsystem consumes another subsystem's random stream.

## Initial terrain composition

`geodesic_belts::Origin::FixedBoundaryArcs` samples these sources with the existing
direct spherical envelope kernel. It retains target-grid v2 baseline, detail and
relief modulation. Trusted same-world component inputs are still required by
the lower-level terrain adapter. It exports the source identity alongside the
owned origin record.

Shared target directions yield identical initial source contributions and bed
heights when all composition parameters match. This does **not** imply identical
water coverage fits, slopes, basin connectivity, accumulated drainage area or
prepared landscapes across grids. Those use physical discrete geometry and
remain separate convergence/coupling obligations.

The `relief_landscape --geodesic` review now emits
`geodesic-belt-review-2`: graph, geodesic original region seeds, target-sampled
arcs and fixed-geology arcs. It retains the original graph-case water stock and
identical prescribed preparation settings across the four initial worlds.
Earlier version-1 records remain readable; neither diagnostic pin is a recipe
or checkpoint pin. The read-only renderer accepts both review versions.

```bash
cargo run --locked --release --manifest-path native/Cargo.toml \
  --example relief_landscape -- artifacts/NEW_DIRECTORY 6 --geodesic

cargo run --locked --release --manifest-path native/Cargo.toml \
  --example relief_landscape -- artifacts/NEW_DIRECTORY 8 first-light --geodesic
```

## Verification and remaining requirements

Six source tests verify byte-identical owned records/identities and queries
despite changing target resolution/detail/relief/water; reconstruction from
canonical weighted reference components; speed/stopped geometry invariance;
seed/material/width controls; supported maximum 32 plates; seam/pole BVH/full-scan
agreement; finite unit arcs and unchanged production recipe limits.

The terrain adapter additionally compares different target grids with different
target plate-boundary counts: source geometry/identity and every shared initial
baseline, detail, convergence, divergence and bed value agree exactly. Existing
immutable-component and zero-relief/stopped tests now include fixed origins.
The new focused source/adapter/observer/renderer run passes 21 tests. Strict
all-target Clippy passes. Full `npm test` now passes 800 native tests across
142 targets and 96 Node tests, retaining the pre-existing ignored native test.
Formatting, whitespace and old-family/kernel regression checks pass. Owner
renderer files remain unchanged and excluded.

## Retained comparisons

`artifacts/fixed-geology-l6/manifest.json` completes 23 four-way cases.
Ordinary median closed dry share is 9.7819% for fixed arcs, versus 10.9787%
for graph and 10.1111% for target-sampled arcs. Fixed arcs improve that measure
in 15/20 ordinary worlds, not every world. Median dry p95 is 1574.49 m and
closed minima count 8. All 69 preceding graph/point/sampled-arc physical records
remain byte-identical; all four low-relief raw records are identical.

`artifacts/fixed-geology-dense-l8/manifest.json` completes all four variants
at 655362 samples. Fixed first-light retains 2724 geological arcs, compared
with 21886 target-grid arcs. Its mean/max evaluated arcs per query are
66.53/218. Recorded envelope stage is 13.76 s, preparation 52.86 s and full
variant 69.60 s, under overlapping test/cohort workloads. These are not isolated
benchmarks or evidence of interactive performance.

The source geometry JSON is exactly identical between 40962 and 655362
sampling, and every one of the 40962 shared initial bed values agrees exactly.
Prepared beds intentionally differ: fixed dense closed dry share is 34.35%,
minima 140 and dry p95 1759.11 m, versus coarse 10.62%, 6 and 1573.44 m.
Independent initial origins do not remove spatial sensitivity in drainage and
preparation. Dense material/water residuals are `1.42e-16` / `6.12e-14`.

Actual maps inspected: `artifacts/fixed-geology-first-light-l6.png` and
`artifacts/fixed-geology-dense.png`. They retain broad landforms. The version-2
read-only renderer also reconstructs canonical final water/metrics and renders
the retained 40962-region fixed record without advancing simulation.

Initial consistency is not proof of plausible mountains or drainage. Physical
scale detail, drainage-aware landforms, both desktop views, fine terrain tiles,
coarse climate/catchment mappings and checkpoint support remain R3/R4 work.
