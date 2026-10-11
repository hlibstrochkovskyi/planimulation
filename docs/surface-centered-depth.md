# Flat centered-depth comparison

October 11, 2026: the standalone `regional-surface-flat-centered-depth-candidate-1` changes only the interface depth used by the [full-gradient candidate](surface-full-gradient-candidate.md). It is a controlled comparison on flat, closed, terrestrial geometry, not a variable-bed flow scheme, seasonal model or new desktop mode. Its results must be read against both [smooth wet evolution](surface-nonlinear-evolution.md) and the retained [dry-front discrepancy](surface-dry-front-verification.md).

## Question, sources and isolation

The preceding dry-cap problem conserves water but its low-depth contours advance too far. Time-step refinement does not remove that discrepancy. Test whether using source-side depth in nonlinear mobility contributes to the spatial error, while keeping the continuous problem and independent reference unchanged.

[Park, Kim and Kim (2019), section 2.3](https://pdfs.semanticscholar.org/35cc/bfdaf421be2b46bea49e0d8cc57ff0e5ee9c.pdf#page=5) reconstruct interface values with MUSCL and a van Leer limiter, combine them in a Lax–Friedrichs flux and use second-order Runge–Kutta. They also stop velocity below a declared dry-depth threshold. This supports investigating interface reconstruction, but it is **not** our arithmetic-mean scheme or a proof of its accuracy. Their Cartesian algorithm, rainfall-dependent roughness and dry threshold are not copied into this comparison.

Keep the [HEC-RAS diffusive-wave hypothesis](https://www.hec.usace.army.mil/confluence/hecras/beta/technical-reference/hydraulic-equations/diffusive-wave-equation), the project's hydraulic-radius approximation and explicit cap. This remains gravity/friction-dominated, inertia-free flow, not shallow-water dam-break or ocean dynamics. An implicit solver or limiter-based higher-order reconstruction would change more than the factor isolated here and needs separate verification.

On a flat downhill pair:

```text
h_face = 0.5*h_source + 0.5*h_target
beta = min(Dmax, h_face^(5/3) / (n*sqrt(full_gradient_magnitude)))
request = rho*dt*beta*cotangent_weight*(h_source-h_target)
```

Only `h_face` changes. Retain actual spherical areas, chordal full-gradient stencils, averaging gradient magnitudes, the cap, roughness, canonical edge order, frozen leading depths, paired owners, directed histories, numerical retention and independent audits. No mass rescaling, floor, film, dry-depth cutoff or fitted coefficient is introduced. At an exactly wet/dry pair, uncapped mobility is `2^(-5/3)` times the source-depth value; this is an algebraic coefficient ratio, not a predicted trajectory-error reduction. A directed test measures that ratio using completed transfers rather than a second implementation of the formula.

Because `0 <= h_face <= h_source` and `drive <= h_source`, capped mobility retains the existing positive-weight outgoing-stock bound on the admitted geometry. This does not establish an error bound or a safe variable-bed extension. Fully capped frozen dry arrivals match the source-depth stage exactly; a smaller source coefficient alone does not imply that a nonlinear evolving case will improve.

## Narrow implementation contract

The prepared wrapper borrows immutable full-gradient geometry and refuses any varying bed or non-terrestrial region. Common constant bed offsets are allowed and separately tested at `1e15 m`; no offset is added to shallow depth before differencing. Calls refuse body owners, region-to-body assignments, malformed stock/history shapes, invalid paired owners, nonfinite/negative intervals and excessive substep work. Directed refusal tests check that liquid, bodies, histories and resolution accounting remain unchanged for these inputs. This is not a general guarantee that arbitrary downstream failures roll back an in-place caller; the report owns private state and publishes only after successful computation.

The shared paired-stage implementation adds an explicitly selected interface-depth branch. Historic entry points always select source depth with the same arithmetic. A verification-only enum selects report trajectories; no seasonal enum, checkpoint, protocol or renderer can select this experiment. Candidate/report pins are distinct from both historic reports.

## Fixed comparisons

The dry report reuses the exact planar certificate and all eight spherical reference controls **unchanged**. It keeps the same dry-cap initial function, radius, cap, roughness, duration, orientations, three native meshes and three common time cadences. Eighteen centered trajectories are accompanied by two finest-mesh source-depth controls. All initial/final owners, directed histories, sampled reference depths and endpoint errors are retained.

The wet report retains both positive profiles, both orientations, three meshes, three common cadences and all sixteen wet reference controls. Thirty-six centered trajectories are accompanied by four matched source-depth controls. Reference interpolation and uncertainty indicators are unchanged. Do not select a reference/control because it minimizes candidate error.

Threshold contours remain graph-edge proxies at 1 mm, 1 cm and 5 cm, not exact PDE support or exact reconstructed flooded area. Native point initialization is not rescaled to the reference's band inventory; quadrature discrepancy and initially dry-half inventory remain separate measurements. Green conservation tests cannot substitute for these accuracy comparisons.

## Dry-front result

At the finest cadence:

| Regions | Centered RMS error, x / tilted, mm | Centered maximum 1 cm contour error, x / tilted, m |
| ---: | ---: | ---: |
| 642 | 24.729 / 15.018 | 128.74 / 145.99 |
| 2,562 | 13.658 / 8.712 | 62.27 / 69.37 |
| 10,242 | 6.825 / 4.834 | 25.29 / 30.75 |

At 10,242 regions, source-depth RMS errors were 15.330/16.340 mm, and maximum 1 cm contour errors were 70.47/73.09 m. Thus finest RMS decreases by 55.48%/70.41%, and the 1 cm proxy error by 64.11%/57.93%. The continuous reference's 1 cm contour still advances 152.48 m. All four spatial RMS comparisons and all threshold-contour maximum comparisons improve with refinement, but centering does not eliminate the discrepancy.

Finest maximum errors at 1 mm are 46.63/52.20 m, versus 95.92/97.95 m before centering. At 5 cm they are 5.85/8.99 m, versus 41.29/44.02 m. Some 5 cm crossings now lie slightly behind the reference while others remain ahead; a universal statement that the whole interface is too fast is no longer justified. Threshold dependence matters, and exact support remains unqualified.

The final spatial RMS ratios are 0.500/0.555. All six successive time-difference ratios are 0.499643–0.499941; the last time difference is at most 0.168% of endpoint RMS error. Last reference spatial/time RMS differences reach 2.557%/2.349% of candidate discrepancy, with the unchanged sub-0.3 m final reference contour differences. These empirical indicators are much smaller than the retained native discrepancy, not proofs of the exact nonlinear solution.

Maximum candidate native local/global relative residuals are `4.364e-16`/`1.374e-17`. The initially dry-half inventory proxy differs from the reference by +8.32%/-0.91% on the finest grid. The inclusive `p<=0` native classification assigns the entire equatorial vertex band to that aggregate, whereas the band reference integrates the negative half; that footprint difference and initial point quadrature remain uncorrected and explicitly recorded. This aggregate is not an exact geometric hemisphere integral or an independent front-accuracy certificate.

All planar/spherical reference controls and matched source-depth trajectories are unchanged. This fixed counterfactual identifies source-side interface depth as a significant contribution to the measured finite-grid discrepancy. It does not establish that depth averaging alone solves wet/dry flow, that the averaging choice is uniquely correct, or that remaining error is entirely due to gradient reconstruction.

## Smooth wet result

At the finest cadence on 2,562 regions, area-weighted RMS errors are:

| Profile / axis | Source depth, micrometres | Centered depth, micrometres |
| --- | ---: | ---: |
| P1 / x | 3.247 | 2.525 |
| P1 / tilted | 3.269 | 2.517 |
| P2 / x | 11.346 | 9.414 |
| P2 / tilted | 11.097 | 9.402 |

Finest reductions are 15.27–23.01%. On the coarsest mesh, RMS instead increases by 1.55–3.82%; not every comparison favors centering. All eight centered spatial-refinement comparisons improve, with final ratios 0.259–0.286. All twelve time-difference ratios remain near one half (0.499734–0.499904); the last difference is at most 0.387% of endpoint RMS discrepancy. Finest errors are 0.4245–0.4259% of actual reference depth change for P1 and 0.7952–0.7966% for P2, not normalized by the standing metre of water.

All wet reference controls and four matched source-depth controls are exactly unchanged. Last reference spatial/time RMS differences reach at most 4.348%/0.0924% of candidate discrepancy. These are empirical uncertainty indicators, not rigorous bounds. Maximum candidate native local/global relative residuals are `1.374e-18`/`2.172e-17` under unchanged audits. Numerical retention remains reported separately from completed transfers. This supports a bounded smooth-wet comparison, not general convergence, calibration or moving-front accuracy by itself.

## Reproduction and limits

```bash
cargo run --release --locked --manifest-path native/Cargo.toml --example surface_centered_report -- artifacts/new-centered-dry.json --dry
cargo run --release --locked --manifest-path native/Cargo.toml --example surface_centered_report -- artifacts/new-centered-wet.json --wet
cargo test --locked --manifest-path native/Cargo.toml --lib surface_centered
```

The CLI accepts exactly a new output path and `--dry` or `--wet`, refuses overwrite and computes the complete report before creating output. Dry/wet report pins are `surface-flat-centered-front-report-1` and `surface-flat-centered-wet-report-1`. Complete arrays belong to ignored `artifacts/`, not seasonal save files.

The [retained scalar record](data/surface-centered-validation.json) keeps all sixty native trajectories, all reference controls, refinement/comparison results, attempted-request accounting and source/report fingerprints. The full 79,548,379-byte dry and 43,185,212-byte wet reports each repeat byte-for-byte. Rebuilt original dry-front, smooth-wet and legacy seasonal reports retain their historic 79,883,936-byte, 43,215,413-byte and 5,272,305-byte hashes. All initial paired owners, reference controls and matched source-depth trajectories are exactly unchanged.

Six new directed tests pass. Full `npm test` passes 566 native tests across 106 targets and all 18 Node files; the same one pre-existing ignored annual-snow test remains ignored. Final release regression passes 111 library, twelve regional-soil and five regional-surface tests. Warnings-denied all-target Clippy, formatting, whitespace and missing/invalid/extra CLI argument and both-mode overwrite checks pass. Invalid output remains absent and existing report hashes unchanged. No production/save pin or desktop contract is promoted by these checks.

Next, keep this witness fixed while isolating full-gradient reconstruction near the sharp wet/dry edge. A separately reviewed variable-bed reconstruction must then test exact partially wet lake-at-rest states, disconnected wet regions and barriers before any seasonal extension. Smaller caller steps do not remove the retained spatial discrepancy; adjusting roughness or the cap to hide it would change the problem rather than diagnose it.

No terrain barriers, disconnected wet regions, finite-body/coastal coupling, soil/cold dynamics, long-run drift, generated-world calibration or seasonal selection are qualified by this flat-only comparison. The owner's renderer edits remain outside this work, and no GUI rerun is claimed.
