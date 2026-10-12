# Dry-front observation audit and parallel method review

October 12, 2026. Three independent research tracks reviewed the contour observable, numerical mathematics, and available implementations. This is a read-only endpoint audit and a proposed next experiment, not a new simulation operator, accuracy qualification, or desktop feature. No physical equation, accounting tolerance, version, save format, or default changes.

## Completed same-mesh observation control

The preceding [element-integrated dry-front study](surface-element-front.md) improved depth RMS but worsened every measured maximum contour-proxy error. Before another evolving cohort, feed its independent reference depths through exactly the existing native graph-edge depth-linear estimator.

Use the six finest-cadence `q=0` endpoints: two orientations, subdivision 3/4/5, and all three thresholds. Export centers, actual physical areas and boundary region pairs using the public native `Surface::build(level, 1000.)` and `moisture_transport::Geometry::from_surface`, not a renderer mesh. Select the retained 4096-band, 0.125-second reference endpoint (spherical control index 7). Independently resample its depths using the existing band interpolation and constant polar extension.

For an edge crossing threshold `d`, the unchanged observer is:

```text
p_i = dot(unit_axis, center_i)
p_cross = p_a + (p_b-p_a) * (d-h_a)/(h_b-h_a)
signed_error_m = 1000 * (acos(clamp(p_cross,-1,1))-acos(reference_p))
```

It locates a threshold on an adjacency edge. It does not reconstruct a physical wet/dry boundary inside a region.

All six independently sampled reference arrays equal the retained arrays exactly. Native depths reconstructed from compensated owner pairs and actual areas reproduce all eighteen element and all eighteen incident threshold-crossing arrays exactly after sorting. All six incident reference-sample arrays also agree exactly. Mesh edge counts equal `3*regions-6`. The main reviewer recompiled the geometry exporter and reran these assertions. No water evolution was rerun.

At 10,242 regions, maximum absolute errors are:

| Threshold | Reference depths through native observer, x / tilted | Element q=0 state through native observer, x / tilted |
| --- | ---: | ---: |
| 1 mm | 32.611 / 37.097 m | 47.506 / 54.736 m |
| 1 cm | 27.382 / 30.829 m | 30.235 / 33.859 m |
| 5 cm | 15.959 / 16.266 m | 9.555 / 11.734 m |

The [scalar evidence record](data/surface-front-observation-audit.json) retains all eighteen cases, signed ranges, unweighted crossing RMS, input/helper/output fingerprints, matched-edge coverage, and endpoint diagnostics. These are crossing statistics, not area-weighted depth RMS. Axis labels `[1,2,3]` denote the normalized tilted axis used by the original run.

The observer produces tens of metres of apparent error even on independently sampled reference depths. This does **not** prove the evolving state accurate. Oracle maxima are not lower bounds on native maxima, and subtracting extrema does not estimate pointwise state error. At 5 cm, the oracle maximum exceeds the native maximum on all six mesh/orientation cases: smoothing can cancel observation bias. Original raw errors and the negative front-remedy decision remain retained.

Matching crossings only on identical graph edges gives finest 1 cm displacements of -1.882 to +7.791 m on 166/366 native x-axis crossings, and -1.847 to +8.802 m on 156/356 native tilted crossings (358 reference crossings). This partial coverage cannot establish whole-boundary error.

At finest 1 cm, 342/366 x-axis and 299/358 tilted reference crossings have an exactly zero endpoint. All six finest threshold maxima involve an exactly zero endpoint. Such an endpoint does not identify where the actual support starts inside the edge. Merely interpolating `h^(7/3)` instead of `h` was also tested as an observer-only control: its finest reference 1 cm maxima worsen to 30.418/37.324 m, and native maxima to 41.475/44.505 m. Do not import that shortcut as a remedy.

## Mathematical lead, not an implemented method

Direct algebra on flat bed with inactive cap gives:

```text
h_t = kappa * Delta_p(h^m)
p = 3/2; m = 13/3; kappa = 1/(n*sqrt(13/3))
v = h^(7/3); K = sqrt(3/7)/n
q = -K*h*|grad(v)|^(-1/2)*grad(v)
```

