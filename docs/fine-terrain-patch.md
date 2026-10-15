# Authoritative fine-terrain patch review

October 15, 2026. Native generation-only components:
`fixed-spherical-patch-1`, `fixed-geological-field-1` and
`fine-patch-landscape-1`; observer `fine-landscape-review-1`.
This implements the bounded fine-patch increment in the
[landscape roadmap](landscape-roadmap.md). It is **not** a supported World
family, a desktop resolution increase, a checkpoint format or global tile coupling.

## Physical geometry and initial conditions

`terrain_patch_geometry` constructs a bounded gnomonic quadrilateral chart on
the reference sphere. Recorded settings include radius, unit center direction,
orientation, dimensions and chart spacing. Shared vertices and cell centers
are sampled on the sphere; reference areas use spherical triangles, and face
and center distances use great-circle lengths. Chart spacing is not asserted
to be constant spherical spacing. Local basis construction handles poles and
the atlas seam without treating either as a physical boundary.

Bounds: 4–512 cells per axis, at most 262,144 cells, 10–5,000 m chart spacing,
and maximum chart extent/radius at most 0.05. Invalid settings refuse before
geometry allocation. Two-point side-face transmissivity is a small-chart
nonorthogonal approximation, not an exact Laplace–Beltrami operator. Eight
neighbors are available for routing; diagonal links are not diffusion faces.

`fixed_geological_field` samples the existing fixed-source geological bed at
arbitrary validated global directions. Continental thresholds retain the frozen
global quadrature, not a patch-fitted fraction. Source identity and independent
random streams remain those of [fixed geological sources](fixed-geological-sources.md).
Stateless queries agree exactly with the existing fixed-origin geodesic adapter
at shared sampled directions. Changing target sampling does not reroll sources.

The diagnostic uses the same 50×50 km outer chart at 1,000, 500 and 250 m
spacing: 2,500, 10,000 and 40,000 physical cells. The synthetic case has uniform
0.5 mm/year uplift, a zero fixed-height perimeter and an initially near-flat bed.
The generated case chooses the highest initial fixed-field bed among the 642
subdivision-3 global directions, with first ID breaking ties. This is a declared
sampling rule, not a claim to find the global maximum. Uplift derives from the
local convergence contribution: `1e-6/year * 2000 m * convergence / 6000 m`.

Both cases use a bounded 1 m, six-mode spherical perturbation with 4 km physical
wavelength and its own `fine-patch-seed-perturbations-1` stream. It is part of
the recorded physical initial bed, not camera-dependent decorative detail.
The perimeter is a one-cell fixed-height external collar, **not ocean**. Its
cell-center position and active area change with spacing despite equal outer
extents; these are not identical continuum boundary conditions across grids.

## Process composition and accounting

Each geological step applies prescribed uplift, rebuilds actual adjacent
downhill routing, applies the existing implicit `m=.5, n=1` incision primitive,
then runs stable explicit [linear hillslope transfers](hillslope-transport.md).
Equal-height plateaus can route to an actual equal/lower exit. Enclosed flats
retain a canonical dry sink. There is no automatic full-lake, fill or breach
connection through higher bed.

The incision coefficient has units year⁻¹, diffusivity m²/year, uplift m/year and
time years. The driver selects diffusion substeps for a maximum CFL target
0.8, with at most 64 substeps per geological step. Preparation is limited to
256 steps and an estimated `cells * steps * (3 + substeps) <= 128,000,000`.
Work-limit failures occur before advancing the internally owned bed. Numerical
and topology checks expect the pinned geometry builder; they do not authenticate
arbitrary caller-mutated coordinates as a reconstructed spherical chart.

Removed bed enters a detachment bookkeeping reservoir. It is not claimed to
have physically reached an outlet or been deposited. Prescribed uplift is a
kinematic reference-prism displacement, not a density-resolved mantle inventory.
Signed external diffusion exchanges and requested internal throughput remain
distinct. Per-step and cumulative actual-bed ledgers check:

```text
net accepted bed prism = accepted uplift prism - detached material
                      + signed external hillslope exchange
```

The tolerance scale uses those changes, never the large initial bed prism.
Subprecision uplift is reported without crediting unaccepted material. Existing
kernel refusals apply to lost external exchanges. No stock correction repairs
a failed ledger. Global pending/active sediment, water and climate are untouched.

## Reproducible inspection

Run with the repository's supported Rust toolchain:

```sh
cargo run --locked --release --manifest-path native/Cargo.toml --example fine_landscape -- artifacts/fine-review first-light 64 4800000 1
```

