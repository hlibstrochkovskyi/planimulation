# Native irregular terrain path

October 17, 2026. Native geometry and two headless generation paths implemented;
no new World/default, desktop integration or completed R3/R4 is claimed. This follows
[drainage-conditioned construction](drainage-landform-construction.md).

## Bounded implementation choice

The implemented geometry generates fixed-perimeter Poisson interior points in
a gnomonic chart with an independent named seed stream. Use the pinned Rust
Spade triangulation library,
preserve input node IDs and canonicalize face/edge order. The library's
[stable bulk loader](https://docs.rs/spade/latest/spade/struct.DelaunayTriangulation.html#method.bulk_load_stable)
retains distinct input vertex order. Its
[published license](https://docs.rs/crate/spade/latest) offers MIT or Apache-2.0.
Use the dependency API, not copied implementation code. Pin both the direct
version and resolved dependencies; do not upgrade existing locked packages.

The chart triangulation is planar Delaunay, not spherical Delaunay. Directions,
edge distances and triangle/reference areas use the declared reference
sphere. Nodal reference areas are spherical triangle area thirds, not the
external comparison's Voronoi areas. They must not be silently interchanged.
The fixed perimeter is an external height constraint, never an inferred ocean.

The first implementation closed the native path using the existing prepared
source field and an explicitly declared bilinear sample onto the irregular
physical mesh. Then construct actual channel profiles/roofs and recompute dry
drainage there. Resampling does not invent resolved source features or prove
an irregular-mesh erosion simulation. Construction makes new physical heights,
not a camera-dependent overlay. Quantities are measured using the new geometry's
own reference areas; no existing World/sediment/water inventory is transferred.

Require actual retained native maps, source/construction provenance, exact
repeatability in the supported environment, geometry/area/distance controls,
malformed-workload refusal, and separate generation costs before integration.
Ordinary world morphology, terrain tiles, desktop picking, climatic/catchment
coupling and checkpoint continuation remain the roadmap's later obligations.

## Direct structural source candidate

The first native resampling maps retain straight large tributary directions
from regular preparation. Thus resampling closes a runtime dependency but does
not select a quality family. This motivated the direct initial-condition source
on the irregular mesh: sample geological potential and prescribed uplift there,
derive a minimax-to-perimeter scaffold, accumulate its active source areas, and
construct upstream initial heights with an explicitly capped steady-profile
grade `U/(K sqrt(C))`. Then apply channel/roof construction and rebuild actual
drainage. The scaffold is a procedural topology assumption, not actual funded
lake routing. Its potential levels and constructed initial heights are retained
separately. This does not fill an existing World's bed or transfer its stocks.
No geological duration is assigned to this initial-condition construction.

Both paths are now implemented. The direct source is the next integration
candidate; the resampled path remains a control that exposes inherited large
regular-grid directions. This is not a claim that geometry alone fixes terrain.

## Geometry and identity

`irregular_patch::build` owns geometry pin
`poisson-spherical-triangle-patch-1`. Boundary points retain the old regular
cell-center perimeter exactly. The interior has a .65 nominal-spacing margin
and .8-spacing minimum separation, with thirty area-uniform annulus candidates
per active point. An independently implemented active-list sampler uses a
spatial-bin lookup; it does not port an external implementation. The number of
accepted points is **measured, not forced equal to nominal grid dimensions**.

Spade 2.15.1 performs stable bulk triangulation. Duplicate removal or reordered
point identity refuses. Faces retain chart-CCW order with minimum node first,
then lexicographic sorting; unique canonical edges are sorted too. Euler counts,
one/two-face incidence, unchanged perimeter and positive spherical reference
areas/edge lengths are checked without repair. The geometry supports 4–256
nominal points per side and at most 262144 actual points; physical chart extent
and reference-sphere bounds remain those of the earlier patch geometry.

The mesh has its own named Mulberry32 stream, keyed by model, anchor and nominal
dimensions. Radius, spacing and orientation do not reroll dimensionless points;
geological fields and their random streams are separate. Full settings, seed,
stream name, random/model pins, Spade version, actual points, triangles,
directions, spherical distances and areas are retained. Different physical
densities can change local source topology and final heights: spatial convergence
is **not** established. Physical resolution is part of identity, not a camera LOD.

The dependency change adds Spade plus eight resolved transitive packages. All
eleven pre-existing non-root lock entries, including checksums, remain unchanged.
Published/package metadata lists MIT-or-Apache-2.0 for Spade, allocator-api2,
autocfg, equivalent, hashbrown, num-traits, robust and smallvec; foldhash lists
Zlib. The downloaded Spade MIT and foldhash license texts were inspected.
Distribution must retain applicable third-party notices; this prototype is not
a completed application-packaging/license audit. Dependency upgrades require
another reproducibility/model-identity review. No external erosion code is
copied or introduced as a production dependency.

## Direct source equations and assumptions

`structural_patch_source::build` owns pin `drainage-scaffold-initial-bed-1`.
The observer samples the existing fixed geological field and bounded physical
seed field directly at irregular directions. There is no interpolated regular
erosion output in this path. It retains those geological potentials P and
prescribed nonnegative uplift U, with U=0 at the external fixed perimeter.

```text
W_i = min_paths_to_perimeter max_potential_on_path
C_i = active reference area_i + sum_scaffold_donors(C_donor)    m²
grade_i = min(S_max, U_i / (K sqrt(C_i)))
H_i = max(W_i, H_parent + grade_i * physical_link_length)       m
```

Potential minimax parents form a deterministic acyclic generation scaffold.
Construct H downstream-first, retaining fixed heights. K has units 1/year for
this fixed m=.5/n=1 expression; grades are dimensionless. The relation is
steady-profile-inspired, **not a converged equilibrium on the resulting actual
drainage graph**. The example uses K=5.61e-7/year and S_max=tan(30°).

This is initialization, not millions of years of erosion, raising an existing
World's bed, or funded lakes. W is a geometric floor used to define new initial
conditions, not a water surface. The source does not debit fictitious eroded
material or advance a geological clock. Zero U returns W exactly, without an
epsilon slope. A connected-to-perimeter scaffold is an explicit local-mountain
assumption, **not a universal rule that planetary closed basins must disappear**.
Global terminals, retained endorheic basins and cross-patch channels still need
design and physical coupling before world promotion.

Next rebuild actual strict downhill receivers on H, apply the previous native
channel/roof construction and account its accepted removed reference prisms.
Rebuild actual drainage again on final physical heights. The retained scaffold,
source actual route and final actual route are separate arrays. They must never
be interchanged in an inspector. The source's profile cap applies to its
contribution along scaffold links; the subsequent roof bounds graph-edge
differences, **not continuous triangle gradients**.

## Commands and measured outcomes

```sh
# With the supported native Rust toolchain:
cargo run --locked --release --manifest-path native/Cargo.toml --example drainage_landform_patch -- artifacts/NEW_DIRECT_DIRECTORY first-light 160 --structural
cargo run --locked --release --manifest-path native/Cargo.toml --example drainage_landform_patch -- artifacts/NEW_RESAMPLED_DIRECTORY first-light 160 --irregular
```

Both commands are entirely native and require no Python runtime or external
prepared report. They retain source fields, resolved recipe/source identity,
native geometry, construction provenance, actual routes/collection and SVG
atlases. Default invocation remains the earlier regular-grid comparison. Its
complete physical reports repeat the preceding increment exactly except timings,
and its SVG bytes remain exact. Invalid numerical work creates no output
directory; completion is manifest-last, and existing outputs refuse unchanged.

The direct flag supports nominal sides 40–256. The old/default and resampling
controls remain bounded at 40–160 because they run the older preparation.
These are headless local experiments, **not higher supported desktop regions**.

First-light at nominal 160 has 24619 synthetic and 24610 generated-context
points. Geometry takes ~.10 s, direct field/scaffold source ~.31 s and
construction/actual rerouting ~.025 s per context in the first observation.
JSON/SVG generation and writing are additional costs. The generated final range
is 3231.1–3936.9 m, versus an intermediate source maximum of 11421.8 m.
The source is scaffolding, not a proposed exposed planetary surface.

The 20 fixed seeds used in the earlier geography cohort all complete, each with
synthetic and highest-initial-field local contexts (40 records). Actual point
counts span 24578–24768; mesh eightfold order spans .000129–.012012. Generated
nodal maxima span 3465.4–4456.7 m. All actual closed fractions are zero under
these through-connected local assumptions, not evidence of funded water or
realistic global lake distributions. Maximum construction material residual is
`1.85e-16`. Median complete seed invocation, including both contexts and
JSON/SVG writing, is ~2.13 s; concurrent work means this is not an isolated
benchmark. Original seed order and failures are retained, never rerolled.

At nominal 256, first-light has 63101/63050 points. One fresh native subprocess
including both complete contexts and JSON/SVG takes 7.42 s, with Linux child
peak RSS 379800 KiB (~371 MiB). This is a local generation observation, not
global resolution, interactive rendering, climate or checkpoint qualification.
The generated range is 3231.0–3968.1 m and terminal relative area residual
`1.11e-14`. A physical-density change is not an innocent display-only setting.

## Quality limits, verification and next integration

Actual maps were inspected, not just numerical reports. The direct path removes
the inherited cross-shaped main channels in the resampling control and produces
more dissected local patterns, but the raw triangle display is faceted and the
family is not calibrated to Earth. In the generated-context cohort, median
source profile-cap fraction is **94.37%**. Thus claiming that local relief is
primarily a quantitatively resolved uplift/erosion equilibrium would be false.
The capped construction choices dominate many source links. Broader geology-
conditioned relief, plains, coasts and lake-bearing contexts remain unqualified.

Nine geometry tests pass: exact repeated identities and stream independence,
independent pairwise separation/perimeter, face incidence/Euler counts, analytic
gnomonic-rectangle area and chord-distance oracles, physical rescaling and
rotation, seam/pole frames, bilinear tensor-polynomial/constant controls,
immutable refusal, actual coupled routing, and a real maximum-size build.
Four source tests check independent threshold-connectivity minimax levels,
ancestry-based areas and known grade formulas, explicit caps, exact zero forcing,
repeatability, invalid settings and unreachable-scaffold refusal. A separate
orientation-oracle test passes. Together with eleven prior construction tests
and 67 existing geometry/field/incision/transport/preparation tests, **92 native
tests pass across twelve nonempty focused targets**. Strict all-target Clippy
and formatting pass; this is not a new full-native or Node/UI regression claim.

An independent NumPy/SciPy audit checks all 40 actual cohort records: report
SHA-256, min-spacing, spherical area thirds, a minimax optimality certificate,
scaffold acyclicity and collection, source-height formulas, independently chosen
actual steepest receivers/areas before and after construction, fixed heights,
roof bounds and accepted debits. Maximum source-profile formula difference is
`1.82e-12 m`; maximum chart triangle gradient is 1.22225, despite graph-edge
slopes <=tan(30°). Triangle gradients are not silently relabeled as edge slopes.
Seven malformed new CLI requests create no outputs; existing manifests remain
unchanged. First-light direct physical reports repeat exactly except explicit
timings; both atlases repeat byte-for-byte. The owner's UI files are untouched.

Records: `artifacts/native-structural-cohort-160/{cohort,audit}.json` and the
40 physical JSON/SVG pairs; `native-structural-{first,repeat}`; the explicit
256 workload and its `native-structural-256-benchmark.json`; and
`native-irregular-{first,final,seed-5c151f46,seed-4836187e}` resampling controls.
The first resampling records precede the observer schema/edge-rasterization
cleanup and are retained as preliminary observations, not overwritten.

Next expose these owned physical data in an inspectable desktop view, retaining
model assumptions and source-versus-final distinctions. Local prototypes do not
meet the roadmap's twenty **global** worlds, directed morphology controls,
coast/biome quality, globe/atlas picking, terrain tiles/LOD, cross-patch storage,
conservative climatic/catchment coupling or checkpoint continuation. R3/R4 stay
open; no production model or resolution default changes in this increment.
