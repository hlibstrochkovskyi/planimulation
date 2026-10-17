# Drainage-conditioned physical landform construction

October 17, 2026. Native `drainage-landform-construction-1`, following the
[irregular-mesh comparison](irregular-mesh-reference.md). This is an implemented
generation component, **not** a new World family, an Earth-calibrated erosion
history, desktop integration or completed R3/R4.

## Scope and decision

Do not require a quantitatively resolved geological history before constructing
useful initial geography. A declared procedural landform construction is allowed,
but its output must be the physical bed used by subsequent routing and inspection.
It cannot be renderer noise that hides a different simulation surface.

The comparison now has three explicit candidates on retained unstructured beds:

1. A lower slope envelope after the uncapped preparation.
2. The same envelope after every uplift/incision step.
3. Concave channel-profile constraints followed by a lower slope envelope.

The first two suppress extreme peaks but leave overly uniform broad slopes.
The third retains a substantially more dissected valley/ridge pattern in these
two local examples. It is the selected **construction candidate for further
integration**, not an accepted global generator. The native regular-grid control
still exposes strong axial channels; porting the formulas alone does not remove
the source geometry's limitations. Do not promote that control as the new family.

## Construction, physical quantities and approximation

The native kernel accepts finite bed samples, positive reference areas, fixed
height flags, unique canonical undirected edges with physical distances, and an
adjacent nonascending acyclic structural receiver graph. Fixed nodes must be
terminals. It validates these numerical properties, not arbitrary geometry's
provenance. The patch adapter obtains geometry and both actual-bed routes from
their pinned native builders. The unstructured observer independently rebuilds
strict steepest-descent receivers on both retained source and constructed bed;
it does not trust source reports' stored drainage arrays.

Active sources contribute their physical reference area, fixed sources zero:

```text
C_i = area_i(active) + sum_donors(C_donor)                    m²
channel_i = C_i >= C_head
S_i = S_head * (C_i / C_head)^(-theta)                        dimensionless
profile_i = min(input_i, profile_receiver + S_i * distance_i) m
```

