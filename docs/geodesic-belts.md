# Direct spherical belt envelopes

October 14, 2026. Native initial-condition review, **not a selectable World,
desktop default, evolving plate model or completed R3 milestone**. Existing
recipes and geographic families remain unchanged.

## Separate interventions

The [distance control](physical-relief-review.md#follow-up-measured-graph-distance-bias)
found persistent directional excess in edge-only graph distance. The new kernel
`geodesic-arc-envelope-1` evaluates the strongest weighted source by direct
reference-sphere distance. Two adapters deliberately separate propagation from
source placement:

| Variant | Source origins | Attenuation |
| --- | --- | --- |
| `graph-300` | Frozen region seeds | Frozen graph shortest paths |
| `geodesic-points-300` | Exactly those region seed positions/amplitudes | Direct great-circle distance |
| `geodesic-arcs-300` | Actual sampled half-dual boundary arcs | Distance to the entire minor arc |

All widths are 300 km. Retain v2 crust columns, plate motion, geographic detail,
relief modulation and their independent random streams. No added noise, guessed
outlet, extra plate motion or renderer-defined physical detail enters this review.

The initial contribution in channel j is:

```text
envelope_j(p) = max_s amplitude_s,j * exp(-radius * distance_angle(p,s) / width)
```

Channels are closing, oceanic spreading ridge and continental rift magnitudes.
Original amplitude laws and final convergence/divergence modulation are retained.
This changes initial relief; prescribed uplift derived from that relief changes
too. It is not a pure visual intervention.

## Geometry and acceleration

A minor arc is represented as `a*cos(t) + tangent*sin(t)`, for `0 <= t <= length`.
The closest interior candidate maximizes the dot product with the query direction;
otherwise an endpoint wins. Angular distance uses `atan2(norm(cross), dot)`.
Coincident endpoints represent a point. Distinct unresolved tiny arcs, ambiguous
near-antipodal endpoints, nonunit directions and nonfinite inputs refuse. The
midpoint uses the arc basis, avoiding near-antipodal cancellation in `unit(a+b)`.

[CGAL's spherical-arrangement manual](https://doc.cgal.org/latest/Arrangement_on_surface_2/index.html)
describes minor great-circle arcs and antipodal ambiguity. Our implementation
and tests are independently derived; no CGAL implementation or dependency is
imported. Floating-point guard bands are not exact rational constructions.

A deterministic balanced BVH groups spherical source caps. Triangle-inequality
distance lower bounds and per-channel maximum amplitudes bound each subtree's
possible contribution. Do not choose only the geometrically nearest source:
a farther, stronger source can win. Duplicate equal positive contributions use
the lowest original source index. Zero/underflowed values have no winner.
The accelerated result is compared with an unpruned full-source oracle, including
wide arcs, seams, poles and near-antipodal limits. Numerical padding and tests
are not a universal proof of floating-point conservative bounds.

Source count is bounded at 131072. Tree queries still have a full-scan worst
case, and allocate a traversal stack per sampled region. Performance figures
describe the retained generated sources, not adversarial guaranteed latency.

## Important structural limitation

The existing plate partition is connected **weighted graph growth**, with a
continuous seeded resistance field. Stable seeded root directions do not make
its sampled boundaries resolution-independent. These adapters reuse that actual
partition, its sampled arcs and motion; they do not silently replace it with an
unweighted spherical Voronoi partition. Thus even direct-distance arc origins
still change with physical sampling resolution.

Adapter checks validate array shapes, finite fields, zero preparation, arithmetic
decomposition and finite output. They do not authenticate that separately supplied
crust/plate/terrain arrays were generated from the reference recipe. Callers must
supply trusted same-world components. The review obtains all components together
through its initial-field builder. Source indices are record-local, not persistent
plate or region identities.

Next structural requirement: construct and pin source geometry independently
of the target terrain grid, then test shared-direction samples across resolutions.
A fixed reference geological mesh is a possible explicit approximation; it is
not equivalent to a fully continuous evolving tectonic model.

## Reproducible physical comparisons

```bash
cargo run --locked --release --manifest-path native/Cargo.toml \
  --example relief_landscape -- artifacts/NEW_DIRECTORY 6 --geodesic

cargo run --locked --release --manifest-path native/Cargo.toml \
  --example relief_landscape -- artifacts/NEW_DIRECTORY 8 first-light --geodesic
```

No seed runs twenty ordinary seeds and fragmented/broad/low-relief controls.
The graph variant resolves water coverage once; changed initial beds retain its
resolved water stock, and identical beds reuse the exact initial water snapshot.
Each variant runs the same 32-step sedimentary preparation. This is an initial
world ablation, not a dynamic intervention or actual lake redistribution.

`geodesic-belt-review-1` retains source geometry JSON, resolved reference
parameters, source/kernel pins, query counts, initial/prepared metrics, physical
raw arrays, material histories and water-stock diagnostics. The newer record also
includes envelope construction/sampling seconds. The manifest is written last.
These artifacts are diagnostic records, not resumable checkpoints.

`artifacts/geodesic-belts-l6/manifest.json` completes all 23 three-way cases.
Ordinary area-weighted diagnostic medians:

| Diagnostic | Graph | Geodesic region seeds | Geodesic sampled arcs |
| --- | ---: | ---: | ---: |
| Static closed dry share | 10.9787% | 10.7927% | 10.1111% |
| Closed dry minima | 10.5 | 9 | 9 |
| Dry height p95 | 1456.90 m | 1482.57 m | 1583.57 m |
| Worlds with lower closed share than graph | — | 13/20 | 14/20 |

The maximum relative material-ledger residual is `5.56e-16`; initial-water stock
reconstruction residual is `3.28e-14`. These prove measured accounting behavior,
not scientific calibration. Static closed share is not observed endorheic coverage.

All 23 graph raw records are byte-identical to the preceding wide/old-detail
factorial controls. The three low-relief records are identical. A selected
first-light repeat reproduces all eight physical columns and both source files
exactly, despite later observer heading/timing additions. Retained repeat:
`artifacts/geodesic-belts-repeat-l6/manifest.json`.

## Dense results and decision

`artifacts/geodesic-belts-dense-l8/manifest.json` completes 655362 samples.
For first-light:

| Diagnostic | Graph | Geodesic region seeds | Geodesic sampled arcs |
| --- | ---: | ---: | ---: |
| Static closed dry share | 35.82% | 33.57% | 35.71% |
| Closed dry minima | 154 | 140 | 130 |
| Dry height p95 | 1719.57 m | 1755.81 m | 1780.88 m |
| Source count | — | 10912 | 21886 |
| Mean evaluated sources/query | — | 106.25 | 138.47 |
| Maximum evaluated sources/query | — | 442 | 652 |
| Envelope stage | — | 16.27 s | 28.37 s |
| Coupled preparation | 57.75 s | 29.40 s | 29.18 s |

This run overlapped regression work. Its graph timing is not a controlled speed
comparison, and envelope-stage timing includes writing the source geometry file.
Previous preparation-only measurements are documented separately; this run does
not establish interactive performance or dense World/checkpoint compatibility.

Actual-field SVGs convert with ordinary converter limits. Inspected PNGs:
`artifacts/geodesic-first-light-l6.png` and `artifacts/geodesic-arcs-dense.png`.
Lighting uses declared display-only 20x slope exaggeration; colors are not
biomes, and blue links are static receivers, not river discharge.

Direct geometry removes the identified graph-distance proxy, but the dense map
still has broad forms and many closed basins. Source-origin changes have mixed
drainage effects. **Do not promote either adapter as the Earth-like terrain fix.**
Independent structural origins, drainage-aware landforms, physical-scale detail,
both desktop projections and scale coupling remain open R3/R4 requirements.

## Verification scope

Seven kernel tests cover analytic point decay, independently sampled arc minima,
known endpoint/interior/pole distances, accelerated/full-scan exact agreement,
wide arcs, stronger-farther sources, deterministic duplicate ties, zero sources
and invalid/unresolved geometry. Three adapter tests cover repeatable composition,
unchanged owned columns/detail/inputs, stopped/zero relief and immutable malformed
or overflow failures. Boundary arithmetic checks are not full seed authentication.

The independent read-only review identified tiny-arc underflow and finite-input
composition overflow refusal edges; both now have guards and regression controls.
These guards do not change the retained valid generated physical records.
Full `npm test` passes 793 native tests across 141 targets and 96 Node tests,
with the pre-existing ignored native test retained. The two subsequent refusal
guards and observer metadata correction pass focused kernel/adapter/observer/
renderer checks (21 tests); the full run preceded these guards. Five release
CLI refusals preserve existing outputs and do not create a new directory.
Strict all-target Clippy, formatting and whitespace checks pass. Owner renderer
files retain their hashes and are excluded from this increment.
