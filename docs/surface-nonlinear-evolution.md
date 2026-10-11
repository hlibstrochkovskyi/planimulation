# Independent nonlinear wet evolution verification

October 11, 2026: the standalone `surface-nonlinear-evolution-report-1` compares the [full-gradient candidate](surface-full-gradient-candidate.md) with a separately implemented latitude-band reference over time. This advances beyond the preceding frozen weak-work screen. It does not select a new seasonal model, migrate a checkpoint, change rendering or qualify moving wet/dry fronts.

## Physical hypothesis and independent discretization

Keep the previously declared capped diffusive-wave hypothesis. [HEC-RAS](https://www.hec.usace.army.mil/confluence/hecras/beta/technical-reference/hydraulic-equations/diffusive-wave-equation) supplies the gravity/friction balance and full-gradient mobility; replacing hydraulic radius by depth and imposing our cap remain project approximations. This law omits inertia, wind and Coriolis. It is not a dam-break, ocean-wave or universally valid flow model.

For a flat sphere and an axisymmetric depth, let `p = axis . unit_position`. Spherical area is `2*pi*R^2*dp`, and the unforced equation reduces to:

```text
dh/dt = R^-2 * d_p [beta * (1-p^2) * d_p h]
beta = min(Dmax, h^(5/3) / (n * sqrt(sqrt(1-p^2)*abs(d_p h)/R)))
```

Use closed fluxes at `p=-1,+1`. Both reference and candidate use `R=100,000 m`, `n=0.04 s/m^(1/3)`, `Dmax=1e6 m^2/s`, and evolve for 1,800 seconds. Initial depths are `1+0.1*P_l(p)` metres for degrees 1 and 2; they stay strictly positive. These are numerical verification settings, not calibrated terrestrial parameters.

The independent reference uses equal-area bands, centered face depths and backward Euler. Damped Picard iteration freezes coefficients, solves a tridiagonal M-matrix and relaxes by 0.5. The nonlinear residual is rebuilt at the proposed state, rather than evaluated with stale frozen coefficients. Iterations stop at an absolute depth-equation residual of `1e-12 m`; a 256-iteration limit refuses incomplete results. [Langtangen's nonlinear-equation notes, section 3.2](https://hplgit.github.io/num-methods-for-PDEs/doc/pub/nonlin/pdf/nonlin-4print-A4-2up.pdf) describe the backward-Euler/Picard approach. The particular spherical band discretization and stopping controls here are independently implemented project choices.

Reference code imports no native surface mesh, cotangent weights, triangle reconstruction, stock histories or native solution. Only the compensated scalar summation helper is shared. The reference is **another numerical approximation**, not an exact nonlinear solution or an external solver's certification.

## Reference certificates and uncertainty controls

The tridiagonal solver is checked against a three-band system with independent dyadic solution `[5/8,1/4,1/8]`. Polynomial integration independently checks exact initial band averages. Zero mobility gives exact rest. Repeated nonlinear runs are identical and audit inventory without clipping or mass rescaling.

Fully capped modes provide exact independent certificates: the spherical Laplacian eigenvalue is `-l*(l+1)/R^2`, so the continuous solution decays by `exp(-l*(l+1)*Dmax*t/R^2)`. See the [University of Toronto spherical-harmonic derivation](https://www.math.utoronto.ca/courses/mat394h1/20139/L30.html). For these two band-averaged profiles the discrete spatial operator also has that eigenvalue; tests check both the exact backward-Euler factor and convergence toward the continuous decay as time steps halve. These certificates validate the linear branch, not the nonlinear answer by themselves.

Each nonlinear profile retains eight reference controls:

| Bands | Time step, seconds |
| ---: | ---: |
| 512 | 1 |
| 1,024 | 4, 2, 1 |
| 2,048 | 1, 0.5 |
| 4,096 | 0.5, 0.25 |

The finest control is sampled by linear interpolation/extrapolation of band-center averages. Native initial stocks instead use continuous point samples times actual spherical areas, without global mass fitting. The footprint difference and pole extrapolation are therefore part of reference spatial uncertainty. Equal-area sample comparisons include both poles; additional differences are measured at the actual native centers with their actual area weights. Refinement differences are empirical uncertainty indicators, **not rigorous error bounds**. Maximum errors can be more pole-sensitive than area-weighted RMS.

The reference has its own scalar inventory-drift guard of `1e-9`; this does not widen native local/global audit tolerances. Every reference state must remain in its certified wet depth range. Any nonconvergence or failed inventory/depth check aborts report construction before output creation.

## Evolving native cohort

Two profiles, two axes (`[1,0,0]` and normalized `[1,2,3]`), three meshes (162, 642 and 2,562 regions), and three common time cadences give 36 candidate trajectories. All start from the same continuous problem, not different generated terrains. The finest-mesh positivity bound sets 425, 850 and 1,700 steps across every mesh: approximately 4.2353, 2.1176 and 1.0588 seconds. Every call performs one stable frozen stage.

Four matched finest-mesh controls use the unchanged old directional-mobility branch. They share geometry, initial stocks, elapsed time, cadence and reference. This isolates the coefficient change rather than comparing unrelated worlds.

All trajectories keep initial/final paired liquid owners, directed cumulative transfer pairs, sampled reference depths and signed endpoint errors. Independent local/global stock-history identities run every 64 steps and at the endpoint. Native numerical retention remains reported; it is not an omitted transfer or budget repair. Edge histories still mean weak-form adjacency exchanges, not literal barycentric-face discharge.

## Results and reproduction

At the finest native cadence, endpoint area-weighted RMS errors are:

| Profile / axis | 162 regions, m | 642 regions, m | 2,562 regions, m | Old branch at 2,562, m |
| --- | ---: | ---: | ---: | ---: |
| P1 / x | 4.96230e-5 | 9.93331e-6 | 3.24684e-6 | 1.61616e-4 |
| P1 / tilted | 2.74149e-5 | 9.91240e-6 | 3.26891e-6 | 1.66017e-4 |
| P2 / x | 2.04648e-4 | 3.37599e-5 | 1.13458e-5 | 2.75015e-4 |
| P2 / tilted | 1.25567e-4 | 3.61230e-5 | 1.10974e-5 | 2.53640e-4 |

All eight spatial comparisons decrease; the final refinement ratios are 0.307–0.336. All twelve successive native time-difference ratios are 0.499734–0.499904. The last time difference is at most 0.328% of the endpoint RMS discrepancy. The matched old-to-candidate RMS reductions are 22.86–50.79 times. At the finest mesh/cadence, error is 0.548–0.552% of the reference's RMS depth change for P1 and 0.939–0.960% for P2. Normalizing against the total metre of standing water would conceal the relevant evolution scale.

Reference refinement matters: the initial six-control pilot's last spatial difference reached about 8–10% of finest native RMS error. The final 4,096-band controls reduce that indicator to at most **3.372%** across all candidate runs; the last reference time difference is at most 0.0782%. On the separate 4,097-point sample grid, the last reference spatial RMS differences are `9.074e-8 m`/`3.027e-7 m`, but pole-sensitive maximum differences are `3.862e-6 m`/`1.214e-5 m`. The RMS result is stronger than a pointwise accuracy claim. No universal order, error bound or nonlinear analytic solution is inferred from these four smooth problems and three meshes.

All sixteen reference runs converge, with at most 56 Picard iterations in a step, maximum accepted nonlinear residual below `1e-12 m` and maximum scalar relative inventory drift `9.238e-14`. Native maximum local/global relative stock-history residuals are `1.737e-18`/`2.024e-17`, under unchanged audit limits. Across the forty native trajectories, 102,358 microscopic attempted requests are explicitly retained by the existing numerical policy, all in the eighteen x-axis candidate runs; maximum single/summed-per-run attempted amounts are `6.905e-8 kg`/`7.992e-5 kg`. Tilted candidate and all legacy controls have no deferrals. These are unchanged donor stocks and recorded attempts, not discarded water, completed transfers or a new accumulation reservoir.

The [retained scalar record](data/surface-evolution-validation.json) keeps every case, reference diagnostics, refinement comparisons and source/full-report fingerprints. Complete arrays remain in ignored `artifacts/`, not seasonal save formats. This closes a bounded **smooth fully wet RMS-evolution check**, not sharp-head, shoreline or general hydrological accuracy.

The complete 43,215,413-byte report repeats byte-for-byte. Rebuilt historic constitutive and active-flow legacy seasonal reports remain identical to their retained 66,531,115-byte and 5,272,305-byte witnesses. Nine new directed tests cover reference certificates, bounded input refusal and repeated evolving native accounting. Release regression passes all 99 library, twelve regional-soil and five regional-surface tests; warnings-denied all-target Clippy, formatting and whitespace checks pass. Missing/extra CLI arguments, invalid options and overwrite attempts in both modes refuse; invalid outputs remain absent and existing report hashes unchanged. No GUI rerun is claimed for this standalone headless verification. The owner's renderer edits remain untouched and excluded.

Full `npm test` passes 554 native tests across 104 targets and all 18 Node files. The same one pre-existing ignored annual-snow qualification remains ignored; it is not counted as a pass or silently enabled for this unrelated increment.

```bash
cargo run --release --locked --manifest-path native/Cargo.toml --example surface_evolution_report -- artifacts/new-evolution.json
cargo run --release --locked --manifest-path native/Cargo.toml --example surface_evolution_report -- artifacts/new-zonal-reference.json --reference-only
cargo test --locked --manifest-path native/Cargo.toml --lib surface_evolution
```

Both CLI modes require a new output path and refuse overwrite. Reference-only output has its own `surface-zonal-reference-report-1` pin and cannot constitute a native accuracy pass. The candidate retains `regional-surface-full-gradient-candidate-1`; no physical stage or production version is changed by observation.

Follow-up: [moving wet/dry-front verification](surface-dry-front-verification.md) adds an exact planar reference certificate and a dry-cap spherical cohort. Its retained source-depth discrepancy motivates the subsequent [flat centered-depth comparison](surface-centered-depth.md), which improves finest dry/wet errors while keeping this independent reference unchanged. Residual sharp-front gradient effects and sharp-bed/barrier controls remain before seasonal selection/persistence; complete moving-front accuracy is not qualified. Fixed coastal heads, long-run drift, soil/cold coupling, generated-world qualification and calibration remain separate gates; milestones C/D and ecology readiness are not completed by these bounded comparisons.
