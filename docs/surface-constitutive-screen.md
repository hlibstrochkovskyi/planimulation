# Nonlinear surface constitutive-law screen

October 11, 2026: the headless `surface-constitutive-work-report-1` harness screens the **unchanged** paired surface stage with cotangent conductances. The nonlinear isotropic-reference gate is **not passed**. This is a prerequisite to defining a credible sharp-head/dry-front refinement problem, not another generated-world endurance run or a replacement simulation model. Defaults, equations, seasonal pins/checkpoints, desktop protocol and owner renderer edits are unchanged.

## Why this precedes front qualification

The [dense seasonal controls](cotangent-dense-study.md) establish reproducibility and bounded cadence behavior, while [smooth capped controls](surface-cotan-candidate.md) qualify a linear regime. Neither establishes the nonlinear continuum law. The inherited mobility divides by the square root of the **directional edge slope**. In contrast, [HEC-RAS's diffusive-wave reference](https://www.hec.usace.army.mil/confluence/hecras/beta/technical-reference/hydraulic-equations/diffusive-wave-equation) uses the magnitude of the water-surface gradient. This difference must be examined before choosing an isotropic PDE front as an implementation's reference.

The selected screening reference, with hydraulic radius approximated by depth, is:

```text
q = -beta * grad(h)
beta = min(Dmax, h^(5/3) / (n * sqrt(|grad(h)|)))
dh/dt = div(beta * grad(h))
```

This is an explicitly declared verification target, not an agreed complete planetary law, a HEC-RAS implementation or a scientifically validated shallow-water model. The cap is our regularization; physical mobility calibration and momentum-dominated flows remain outside this target.

## Fixed input and independent weak reference

All cases use a closed flat sphere of radius 100,000 m, positive terrestrial depths `h(s)=1+0.1*dot(axis,s)` metres, roughness 0.04, the existing spherical column areas and cotangent weights. Two axes, four meshes (162–10,242 regions), two caps (100 and 1e6 m²/s) and two caller intervals (0.01/0.005 s) give 32 cases. Every case starts afresh, executes exactly one stable frozen production substep and retains complete paired owners and directed histories. There is no atmosphere, soil, body contact, forcing, dry cell or evolving front. Point samples are multiplied by actual areas without global mass rescaling. The 0.9 m minimum depth exceeds the maximum possible 0.2 m head difference, so flat-bed drive clipping cannot explain this witness.

For a closed sphere, multiply the reference PDE by the **initial** h and integrate by parts. The resulting weak-work functional is:

```text
I = integral(beta * |grad(h)|^2 dArea)
  = 2*pi*A^2 * integral[-1,1](beta(p)*(1-p^2) dp)
p = dot(axis,s), |grad(h)| = A/R * sqrt(1-p^2)
```

Independent composite Simpson quadrature uses 16,384/32,768 intervals and refuses unresolved differences above 1e-7 relative. A 65,536-interval test checks further refinement. For the fully capped control the exact result is `8*pi*Dmax*A^2/3`, independently checking the quadrature and gradient/area normalization. At zero gradient the capped integrand is explicitly zero. The reference does not sum mesh edges, fit a scalar coefficient, or advance a substitute discrete law.

Measured discrete work uses **completed** signed high/low histories: sum the net adjacency volume rate times its initial head difference. Independent reconstruction sums initial head times each column's actual mass loss. Both have units m⁴/s. Their agreement is checked alongside the existing independent local/global inventory audit and normalization/positivity checks. This is a frozen weak-action measurement, not a finite-interval energy balance, instantaneous physical barycentric-face discharge, a river-flow estimate or a claimed conserved energy store. The initial h is a test function; later squared-depth energy change is not substituted for it.

## Retained result

The [scalar validation record](data/surface-constitutive-validation.json) retains every case, source hashes and complete report fingerprints. At the 0.01 s interval:

| Regions | Fully capped signed work discrepancy | Nonlinear discrepancy, x axis | Nonlinear discrepancy, tilted axis |
| ---: | ---: | ---: | ---: |
| 162 | -1.882182% | +11.526384% | +10.699166% |
| 642 | -0.476493% | +12.028263% | +11.158678% |
| 2,562 | -0.119499% | +11.901117% | +11.021410% |
| 10,242 | -0.029898% | +11.744705% | +10.861773% |

The certified capped control refines approximately fourfold per mesh refinement, independently supporting the geometric operator and reference normalization. The nonlinear discrepancy persists above 10% on the finest grid. Halving the caller interval leaves completed work per second unchanged at reported precision; this is expected for a frozen single stage, **not temporal convergence of an evolving PDE**. Nonlinear quadrature differences are only 6.6358e-9 relative. Tilted-axis cases have no numerical deferrals and still show the discrepancy; donor retention is therefore not its sole explanation. Different axis results also expose orientation sensitivity against a rotationally invariant reference.

These measurements establish a failed necessary weak-work screen for the chosen isotropic law on these four meshes, not a universal error bound, an 11% endpoint-depth/river-discharge error, or a proof of the exact limiting graph PDE. Directional versus full-gradient slope is a concrete consistency concern; upwind depth and chordal-versus-spherical approximation also contribute finite-grid errors. A scalar correction fitted to this one functional would not fix the formulation or establish front accuracy.

There is also an independent local explanation on an **ideal planar equilateral lattice**, away from the cap and dry cells. At a point with positive depth and a smooth nonzero gradient, freeze the leading depth factor in the local refinement limit. For gradient angle theta, edge mobility divides by `sqrt(|grad(h)|*|cos(theta-k*pi/3)|)`. Normalizing its cotangent weak work by the isotropic continuum work gives:

```text
F(theta) = (2/3) * sum[k=0,1,2](|cos(theta-k*pi/3)|^(3/2))
F(0) = 1.138071..., F(pi/6) = 1.074570...
```

The prefactor follows from edge weight `1/sqrt(3)` and area per vertex `sqrt(3)*edge_length^2/2`. A zero directional slope contributes zero work; no division by zero is needed. This derived illustration shows why directional mobility can retain excess work and orientation dependence even when the linear stiffness is consistent. It is not a fitted correction, an exact native spherical-lattice limit, or a quantitative explanation of every measured finite-grid contribution.

The complete 66,531,115-byte report repeats byte-for-byte in the final build. The legacy twelve-case one-day report, including three actively flowing funded branches, stays byte-identical to its historic 5,272,305-byte artifact. Independent maximum local/global relative inventory residuals are `1.2019e-23` and `7.0978e-18`; the graph/stock work identity differs by at most `2.1611e-16` relative. The maximum single/summed deferred requests are only `8.6419e-10`/`8.1363e-9` kg. These tiny counters are not lost stocks or general physical-error bounds.

Validation: full `npm test` passes 535 native tests and all 18 Node files, with the same one pre-existing ignored annual-snow test. All five new library checks pass in debug and release; release regression passes all 80 library and twelve regional-soil tests. Warnings-denied all-target Clippy, formatting and whitespace checks pass. Missing/extra CLI arguments and overwrite controls refuse without creating an invalid output or changing existing bytes. No GUI rerun is claimed; the owner's renderer edits stay excluded.

## Reproduction and next gate

```bash
cargo run --release --locked --manifest-path native/Cargo.toml --example surface_constitutive_report -- artifacts/new-constitutive-work.json
cargo test --locked --manifest-path native/Cargo.toml --lib surface_constitutive
cargo test --release --locked --manifest-path native/Cargo.toml --lib surface_constitutive
```

The CLI requires one new output path, refuses overwrite with exclusive final creation and aborts rather than publishing an incomplete cohort. Full paired arrays belong in ignored `artifacts/`; the retained record omits them but fingerprints complete files. Passing the nonlinear **witness test** preserves the failure; it does not turn the physical gate into a success.

The next candidate must define and verify full tangent-gradient reconstruction and a conservative positive bounded mobility on the unchanged mesh. It needs separate version selection and must retain this old witness; no existing save/default may silently receive a new law. Verify smooth nonlinear weak action and additional orientations/fields before evolving sharp/dry fronts. Crest/downhill clipping, zero-gradient regularization, dry limiting behavior, fixed reference heads and soil/cold calibration remain separate gates. An elementwise coefficient or reconstructed face coefficient is a proposal, not implemented functionality. No ecology readiness or milestone C/D completion follows.