Arguments after a new output directory are optional seed, steps (1–128), total
duration (1–50,000,000 years) and diffusivity (0–1 m²/year). Library workload
limits may reject an otherwise valid longer duration. Existing output paths
refuse; successful manifests are written last. Defaults are 64 steps, 4.8 Myr,
`K=1e-6/year`, `D=1 m²/year`. Coefficients are controlled hypotheses, not calibrated
Earth properties. No external source code was copied.

Each variant retains six region-major f64-LE columns: initial bed, prescribed
uplift rate, prepared bed, accepted uplift displacement, removed incision
thickness and signed hillslope height change. JSON records source geometry,
resolved settings, ledgers, histories, metrics and native centerline profiles.
SVG panels show actual initial/incision/coupled heights with a common height
range and 1× slope lighting. Cyan links represent static contributing area,
not water discharge. They are not a new seasonal river simulation.

The latest observer additionally queries the same central 40 km transects at
250 m intervals with bilinear diagnostic interpolation. Its 1 km difference
roughness separates common physical support from neighbor-scale slopes. These
queries are not new physical samples, conservative restriction, or a mapping
that preserves channels, spill thresholds and storage.

## Retained results and acceptance decision

Local ignored artifacts include `fine-landscape-review`, `fine-landscape-repeat`,
`fine-landscape-temporal-32`, `fine-landscape-temporal-128`,
`fine-landscape-age-24my`, `fine-landscape-diffusion-control` and
`fine-landscape-qualified`. The latter retains current observer metadata for
the default coefficients. Raw physical records, not screenshots, are the
repeatability authority. These local paths are not checked-in fixtures.

Generated case, 4.8 Myr, 64 steps:

| Spacing | D (m²/year) | Bed p95 (m) | Neighbor RMS slope | Dry closed active area |
| ---: | ---: | ---: | ---: | ---: |
| 1,000 m | 0 | 5,679 | 0.302 | 0% |
| 500 m | 0 | 6,059 | 0.570 | 0% |
| 250 m | 0 | 6,226 | 1.048 | 0% |
| 1,000 m | 1 | 5,902 | 0.154 | 0% |
| 500 m | 1 | 6,098 | 0.176 | 0% |
| 250 m | 1 | 6,167 | 0.196 | 0% |
| 250 m | 0.01 | 6,245 | 0.670 | 43.33% |

Dry closed area is a static downhill-routing diagnostic, not observed lake
coverage or water loss. In the synthetic 250 m case, D=1 still leaves 4.79%
closed area. Coupled ledgers in the baseline remain below `1.5e-16` relative.
The 12 raw baseline physical records and source record repeat byte for byte.
These facts verify accounting/repeatability, not adequate geography.
The current observer's generated D=1 central-transect RMS slope at a fixed 1 km
query distance falls from 0.140 to 0.112 to 0.075 across these grids. Neighbor
roughness and apparently stable height quantiles cannot substitute for
fixed-physical-support agreement. These two transects are not world-wide statistics.

Actual maps expose strong axis/diagonal routing alignment. Incision alone makes
narrow steep valleys and broad unincised surfaces; D=1 smooths much of that
relief into broad rounded forms. Reducing D to 0.01 retains the aligned drainage
and can increase closed area. Increasing age to 24 Myr does not repair the
problem: generated maxima rise from 7,604 to 12,663 m across 1,000→250 m sampling.
Neither parameter adjustment is selected as a quality fix.

At 250 m with D=1, temporal refinement 32→64→128 steps gives bed RMS differences
58.32→13.71 m (synthetic) and 50.59→32.21 m (generated); the final generated
maximum difference remains 261.14 m. This is not timestep-qualified convergence.
Initial, no-diffusion and coupled versions are retained even when one looks
worse. No candidate is promoted to the desktop/default family.

## Relation to published models and next decision

