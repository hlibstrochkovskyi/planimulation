# Fixed-input paired surface-flow verification

October 9, 2026: implemented a bounded native verification harness for the **actual** paired surface stage used by `regional-seasonal-water-1`. The measured spatial gate is **not passed**. Accounting and time-step refinement alone do not establish spatial accuracy. No physical equation, default, checkpoint pin, protocol or desktop behavior is changed.

## Shared implementation, independent reference

The paired face stage is extracted without changing its formulas, operation order, frozen leading-component depths, canonical face order, owner mapping, grant checks or retention policy. Seasonal advancement and the verification harness call the same stage and transfer routine. The existing geometry preparation accepts internal physical arrays as well as generated worlds. This does not expose arbitrary terrain as a valid generated-world recipe or seasonal checkpoint.

The harness uses a closed sphere of radius 100,000 m, a flat bed, and exclusively terrestrial columns. There are no reference-body contacts, soil, atmosphere, drainage, external forcing, wetting fronts or shoreline changes. Initial depth is the same continuous function on every mesh:

```text
h(s,0) = H + A dot(axis,s)
H = 100 m, A = 1 m, |s| = |axis| = 1
D = 10,000 m²/s, roughness = 0.04
h(s,t) = H + A exp(-2 D t / R²) dot(axis,s)
```

