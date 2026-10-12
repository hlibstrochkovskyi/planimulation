# Positive elementwise mobility integration

October 12, 2026: `regional-surface-flat-element-mobility-candidate-1` is a standalone flat, closed-land coefficient integration experiment. The preceding [incident-triangle comparison](surface-incident-mobility.md) retains its negative front-placement result. This increment checks local integration, cap/stock bounds and frozen weak work; it does not claim an improved evolving front or select a seasonal/desktop model.

## Research and selected formulation

The [official FEniCS nonlinear Poisson example](https://olddocs.fenicsproject.org/dolfin/2016.1.0/python/demo/documented/nonlinear-poisson/python/documentation.html) forms the weak operator by integrating the solution-dependent coefficient with trial/test gradients. It supports element integration rather than sampling a nonlinear coefficient only on an edge. Its equation, boundary conditions and Newton solver are not a dry-water algorithm and are not copied into this project.

For affine P1 depth on a chordal triangle, the gradient is constant while depth is not. Consequently, its local stiffness uses the triangle-average coefficient:

```text
h_T = lambda_0*h_0 + lambda_1*h_1 + lambda_2*h_2
g_T = |grad_T h_T|
beta(h,g) = min(Dmax, h^(5/3)/(n*sqrt(g)))
bar_beta_T = (1/area_T) integral_T beta(h_T,g_T) dArea
C_ij = c_left*bar_beta_left + c_right*bar_beta_right
request_ij = rho*dt*C_ij*abs(h_i-h_j)
```

Each `c_T` is the existing opposite half-cotangent. The implementation evaluates a bounded weighted mean and multiplies by the unchanged assembled weight; this is algebraically the expression above. The relationship follows from constant local P1 basis gradients, not from an externally supplied planetary water solver. The actual spherical region areas remain the stock measure. This is a chordal weak operator with spherical storage, not a claim to reproduce a standard planar FEM mass matrix or a proved spherical nonlinear convergence theorem.

## Positive quadrature and explicit cap intersections

[John Burkardt's triangle geometry lecture, section 14](https://people.sc.fsu.edu/~jburkardt/classes/math1800_2023/geometry2/geometry2.pdf#page=11) supplies a six-point degree-four triangle rule. The two symmetric barycentric orbits are `(a,b,b)` and `(c,d,d)`, with `a=0.816847572980459`, `b=(1-a)/2`, `c=0.108103018168070`, `d=(1-c)/2`. Per-node average weights are `u=0.109951743655322` and `v=0.223381589678011`. Nodes are interior and weights positive. The implementation combines each orbit's three values, then uses `v/(u+v)` to normalize the rounded printed weights explicitly; it is not a reference-triangle integral with total weight one half. No library or dependency is introduced.

The [University of Padua quadrature catalogue](https://www.math.unipd.it/~alvise/SETS_CUBATURE_TRIANGLE/rules_triangle.html) distinguishes weight normalization and lists rules with negative weights or exterior nodes. Higher polynomial degree alone is not the selection criterion. Independent polynomial moments through degree four certify the selected scalar-depth rule; the nonlinear fractional power itself is not integrated exactly.

Midpoint subdivision gives four equal-area subtriangles per level. The bounded parameter is 0–3, with 6/24/96/384 nodes on an uncapped triangle. A cap-crossing triangle needs at most twice those nodes. Constant coefficients and certified whole-triangle caps return unchanged, without a rounded sum.

Unsplit nodes can all lie above a tiny uncapped corner, giving identical capped values at several refinements despite a nonzero exact discrepancy. Therefore integration splits at the affine level

```text
h_cap = (Dmax*n*sqrt(g_T))^(3/5)
```

Sort vertex depths `a<=b<=c`. If `a<h_cap<=b`, the uncapped part is a triangle with depths `(a,h_cap,h_cap)` and area fraction `(h_cap-a)/(b-a) * (h_cap-a)/(c-a)`. If `b<h_cap<c`, the uncapped quadrilateral is split into two triangles with depths `(a,b,h_cap)` and `(a,h_cap,h_cap)`. Their area fractions are `(h_cap-b)/(c-b)` and `(h_cap-a)/(c-a) * (c-h_cap)/(c-b)`. Remaining area is capped and integrated exactly. Nested convex arithmetic represents these fractions without clipping or post-hoc renormalization. This geometry is a project derivation and has independent integral controls for both configurations, repeated vertices and a cap passing the middle vertex.

All-dry triangles have exactly zero mobility. Positive depth with zero gradient uses the capped constitutive limit; zero head differences still produce no transfer. No wet film, slope floor, dry cutoff, fitted coefficient, inventory rescaling or tolerance relaxation is introduced. Invalid/nonfinite parameters, quadrature levels above three and unrepresentable numerators, denominators or cap intersections refuse explicitly. Finite inputs at extreme floating-point limits are not universally supported. The law's cap is applied pointwise before integration, not to an uncapped mean.

## Accounting, energy and isolation

Individual local half-cotangents must be finite and strictly positive. Assembled positivity alone is insufficient; signed local contributions are not clipped. Positive area/quadrature weights give `0<=bar_beta_T<=Dmax`, hence `0<=C_ij<=Dmax*w_ij`. On the admitted flat terrestrial geometry, the inherited `0.45*min(area/(Dmax*sum w))` step bound limits exact-arithmetic outgoing requests to 45% of the frozen leading donor stock. Existing paired owners, canonical exchange order, directed histories and numerical retention remain in use.

The local identity is

```text
sum_edges C_ij*(h_i-h_j)^2
    = sum_triangles area_T*bar_beta_T*|grad_T h_T|^2
```

The diagnostic independently reconstructs triangle energy from gradient basis/area, graph work from completed directed histories, and stock work from owner changes. It refuses discrepancies above the existing `128*epsilon` weak-work criterion. Local/global stock audits retain their existing tolerances. These checks establish algebra/accounting for completed frozen exchanges, not front accuracy. Edge histories remain weak numerical adjacency exchanges, not literal dual-face discharges.

Preparation borrows immutable geometry and reuses the centered wrapper's flat-land/no-body preflight. Malformed stocks/histories, invalid intervals and excessive substep work refuse before mutation in directed tests. This is not arbitrary downstream in-place rollback. Private report state is published only after the entire cohort succeeds. The earlier stage arithmetic, references, version pins, save schemas and desktop selectors are unchanged.

## Bounded verification

Analytic local controls use `(0,0,H)` and `(0,H,H)`, heights 0.001/1/1000 m, cap ratios `1e-6`, `0.2`, `2`, and four quadrature levels. For an uncapped unit-height coefficient they have exact means `9/44` and `6/11`. Capped controls integrate the independent affine-height density; very small capped corners cannot be silently declared fully capped. General three-distinct-depth controls test both cap-intersection geometries against analytic antiderivatives. Directed completed-stage checks cover rest, zero cap, frozen one-ring dry arrivals, common `1e15 m` bed datum, donor bounds, repetition and unchanged fully wet capped transfers.

The frozen spherical report retains 96 cases: two unchanged smooth wet profiles, two orientations, two caps, four meshes (levels 2–5), and three quadrature levels (0–2), each taking one 0.01-second step. Eight level-3 half-step controls check temporal independence of frozen work. The 72 local analytic controls are separate from spherical evolution. Initial stocks use unchanged point depths and actual spherical areas without rescaling; all initial/final owners, directed histories, resolutions and audit scalars are retained. The independent continuum Simpson references and exact capped harmonic certificates remain unchanged.

```bash
cargo run --release --locked --manifest-path native/Cargo.toml --example surface_element_report -- artifacts/new-element-work.json
cargo test --locked --manifest-path native/Cargo.toml --lib element
```

The CLI requires exactly one new output path and refuses overwriting; `surface-element-work-report-1` is not a checkpoint. Frozen smooth work and analytic dry-triangle tests do not substitute for the fixed evolving dry/wet comparisons. Those comparisons, explicit quadrature uncertainty/cost, then terrain lake-at-rest/disconnected-water/barrier controls remain gates before any seasonal integration.

## Results

All 96 frozen cases, eight half-step controls and 72 local analytic controls complete. The full 202,665,162-byte report repeats byte-for-byte. On the finest 10,242-region mesh, nonlinear frozen work discrepancies are:

| Profile / axis | Previous full-gradient source-depth work discrepancy | Element-integrated work discrepancy |
| --- | ---: | ---: |
| P1 / x | +0.199464% | -0.029963% |
| P1 / tilted | +0.201539% | -0.029857% |
| P2 / x | +0.269518% | -0.055571% |
| P2 / tilted | +0.273520% | -0.056894% |

This comparison is with the retained full-gradient frozen-work report, not an invented frozen incident-stage baseline. All eight profile/orientation/cap groups decrease in absolute work discrepancy on every measured spatial refinement. Final ratios are 0.2493–0.2504. Capped controls are unchanged, not credited as a new improvement. These bounded smooth measurements are not a universal convergence proof or a prediction of dry-front displacement.

The maximum relative smooth-work difference from quadrature level 0 to 1 is `2.729e-14`, and from 1 to 2 is `4.069e-16`. All eight half-step work-rate differences are exactly zero in this build. The independent continuum reference's last quadrature difference is at most `2.622e-8`. Smooth quadrature differences near roundoff do not certify singular dry triangles: the local controls separately retain a worst relative error of `3.869e-6` at level 3, including the one-wet uncapped corner. That is a measured local error, not a global front-error bound. No quadrature level is selected for an evolving front based only on these wet results.

All 104 complete initial owner arrays and continuum-reference values match the retained source-depth controls. All 52 fully capped final stock/history/resolution arrays match them exactly. Maximum relative triangle/graph and stock/graph work differences are `2.162e-16`/`2.715e-16`; maximum local/global relative inventory residuals are `1.255e-23`/`1.031e-17`. There are 810 attempted numerical deferrals, with maximum single request `2.179e-10 kg` and maximum per-run summed attempt counter `5.324e-9 kg`. They are unchanged-donor numerical attempts, not completed transfers or lost mass.

The rebuilt historic 66,531,115-byte constitutive witness and 5,272,305-byte legacy seasonal report retain their hashes. A complete evolving dry/wet cohort is not rerun or relabelled as qualified here. The [retained scalar record](data/surface-element-validation.json) contains column mappings, every scalar case/control, independent comparisons and complete report/source fingerprints. Full owner/history arrays remain in ignored reports. No product selector, save or renderer changes are included.

Eleven new directed tests pass. Final `npm test` passes 587 native tests across 108 targets and all eighteen Node files, with the same one pre-existing ignored annual-snow test. Final release regression passes 132 library, twelve regional-soil and five regional-surface tests. Strict all-target Clippy, formatting, whitespace and three CLI argument/overwrite refusals pass. Invalid output remains absent and the existing output unchanged. Owner renderer files remain untouched and excluded; no GUI rerun or evolving dry/wet qualification is claimed by this increment.