Integrate the profile downstream-first, retaining terminal heights. Channel
selection is downstream closed because collection cannot decrease downstream.
This normalized concave profile is a **generation choice**, not a solution for
the source bed's original age, uplift and erodibility. A power-law relation
between grade and drainage area follows the steady uniform-forcing stream-power
discussion in [Cordonnier et al., section 6.4](https://cs.purdue.edu/homes/bbenes/papers/Cordonier16CGF.pdf).
Our threshold, grade, lower-only constraint and roof operator are independent
choices; this does not reproduce their slope correction or landform primitives.

All non-channel samples initially retain their input upper constraint. Apply
multi-source Dijkstra to obtain the lower graph-distance envelope:

```text
h_i = min_j(profile_j + S_roof * shortest_graph_distance(i,j))
```

The operator only lowers heights and consumes no random stream or simulation
time. Incompatible fixed heights refuse the complete construction; they are not
restored afterward by a clamp. Graph order and heap ties use stable node IDs.
Zero/nonfinite/overflowing or unrepresentable positive offsets refuse without
mutating caller inputs. Limits are 262144 nodes and at most four times as many
unique undirected edges; the latter is not a per-node degree cap.

The roof bounds **edge differences**, not the continuous gradient inside a
triangle. For example, heights `(0,1,1)` on a unit right triangle satisfy every
edge slope <=1 while the triangle gradient is sqrt(2). Both quantities are
measured; describing this as a universal 30-degree terrain limit would be false.
Nor is a prescribed slope roof a time-resolved thermal/landslide transport law.

## Accepted changes, drainage and inspection

For each node, measure actual accepted `input-profile`, `profile-h` and
`input-h`. Multiply each by that node's reference area. Compensated sums retain
separate channel-profile and roof debits, a combined detached-material reservoir
and a `1e-12` relative reconciliation gate. This is reference-prism bookkeeping,
not measured outlet export, deposition, a regolith layer or material density.
Future integration must explicitly incorporate it rather than silently changing
global sediment/water stores.

Retain the profile constraints, contributing areas, channel mask, accepted
removed thickness, settings/model pin and the limiting roof-constraint node.
The latter supports WHY provenance; it is **not a physical drainage receiver**.
Actual receivers and source collection are rebuilt after construction, since
other constraints may cut across the retained structural tree. No fabricated
rainfall, funded lake or epsilon slope is introduced.

The generic strict router keeps flats/minima as terminals. The older spherical
patch router additionally handles equal-height paths to actual exits and
canonical closed flats. These are distinct declared policies, not identical
solvers. A real enclosed bowl remains closed in the no-change patch control.

## Retained unstructured comparison

The two retained sources use the earlier 23945-node Poisson mesh, 19.875 km
perimeter-center extent, uncapped m=.5 incision, 200 steps and 50 Myr. Geometry
and source preparation remain **external** in this comparison. Construction uses
`C_head=250000 m²`, `S_head=.1`, `theta=.5`, `S_roof=tan(30°)` and unchanged
source boundary heights. Values below are actual nodal maximum heights, not
an interpolated diagnostic quantile:

| Case | Uncapped source (m) | Envelope afterward | Envelope each stage | Channel profiles + envelope |
| --- | ---: | ---: | ---: | ---: |
| Synthetic | 7203.353 | 3984.955 | 3342.306 | 667.182 |
| Generated mountain context | 12266.992 | 7450.752 | 6942.303 | 3929.931 |

The generated channel construction is a dissected upland patch, not evidence
that every mountain should have only ~700 m local relief. Relief and geological
conditioning still need ordinary-world evaluation. Do not silently interpret
post-construction output as a faithful continuation of the 50 Myr source history.

Native channel construction agrees with the independent Python calculation to
maximum absolute height differences `2.28e-13 m` / `9.10e-13 m`. Final native
and external actual-dry receiver arrays agree exactly on both retained cases;
their collected areas agree within independent floating-point accumulation
tolerances. Of 3177/3280 selected structural channel links, 0%/.3049% change
their immediate receiver after construction. Both final closed fractions are
zero, which is not a general realism or lake-simulation criterion.

Accepted combined detached reference prisms are `9.86992636395e11 m³` and
`1.16915954389e12 m³`. Maximum triangle gradients are 1.1461/1.1601, with
75.58%/75.88% of triangle area above tan(30°), despite the graph-edge bound.
This remaining geometry/roof limitation is explicit, not hidden by the maps.

The native observer's construction plus both receiver reconstructions and final
area accumulation take roughly .03–.05 s on this machine at 23945 nodes. This
excludes external source generation, JSON parsing/writing, mesh validation and
atlas rendering; concurrent work ran during these observations. It is not a
planet-scale or interactive desktop benchmark.

## Native implementation and commands

`native/src/drainage_landforms.rs` contains independent Rust implementations of
channel construction, the heap envelope, accepted-material bookkeeping, a strict
actual-bed graph router and the spherical-patch adapter. No Cargo dependencies,
external implementation source, old model pins or production defaults change.

```sh
# Use the supported Rust toolchain, as for other native examples.
cargo run --locked --release --manifest-path native/Cargo.toml --example drainage_landform_patch -- artifacts/NEW_PATCH_DIRECTORY first-light 160
cargo run --locked --release --manifest-path native/Cargo.toml --example drainage_landform_mesh -- artifacts/trimesh-geometry-final/generated-paper-law-poisson.json artifacts/NEW_MESH_DIRECTORY
```

The first command generates two complete patches **entirely in Rust**: a
controlled synthetic context and the highest initial fixed-field context among
subdivision-3 directions (first ID breaks ties). It uses existing specific-area
preparation for 4.8 Myr/128 stages, D=1, K_s=340/20000^1.4, m=.4, then separate
construction. It retains resolved recipe/source/seed-field identity, geometry,
initial bed, prescribed uplift, source history, both physical beds, construction
provenance, actual final routing and direct SVG comparisons. Its eight-neighbor
regular geometry remains a visible limitation. Source preparation costs roughly
6 s per 25600-cell patch and construction/rerouting ~.02 s in this observation.

The second command constructs the retained unstructured physical bed in Rust.
It does **not** generate the external initial mesh or source erosion history.
It validates bounded JSON, indices, unique nondegenerate triangles, reference
area sum and kernel inputs. This is not a general triangulation certificate.
Exact source bytes receive an FNV-1a64 diagnostic binding, not security
authentication. Reports retain full source geometry/bed and new physical fields.
Native SVGs shade the actual triangles with common 1x normal lighting and no
height normalization, extra noise or interpolation.

All commands require a new output path. Numerical refusal happens before output
creation; a completion manifest is written last. They do not alter recipes,
World/checkpoint/IPC schemas, current water stores or the owner's UI changes.

Optional independent controls:

```sh
uv run scripts/reference-slope-envelope.py SOURCE_EXTERNAL.json NEW_REPORT.json --mode channels
uv run scripts/reference-slope-envelope.py SOURCE_EXTERNAL.json NEW_REPORT.json --mode post
uv run scripts/reference-slope-envelope.py SOURCE_EXTERNAL.json NEW_REPORT.json --mode coupled
PLANIMULATION_LANDFORM_MESH_BINARY="$PWD/native/target/release/examples/drainage_landform_mesh" uv run scripts/test-reference-slope-envelope.py
uv run scripts/plot-slope-envelope.py SOURCE_EXTERNAL.json POST.json CHANNELS.json NEW_PLOT.png --streams
```

The reference pins Python 3.12 / Fastscapelib .3.0 / NumPy 2.2.6 / SciPy 1.15.3;
the plot separately pins Matplotlib 3.10.1. It is not a production dependency.
Plots verify matched exact source bytes/process/mesh settings and refuse existing
outputs. Their linear point-query shading is diagnostic; native SVGs separately
expose actual triangle geometry. Neither display changes the bed.

## Verification and remaining product work

Eleven new native tests pass: independent unequal-area channel/roof debits,
known concavity profiles, an all-pairs min-plus oracle, edge-order determinism,
idempotence with no selected channels, constant/affine/disconnected fields,
incompatible-boundary immutable refusal, malformed/cyclic/uphill/nonadjacent
graphs, limits/overflow/subprecision, dimensional rescaling/datum shifts,
actual strict routing/ties/flats, an enclosed-bowl adapter and a 20000-node chain.
The focused native run passes 54 tests across five targets, including existing
specific-area, hillslope, facet and patch compatibility tests. A second run
passes 24 existing incision, physical seed, fixed-source and spherical geometry
tests, plus compilation of both example test targets (zero internal tests).
Thus 78 native tests pass across nine nonempty focused targets. The preceding
888-test full native regression is not represented as a new full run here.

Nine optional reference tests pass with the native binary explicitly selected,
including an independent native/reference physical comparison, actual receiver
and collection equality, input-byte binding, seven malformed native CLI
refusals without output directories and existing-output preservation. Without
that explicit binary the native cross-check is skipped, not claimed passing.
Separate controls check coupled material accounting, channel formulas, graph
versus triangle slopes, boundary refusal and deterministic envelope oracles.

Final six external construction controls are in `artifacts/slope-envelope-final`;
native unstructured final/repeat records are
`artifacts/native-landform-mesh-{synthetic,generated}-{final,repeat}`. An independent
audit (`artifacts/native-landform-audit.json`) binds report SHA-256 values, checks
native repeatability except timing, matches both receiver graphs against the
external actual-bed router, verifies reference agreement, accepted debits,
unchanged boundaries and all six cumulative material balances. Earlier
`slope-envelope-*` files are retained preliminary observations, not rewritten.
The native regular source comparison is in `artifacts/native-landform-patch-final`.
Its `native-landform-patch-repeat` reports agree exactly except declared timings,
and both SVG atlases repeat byte-for-byte. The separately retained
`native-landform-repeat-audit.json` checks physical-field repetition, direct
accepted reference-prism debits, unchanged boundaries and both exact external
channel-construction repeats. Five malformed native patch CLI requests create
no output; refusing an existing output preserves its manifest. Strict all-target
Clippy, Rust/Python formatting and Python linting pass. No Node/UI regression
is claimed against concurrent owner UI work.

Next implementation priority is a direction-flexible **native fine-terrain
generation path** feeding this construction, followed by an inspectable desktop
view of the resulting physical data. Do not repeat a broad coefficient/noise
search or rewrite every global subsystem around a TIN. This component does not
close ordinary-seed/global coast quality, plate history, climate coupling,
terrain tiles/LOD, seam/pole handling, conservative restriction or checkpoints.
Those R3/R4 requirements remain open.