This is doubly nonlinear diffusion, not ordinary porous-medium diffusion. The exponent agrees with the pressure variable in [Audrito–Vazquez, equation 3.4](https://arxiv.org/html/1601.05718v2). Their reaction-wave speeds are not imported. In our exact planar wave, pressure is affine on the wet side. The original flat cap can be retained as `q=-h*min(3*Dmax/(7*v), K/sqrt(|grad(v)|))*grad(v)` in wet regions, with the original finite zero-gradient/dry limits. Variable bed requires the original full `grad(b+h)` and separate analysis.

The proposed next reconstruction represents a signed latent affine pressure, then uses `h=(v_+)^(3/7)`. Its intercept is solved from the existing local stock integral, rather than globally fitting inventory. Unlike interpolation of already clipped nonnegative samples, it can represent partial wet support. This requires independent slope information, cut integration, physical areas and a reviewed relation between region stocks and reconstructed fields. No positivity, convergence, cost, or spherical accuracy claim is established for this proposal.

The closest direct method lead is [Manorost–Bastian's diffusive-wave preprint](https://arxiv.org/html/2512.16525v1). It covers Manning exponents and full slope magnitude. Its finite-volume nonnegativity result is conditional on nonlinear solvability and its stated mesh/boundary/source assumptions. Its cut-cell DG implementation uses a positive initial film and low-depth flux suppression; its second-order analytical experiment uses ordinary porous-medium parameters, not Manning diffusion. Partial-support geometry is useful inspiration, not a qualified unchanged-law replacement. No public linked implementation or software license was identified in this review; the article's license is not a code license.

## Ready implementation review

Actual constitutive/flux sources were inspected; no external package was built or executed. No compatible ready replacement was identified among these sources. License observations are preliminary provenance notes, not authorization to copy code.

| Source | Relevant difference from this witness |
| --- | --- |
| [Proteus coefficient kernel](https://github.com/erdc/proteus/blob/48f54a60c12b5d7e1456dba69d25800ae73c0444/proteus/transportCoefficients.c#L12007-L12041), [configuration](https://github.com/erdc/proteus/blob/48f54a60c12b5d7e1456dba69d25800ae73c0444/proteus/TransportCoefficients.py#L9703-L9807), [MIT license](https://github.com/erdc/proteus/blob/48f54a60c12b5d7e1456dba69d25800ae73c0444/LICENSE.md) | Manning-shaped exponents with epsilon regularization; no roughness argument or matching mobility cap in that coefficient kernel. Completed-step positivity/accounting needs separate qualification. |
| [ParFlow diffusive kernel](https://github.com/parflow/parflow/blob/954b8bc0d76fe8c4ba144c057e6ed020030f9131/pfsimulator/parflow_lib/overlandflow_eval_diffusive.c), [regression example](https://github.com/parflow/parflow/blob/954b8bc0d76fe8c4ba144c057e6ed020030f9131/test/python/overland_slopingslab_DWE.py), [LGPL license](https://github.com/parflow/parflow/blob/954b8bc0d76fe8c4ba144c057e6ed020030f9131/LICENSE.txt) | Full slope magnitude, but lagged denominator gradient, slope floor and upwind depth differ. Structured surface/subsurface machinery is not a small dependency. |
| [ATS Manning coefficient](https://github.com/amanzi/ats/blob/86114e29df238a800013d5f3318d81a4453fcb38/src/pks/flow/constitutive_relations/overland_conductivity/manning_conductivity_model.cc), [BSD-3 license](https://github.com/amanzi/ats/blob/86114e29df238a800013d5f3318d81a4453fcb38/LICENSE) | Zero coefficient at nonpositive depth, but slope floor and a depth-exponent cap, not our mobility cap. The evolving slope-field provider was not verified. |
| [Basilisk author sandbox](https://basilisk.fr/sandbox/nlemoine/SWE/diffwave/diffwave.c) | Full-slope implicit diffusion, slope epsilon, nonnegative face depth and explicit excess-water compensation after correction. Sandbox redistribution permission was not established. |
| [AdH velocity kernel](https://github.com/erdc/adhv8_kraken/blob/8ddbb4d67913e58cdb7677ef5add743e6999a49c/src/fe/diffusive_wave/fe_diffusive_velocities.c) | Separate signed square roots of gradient components change the law under rotation. No repository license was located in this inspection. |

Existing [Landlab source review](surface-element-front.md#external-implementation-review) remains relevant: a local-inertial solver and thin initial film are different hypotheses, not exact witnesses for this equation.

## Decision and bounded next work

The first interval-only algebra/data control is now completed in [frozen planar reconstruction](surface-planar-reconstruction.md). It does not close the triangle, evolving-front or spherical qualification gates below.

Parallel research is useful for independent observer audits, mathematics and source review. Integration of competing operators remains sequential: each branch must use the same inputs, conservation contract and declared metrics. Running several full cohorts simultaneously can contend for the same CPU and memory; more agents are not automatically more numerical throughput.

1. Preserve the original depth-linear proxy and add observer-aware comparisons with explicit correspondence/coverage. A reference-only rotation/mesh-phase sweep and contours at fixed physical meridians are candidate cheap controls; neither was completed here. Do not silently weaken or replace the existing criterion.
2. Translate the existing exact planar wave through a small set of cells/triangles and orientations. Compare depth-linear, clipped-pressure-linear and stock-constrained signed-pressure reconstruction. Separate exact input/slope checks from any data-derived slope reconstruction. Measure local inventory, threshold/support positions and constitutive flux before evolving water.
3. Only a successful local control leads to short planar evolution at two mesh spacings and two time steps. Retain original-law cap/gradient behavior and paired stocks. A new reconstruction is not automatically a production operator.
4. Keep implicit stepping as a separate cost hypothesis, not a claimed spatial-front cure. Measure nonlinear convergence, tails, physical residuals and conservation before any integration.
5. Do not expand expensive quadrature cohorts now. The existing millimetre-scale contour-extrema changes under quadrature refinement cannot explain the tens-of-metres proxy discrepancy. Exact zero-minimum triangle integration and removal of recursive allocations are possible local optimizations, not demonstrated front remedies or speedups.

## Reproduction scope

The compact record identifies source commit `faedcbcb9f3a4181f72f2c82dfc8812772098676`, both retained full-report hashes, temporary helper hashes and output hashes. Research helpers remain in `/tmp/planimulation-front-observable-z3sL2T/`, not in an installed product or stable report API. For the retained local build:

```bash
rustc --edition 2024 /tmp/planimulation-front-observable-z3sL2T/mesh.rs --extern planimulation_core=native/target/release/deps/libplanimulation_core-6314fcd9f0966ba9.rlib -L dependency=native/target/release/deps -o /tmp/planimulation-front-observable-z3sL2T/mesh
node /tmp/planimulation-front-observable-z3sL2T/audit.mjs
```

Those temporary files and compiled-library filenames are environment-specific and may disappear; the method, scalar evidence and fingerprints are retained here. No full PDE replay, external-solver benchmark, full package regression, or GUI test was rerun for this documentation-only increment. Pre-existing renderer changes are untouched.