[Theodoratos et al.](https://esurf.copernicus.org/articles/6/779/2018/) provide
the characteristic scales `sqrt(D/K)`, `U/K` and `1/K` for this fixed-exponent
model. Here D=1 gives a 1 km characteristic length; D=0.01 gives 100 m, already
below our finest 250 m spacing. A sharper image at that setting is not evidence
of spatial convergence. The linear diffusion law is not a complete steep-slope
failure model, and grid-scale channel representation remains an approximation.

The [Fastscapelib mountain example](https://fastscapelib.readthedocs.io/en/latest/examples/mountain_py.html)
uses a 250 m raster, single-flow routing, a separate MST sink resolver, `m=.4`,
`K=2e-4`, `D=.01`, random initial perturbations and fixed boundaries. Its coefficient
units and sink assumptions differ. We have not reproduced that experiment and
cannot copy its numerical coefficient into our `m=.5` model as a calibration.

An independent 22° chart-plane slope witness now checks the actual pinned
router: at 1,000, 500 and 250 m spacing its center receiver remains due east,
22° away from the analytic chart descent. Refinement shortens the step without
adding a new allowed direction. This isolates a limitation of the routing stencil,
not proof that it alone causes every observed landscape defect.

Next select a direction-flexible routing/mesh formulation with an explicit
incision discretization. [TauDEM's D-infinity description](https://hydrology.usu.edu/taudem/taudem5/help/DInfinityFlowDirections.html)
provides a primary reference for triangular-facet descent with up to two
neighbors. The subsequent adaptation implements the direction/area component,
not an accepted erosion model; its flat/pit assumptions remain explicit and
separate. Changing the simulation mesh or distributing
flow is a physical algorithm choice, not a renderer smoothing fix. Any later
humid-prehistory sink connection requires stated water/base-level assumptions;
it must not silently replace the existing conservative seasonal water solver.
Do not expand a parameter search before addressing the observed mesh alignment.

The first candidate should reconstruct descent on local triangular facets while
retaining this patch's spherical metrics, fixed external collar and real dry
sinks. Before coupling it to erosion, require affine-plane direction/slope
oracles over rotations, nonnegative normalized receiver weights, adjacent
strictly downhill transfers (with flats handled explicitly), an acyclic graph
and terminal contributing-area reconciliation. Area accumulation is not a water
inventory or a proof of flow convergence on arbitrary surfaces. The existing
single-receiver implicit incision update cannot silently be reused as a
multi-receiver equation: its slope/receiver interpretation and detachment ledger
need an explicit derivation and separate model pin. Retain the old patch as a
control, then compare actual prepared beds before accepting the new candidate.

The subsequent [triangular-facet routing review](facet-routing.md) implements
the direction/area component and read-only actual-bed comparisons. It does not
yet implement coupled erosion. A new counterexample exposes branch reversal in
the unconstrained extension below, so that extension is not accepted unchanged.

One proposed, **unimplemented** incision extension freezes the reconstructed unit
descent direction `d` during a geological step. With local receiver displacement
vectors `v_a`, `v_b`, write `d = alpha_a v_a + alpha_b v_b`. A direction inside
the facet cone gives nonnegative alphas with units m⁻¹. Then:

```text
S_i(new) = alpha_a (h_i(new) - h_a(new))
         + alpha_b (h_i(new) - h_b(new))
h_i(new) = [h_i(old) + dt K sqrt(A_i) sum(alpha_j h_j(new))]
           / [1 + dt K sqrt(A_i) sum(alpha_j)]
```

This is a frozen-characteristic approximation, not a fully implicit re-solution
of facet orientation or the gradient magnitude. Area-partition weights and
these directional-derivative coefficients are different quantities; neither
should be substituted for the other. Prove single-edge reduction, known two-exit
solutions, downstream ordering, no-increase behavior and actual detached-prism
accounting before choosing this extension. No existing model pin changes here.

## Verification scope

Geometry tests cover spherical area/lengths, reciprocal routing, rigid rotations,
seam/pole locations, nested outer extents, validation and maximum bounded size.
Field tests cover exact shared-direction agreement and query-order independence.
Preparation tests cover no forcing, real flat exits versus closed basins,
accepted uplift, repeatability, fixed boundaries, combined ledgers, diffusion
substeps, an independent single-step face balance, subprecision uplift and
aggregate workload refusal, plus the persistent plane-direction error witness.
Observer tests check arguments, perturbations and
independent affine diagnostic interpolation. Kernel analytic refinement controls
remain separate from whole-landscape convergence, which has not passed.

The first full `npm test` passes 827 native tests across 147 targets and 96 Node
tests, retaining one existing ignored native test. It predates the last four
preparation controls and the common-support observer control. The current focused
run passes all ten preparation tests and both observer tests. Strict all-target
Clippy, formatting, whitespace, seven actual CLI refusals, repeated physical
records and independent raw-height decomposition checks pass. The later full
`npm test` completes with 831 native tests across 147 targets, the same ignored
test, and 96 Node tests. This run contains nine preparation controls; the final
plane-direction witness passes in the separate ten-test focused run. Owner UI
files and UI-study artifacts are outside this increment.