The angular `l=1` spherical harmonics have eigenvalue `-2`; the radius-scaled surface Laplacian consequently gives the decay above. This reference follows the [University of Toronto spherical-harmonic derivation](https://www.math.utoronto.ca/courses/mat394h1/20139/L30.html). Comparing an implementation with an analytic PDE solution, separately from physical validation, follows [NASA's verification guidance](https://www.grc.nasa.gov/www/wind/valid/tutorial/verassess.html).

The production Manning-inspired law is not generally linear diffusion. This control certifies its **capped** linear regime: flat bed, depths between 99 and 101 m, and a conservative lower bound on uncapped mobility above D. The bound uses the smallest face distance, minimum depth 99 m, and maximum possible head difference 2 m. Every actual substep checks the depth interval, so the certificate remains applicable. No duplicated substitute rate law advances the control.

Initial masses are the continuous depth sampled at region centers, multiplied by existing physical dual-cell areas and water density. They are not exact cell averages; the point-sampling/quadrature error is part of this study. Total mass is not rescaled to force equal initial totals across meshes. No renderer geometry participates.

## Controls and independent audits

Sixteen evolving cases cover subdivisions 2–5 (162, 642, 2,562 and 10,242 regions), two axes (`[1,0,0]` and normalized `[1,2,3]`), and two common steps. Both grids and orientations use the same physical parameters and endpoint of 36,000 s. The finest grid's stability bound determines 546/1,092 steps: approximately 65.934/32.967 s. Each call executes exactly one production surface substep; the common time steps prevent changing spatial resolution from also selecting a different cadence. A separate uniform-depth control uses the finest mesh and 546 steps.

The harness keeps complete paired liquid owners and both directions of every face's gross history. Every substep validates stock normalization/positivity and certified depth bounds. Every 32 steps and at the endpoint, independent graph identities reconstruct each column's initial stock, incoming and outgoing mass. Global accounting counts liquid owners once. Local/global audit tolerances remain `128 * epsilon * max(abs(ledger terms),1 kg)` and `1e-12` of initial mass, respectively. Deferred-request diagnostics remain separate from completed transfers and inventories.

Errors against the analytic solution include area-weighted mean absolute/RMS errors and maximum center error. `rmsErrorOverAnalyticChange` divides by the RMS **change** predicted during this interval, not the much larger 100 m background. Same-mesh time differences use both signed stock components. Successive-grid error ratios and observed exponents based on the square root of region-count ratios are observations, not a Richardson/GCI estimate or an asserted asymptotic order; see [NASA's spatial-convergence discussion](https://www.grc.nasa.gov/www/wind/valid/tutorial/spatconv.html).

A second diagnostic freezes the same initial field and executes the real stage for one second. Its signed net face transfers are compared with an independent, exactly integrated continuum flux over the existing dual boundary:

```text
grad_s(h) = A/R * (axis - dot(axis,s)*s)
outward_face_mass_rate = -rho D A/R * sum_segments(dot(axis,n)*arc_length)
```

Each great-circle segment's conormal n is constant and perpendicular to its radius, making this integral exact for this field. The diagnostic also records the angle between that conormal and the center-connection vector projected into the tangent plane at the segment midpoint. Its temporary one-second owners/histories are audited and **do not modify the study's initial state**. A zero reference-flux or analytic-change denominator is null, never an invented zero relative error.

## Retained findings

The [validation summary](data/surface-spatial-validation.json) retains parameters, all seventeen cases' scalar measurements, temporal/spatial comparisons, uniform control, initial-flux diagnostics, source/report hashes and validation records. The full ignored artifact additionally retains every final liquid/history pair and center error.

| Regions | Fine-step depth RMS error | Error / analytic RMS change | Initial face-flux relative L1 error, x axis | Maximum segment nonorthogonality |
| --- | ---: | ---: | ---: | ---: |
| 162 | 3.815 mm | 9.512% | 4.213% | 10.681° |
| 642 | 3.884 mm | 9.684% | 6.268% | 11.187° |
| 2,562 | 3.702 mm | 9.230% | 7.370% | 11.313° |
| 10,242 | 3.635 mm | 9.063% | 7.933% | 11.344° |

Both orientations have almost identical area-weighted RMS errors, but their individual face-flux errors differ. Finer time steps change the depth field by only about 1.54–1.71 micrometers RMS: roughly 0.04%–0.045% of the analytic error. The spatial error first **increases**, then decreases only weakly. Observed exponents are about -0.026, 0.069 and 0.026. These four meshes do not demonstrate an asymptotic convergent regime, and no positive convergence order is claimed.

The skew segment geometry persists under refinement while the face-flux discrepancy increases. The uncorrected two-point head difference uses the center-to-center direction, not the actual boundary conormal. Together these measurements identify a concrete geometry/flux-consistency concern, not a justification for reducing the time step indefinitely. Nonorthogonal meshes are a known accuracy concern in [NIST's FiPy guidance](https://pages.nist.gov/fipy/en/stable/USAGE.html#meshing-with-gmsh). A specific correction is not established by this diagnosis; point-versus-cell-average representation and reconstruction must also be reviewed.

The tilted-axis trajectories have **no deferred requests**, yet retain the same RMS error. Therefore the measured discrepancy cannot be explained solely by numerical donor retention. Symmetry-related tiny requests do occur in the x-axis cases. The uniform control's maximum depth error is about `1.035e-14 m`: converting rounded mass back to leading-component depth creates microscopic differences, so exact zero gross flow is not assumed. Its completed microflows and numerical deferrals are recorded, not silently removed.

All seventeen cases complete the prescribed endpoint and independent audits. Maximum measured local/global relative residuals are `1.707e-18` and `1.278e-17`. The uniform finest-grid control has about 47.699 kg of gross micro-crossings against approximately `1.257e16 kg` of owned water, and no deferred requests. Gross crossings can repeatedly count the same water. Across the evolving controls, the largest single deferred request is about `1.212e-5 kg`; the largest summed request total is about 0.05649 kg. Neither sum bounds net physical error.

The first draft's exact-equilibrium/no-deferral and monotonic-spatial-refinement assertions failed. Their assumptions were not used to retune the model. Directed tests now preserve the measured nonconvergence witness and distinguish it from temporal effects, verify independently reconstructed accounting and signed tails, and reject balanced stock teleportation. Passing these **witness tests does not mean the spatial gate passes**.

## Reproduction and limits

```bash
cargo run --release --locked --manifest-path native/Cargo.toml --example surface_spatial_report -- artifacts/new-surface-spatial.json
cargo test --locked --manifest-path native/Cargo.toml --lib surface_verification
cargo test --release --locked --manifest-path native/Cargo.toml --lib --test regional_soil --test regional_surface_flow
```

The report takes exactly one new output path and refuses overwrite, including exclusive final creation. Any construction, arithmetic, depth-bound or audit failure aborts report creation with an error rather than publishing an incomplete cohort as successful. The output is a verification report, **not a seasonal checkpoint**. Reproduction is same-build; no cross-platform bitwise promise is made.

The 45,748,412-byte full report repeats byte-for-byte. Repeating all twenty prior dense 30-day cases after extraction preserves their complete 95,360,949-byte report exactly, including native checkpoint components and diagnostics. Six new library tests pass in debug and release; all 67 library, seven unified-cycle and five legacy regional-surface tests pass in release. Full `npm test` passes TypeScript checking, native build, 514 native tests and all eighteen Node test files. The one pre-existing ignored annual-snow test remains unchanged. Warnings-denied all-target Clippy, formatting and diff checks pass. CLI checks confirm existing output is refused without changing its hash and missing arguments are rejected. No GUI test is rerun; the owner's existing renderer camera/style edits remain outside this increment.

The next gate is a separately reviewed consistent surface flux on the physical mesh, with this witness retained. Possible approaches include a compatible multipoint/gradient reconstruction or a separately versioned orthogonal physical dual; neither is implemented here. Any replacement must preserve finite-owner accounting, positivity/work bounds, sill blocking and exact versioned replay, and be tested against this and additional analytic/nonlinear/wetting controls. A scalar coefficient tuned to fit this one harmonic is not a geometric correction.

Existing seasonal/generated-world results remain evidence for their recorded numerical trajectories, not calibrated hydrology. No ecology/population promotion, evolving reference heads, new desktop mode, schema migration or milestone C/D completion follows from this study.
