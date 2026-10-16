# Irregular mesh reference: geometry is not the whole landscape model

October 16, 2026. Optional `external-trimesh-review-1`. This follows the
[specific-area comparison](specific-area-transport.md), not a native mesh
rewrite, new World family, desktop integration or completed R3/R4.

## Question and implementation boundary

Does removing the regular direction stencil, under matched forcing, materially
repair the retained broad landforms? Keep the answer separate from whether an
irregular mesh is useful later for terrain representation.

`scripts/reference-trimesh.py` consumes the existing six-column f64-le patch
record and independently samples its **initial** height and uplift. Its final
native height and displacement columns are not initial conditions. Both mesh
arms use Fastscapelib 0.3.0 single-flow routing and MST sink resolution. Its
[documented eroders](https://fastscapelib.readthedocs.io/en/latest/guide_eroders.html)
support stream-power incision on triangular meshes but built-in diffusion only
on rasters. Consequently this comparison assembles its own symmetric cotangent
operator and backward-Euler diffusion; it does not compare an undiffused TIN
against a diffused raster and attribute the result only to geometry.

Dependencies are explicitly pinned optional Python 3.12 / Fastscapelib 0.3.0 /
NumPy 2.2.6 / SciPy 1.15.3 environments. Plotting separately pins Matplotlib
3.10.1. No Cargo/npm dependencies or production implementation change. No
external implementation code is copied. Production integration or bundling
would require a separate dependency/license review.

## Resolved geometry, quantities and approximations

The 160×160 source has nominal 125 m chart spacing and 20 km outer cell extent.
Actual fixed perimeter **centers** span 19.875 km; those coordinates are shared
exactly between three external meshes:

- Delaunay triangulation of regular positions: 25600 nodes.
- Independent PCG64 seed 1234 offsets, at most .35 nominal spacing per axis,
  only for interior nodes: 25600 nodes.
- SciPy PoissonDisk seed 1234, minimum interior spacing .8×125=100 m, with a
  .65-spacing margin from the same fixed perimeter: 23945 nodes in this case.

The Poisson point count is observed, not forced equal to the raster. Bounds
are 4–200 source cells per side, at most 40000 mesh nodes, 1–256 steps,
10–5000 m spacing and at most 50 Myr. Byte length, finite fields, uplift and
settings are checked before advancing. The fully resolved point/triangle data,
source SHA-256, mesh SHA-256, package/Python versions and settings are retained.
Existing source records are read-only. This is a planar approximation to a
small spherical patch, not a global seam/pole or storage-coupling demonstration.

Initial heights/rates use bilinear interpolation within retained cell centers.
An irregular node therefore does not generally have the same sampled initial
value or reference area as an old regular node. This changes topology, sampling
and nodal quadrature together. It is not an isolated change to neighbor direction.

Let mu_i be the external library's nodal reference area. An edge receives
`w_ij = (cot(alpha) + cot(beta))/2` from its incident triangles. The operator is
`(L h)_i = sum_j w_ij (h_i-h_j)`. With fixed external boundary values:

```text
mu_i (h_i(new)-h_i(old))/dt = -D (L h(new))_i
```

Assembly checks finite nondegenerate triangles and refuses negative active edge
conductance; it does not clip a bad operator into validity. The active sparse
system is factorized once. Each solve retains a relative linear residual and
refuses above `1e-10`. Known-triangle and affine-harmonic controls independently
check its sign, scale and fixed-boundary behavior. This is a numerical reference,
not a new production material ledger or a claim of general spatial convergence.

Unit-runoff sources are one at active nodes and zero at fixed nodes. Collected
C has units m². Two explicit pressure controls are used:

1. Total-area law: E=K_A C^.4 S, K_A=500^(-1.8), D=1.
2. Approximate specific law: E=K_s (C/sqrt(mu_i))^.4 S,
   K_s=340/20000^1.4, D=1. This is **not** native derivative-width transport.
   The external eroder still receives C in m², with effective per-node
   K_A(i)=K_s/sqrt(mu_i)^.4. Its stored source areas are never relabeled lengths.

MST connectivity assumes resolved depressions; it is not funded lake water.
After preparation, a separate unmodified-bed single-flow graph, without MST,
computes actual dry receivers, closed collection and terminal budgets. Plotted
cyan links come from this actual dry graph, not imposed lake connections.

## Twelve matched 128-step cases

All cases retain first-light initial bed/rates, 4.8 Myr, D=1, the same perimeter,
and identical settings within each pressure group. Values below are unweighted
p95 on the **common diagnostic point-query grid**, not native area-weighted p95:

| Case | Pressure | Regular | Jittered | Poisson |
| --- | --- | ---: | ---: | ---: |
| Synthetic | Total | 1522.992 | 1735.362 | 1745.034 |
| Generated | Total | 5073.719 | 5260.171 | 5282.682 |
| Synthetic | Specific approximation | 399.074 | 401.749 | 404.077 |
| Generated | Specific approximation | 3748.963 | 3759.768 | 3747.822 |

All twelve final actual dry closed fractions are zero. Maximum terminal relative
area residual is `1.53e-16`; maximum relative diffusion linear residual is
`1.98e-14`. Neither proves adequate geography or material/water conservation.

To check that the geometry really lost the grid stencil, measure
`R8 = abs(sum(edge_length * exp(8i angle))) / sum(edge_length)` over edges not
connecting two fixed nodes. R8 is 1.0000, .1080 and .00546 for regular, jittered
and Poisson meshes. The actual dry >5 km² link value on the generated specific
case changes 1.0000→.1114→.0504. This is an orientation diagnostic, **not a
universal realism score**; physical domain boundaries also influence directions.

Actual retained maps show less axis-aligned streams on both irregular meshes,
but broad smooth hillslopes and inadequate branching remain in these D=1
controls. Poisson results are not a materially complete R3 family. Thus the
evidence supports neither keeping every future physical grid regular nor
rewriting all World storage/water/climate around a TIN as the landscape fix.

The Poisson specific controls repeat their entire report except elapsed time.
128→256-stage unweighted actual-node bed differences are RMS 33.69/max134.63 m
(synthetic) and RMS54.18/max241.18 m (generated). Temporal sensitivity remains.

## Separate uncapped incision control

[Cordonnier et al., sections 5–6](https://cs.purdue.edu/homes/bbenes/papers/Cordonier16CGF.pdf)
use m=.5, k=5.61e-7/year and a 250000-year step, plus a 30° slope correction.
Their high-detail output additionally uses feature-based terrain construction.
Those are distinct operations, not the result of merely switching mesh type.

Four extra controls use that incision coefficient/exponent and 200 steps
(50 Myr), D=0, on the same retained 20 km inputs and regular/Poisson meshes.
The slope correction and terrain primitives are **not implemented here**.
This is an uncapped law/time control, not reproduction or disproof of the paper.

On Poisson points, synthetic/generated maxima reach 7203/12267 m, with common-grid
p95 5014/9385 m. Networks are far more dissected, but independent triangle
gradients exceed tan(30°) over 99.53%/99.68% of area; maximum gradient magnitudes
are 13.49/20.13. Such raw fields are not an acceptable ordinary Earth-like family.
Removing diffusion does not constitute a satisfactory hillslope model either.

## Decision and next bounded quality comparison

Do not adopt a blanket native TIN rewrite or claim R3 complete. Preserve the
useful direction result and use it when selecting fine-geometry representation.
The remaining quality investigation must combine channel structure and
hillslope/landform construction instead of repeating initial-noise searches.

Next compare an explicit drainage-conditioned terrain construction with these
actual beds: take a structural stream graph and its computed heights as physical
inputs, construct bounded valley/ridge slopes, and recompute drainage on the
resulting **authoritative height field**. This is a proposed bounded candidate,
not implemented functionality or a selected final production model. Require
actual height maps before expanding it into a general framework.

If construction changes a retained physical bed, measure the accepted change
and account for detached material; do not silently call it deposition or export.
If it defines new initial conditions instead, state that distinct scope. In
either case it must not be camera-generated noise or a visual layer that hides
a different hydrologic bed. Ordinary-seed/coast/global/multiscale acceptance in
the roadmap remains required; a better mountain patch does not finish it.

## Commands and verification

```sh
uv run scripts/reference-trimesh.py --cells 160 --spacing 125 --coefficient 0.000013862896863102924 --mesh poisson < artifacts/specific-area-final/synthetic-160-specific.f64le > artifacts/NEW_RESULT.json
uv run scripts/test-reference-trimesh.py
uv run scripts/plot-trimesh-reference.py REGULAR.json POISSON.json NEW_PLOT.png
```

Local final records are in `artifacts/trimesh-geometry-final`: twelve matched
cases, two exact Poisson repeats, two Poisson time refinements and four uncapped
law/time controls. Earlier `artifacts/trimesh-*` top-level files are preliminary
controls. Full points/triangles/heights and actual dry graphs, rather than PNGs,
are the numerical authority. Plots linearly query the physical mesh; they are
diagnostics, not conservative aggregation or extra physical terrain. Their
height range is explicit and slope lighting uses 1x, not exaggeration.

Nine optional reference tests pass: independent cotangent/link formulas,
affine and constant diffusion, exact zero diffusion, nodal/triangle area,
Poisson separation/perimeter/determinism, coupled repeatability, terminal area,
initial-versus-final input separation, units, physical transects, small-domain
null metrics, direction-order controls, and ten malformed CLI refusals with no
partial stdout. The five preceding external Fastscapelib tests also pass.
Degenerate and deliberately non-Delaunay active operators refuse without clipping.
Formatting/linting and scoped whitespace are checked separately.

An independent audit of all twenty final records checks finite heights,
unchanged collars, triangle/reference-area agreement, adjacent downhill
actual-dry receivers, acyclicity, reconstructed source accumulation and terminal
budgets. It retains source report SHA-256 values and steep-slope fractions in
`audit-summary.json`. These are static geometry/area checks, not water stocks.
Native code/defaults remain unchanged; the preceding 888-test native regression
is not represented as a new run. Concurrent owner UI work is untouched.
