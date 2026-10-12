# Frozen planar partial-support reconstruction

October 12, 2026. The [parallel method review](surface-front-observation.md) motivates this small native control before another evolving sphere cohort. It measures one-dimensional representations of the existing exact wave, not time evolution, a new physical model, or a production reconstruction. Two independent reviews checked the formulas and input/measurement contracts. Seasonal operators, pins, saves, defaults and desktop behavior are unchanged.

## Inputs and independent comparisons

Keep `c=0.1 m/s`, `n=0.04`, `Dmax=100 m²/s` and pressure slope magnitude `S=(7/3)*(c*n)^2`. In normal coordinates, the exact depth is `[S*(front-x)_+]^(3/7)`. Widths are 4/2/1/0.5 m, phases are 0, `2^-20`, .001, .05, .2, .5, .8, .95 and 1, and directions are reflected signs -1/+1. There are 72 cases, each with support and the original positive thresholds 1 mm, 1 cm and 5 cm.

For each threshold, the exact position selects its containing interval. This deliberately isolates representation error: it is **not** an algorithm for discovering a contour from unknown data. The support control always uses the front interval `[0,width]`. Positive thresholds select their own upstream intervals, including the 5 cm contour approximately 24.67 m behind support; missing front-cell crossings are not assigned zero error.

Three distinct input/representation families are retained:

- Exact endpoint depths drive depth-linear and clipped-pressure-linear **point observers**. These are not conservative regional reconstructions or the original spherical adjacency estimator.
- Exact analytic interval means and the supplied exact slope drive a **known-slope algebra oracle**.
- The same local mean and the pressure-power secant of two fully wet upstream means drive a **data-slope surrogate**. The upstream orientation and fully wet stencil are prescribed in this family. Neither exact speed nor exact derivative supplies this branch's slope; its inputs nevertheless come from the manufactured exact profile.

Exact integrated means are obtained from the existing planar wave primitive, independently of the reconstructed-interval integral. They are not the historic native point-initialized stocks, and do not rescale or repair those stocks. Per-unit-breadth volume is `mean_depth*width`. The distinction between integrated averages and smooth center-value approximations is also explicit in [Clawpack's initialization documentation](https://www.clawpack.org/user_routines.html); a singular front is not a uniformly smooth initialization.

## Local integral and inversion

For a centered interval of length `L`, let `h=(a+s*x)_+^alpha`, `alpha=3/7`, `B=abs(s)*L`, and endpoint pressures `H=a+B/2`, `Z=a-B/2`. Its mean is:

```text
mean = (H_+^(10/7)-Z_+^(10/7))/((10/7)*B), B>0
mean = a_+^(3/7), B=0
```

For a positive partly wet mean, invert directly: `H=((10/7)*B*mean)^(7/10)`. The fully wet branch is monotone and uses bounded bisection. `log1p`/`expm1` avoid cancellation in the fully wet power difference. The fit stores wetmost endpoint pressure `H`, not just center intercept `a`: a tiny positive wet fraction can disappear when subtracting and adding half the much larger cell pressure span. No wet floor is introduced.

The separate local fit residual must be at most `256*epsilon` relative to the positive input mean. This is a reconstruction arithmetic check, not a relaxation of native accounting. Invalid/nonfinite inputs and failed fits refuse. Zero stock returns `zero-stock-underdetermined`: infinitely many latent negative intercepts have the same zero physical stock. A dry interval alone does not locate its support.

Pressure exponent `7/3` is consistent with [Audrito–Vazquez equation 3.4](https://arxiv.org/html/1601.05718v2); reaction-wave speeds from that paper are not used. Constitutive flux is evaluated at the **same exact physical contour location** for both fits using the original capped law. An independent pressure-variable expression is checked against the original depth-gradient expression in active- and inactive-cap controls. Using exact `q=c*h_reconstructed` would hide the estimated-slope error, so that shortcut is not used.

Affine-extension support/crossing locations are explicitly named. A crossing outside the selected interval has null cell-local `crossingMeters`; extrapolation remains a diagnostic, not a discovered local contour. Exact floating-point interval bounds are used, without snapping or position correction.

## Result and decision

The known-slope branch reproduces threshold/support positions to floating-point arithmetic: maximum affine position error is below `2.35e-13 m`. The maximum positive local stock residual is below `2.82e-15`, under the separate `256*epsilon` limit. Eight zero-stock support fits per branch remain underdetermined, not successful front identifications. All positive thresholds have a stock fit in this family.

The upstream-mean-power branch is **not exact**. Raising a mean depth to `7/3` does not give the mean or center pressure, even in a fully wet interval. At front phase .8, its estimated slope is `1.0083789605756979*S`, and its support is about 8.0003 mm behind the exact front at width 4 m. The largest absolute affine-location errors across these fixed controls are:

| Location | Upstream-mean-power fit |
| --- | ---: |
| Support | 8.0003 mm |
| 1 mm contour | 7.9781 mm |
| 1 cm contour | 7.7425 mm |
| 5 cm contour | 0.8803 mm |

These errors are not comparable to the earlier spherical tens-of-metres proxy errors: geometry, resolution, inputs and observer differ. This control does not qualify that witness. Known-slope success proves representability and the local integral/inversion algebra, not practical slope recovery or evolving accuracy. The self-similar support phase sweep also cannot establish a general convergence order from error scaling with width.

Small position error is not a flux certificate: at 30 of the 72 exact 1 mm contour locations the upstream-mean-power reconstruction is already dry, so its flux is zero against the exact `0.0001 m²/s`. This surrogate is not selected as a remedy. Its slope bias must be addressed before an evolving trial. All matched positive-depth flux controls have an inactive mobility cap; separate unit controls exercise the active cap.

The [scalar validation record](data/surface-planar-reconstruction-validation.json) retains per-width/per-threshold aggregate comparisons, unit-width support-phase diagnostics and report/source fingerprints. All 72 complete cases remain in the reproducible ignored `artifacts/` report. Eight known-slope support crossings at phase 1 fall infinitesimally outside the interval under exact float classification and remain explicitly absent as local crossings; their affine errors are roundoff-scale and are not snapped away. Production ownership and one shared face exchange are not implemented by local inventory fitting. Neighboring reconstructed profiles can disagree at a face; no continuity or evolving conservation conclusion follows here.

**Follow-up:** [adjacent-moment all-cell controls](surface-moment-reconstruction.md) remove the mean-power bias in the manufactured affine-pressure family, but retain support-boundary gaps and two independent face incompatibilities. One shared face operator and its frozen derivative consistency are now the next gate before short planar paired evolution. Triangle/tangent/spherical reconstruction, variable bed, barriers, initial sample-stock compatibility and seasonal integration remain separate gates. Do not promote this oracle as a solver or rerun the large sphere merely because its local formula works.

## Reproduction and validation

```bash
cargo run --release --locked --manifest-path native/Cargo.toml --example surface_planar_reconstruction_report -- artifacts/new-planar-reconstruction.json
cargo test --locked --manifest-path native/Cargo.toml --lib reconstruction
```

The CLI requires one new output path and refuses overwrite/extra arguments. The entire bounded report is computed before creating the file. Its report version is `surface-planar-reconstruction-report-1`, not a checkpoint or model selector.

Four directed tests cover interval integrals (including independent polynomial quadrature), dry/constant/partial/fully wet inversion, same-stock/different-slope nonidentifiability, tiny positive stock without a floor, invalid inputs, original/capped pressure-flux equivalence, reflected coordinates and flux signs, complete report repeat, and an independently predicted biased-slope case. Regression and final report fingerprint details are retained in the validation record. No GUI run or new evolving PDE cohort is claimed.

Full `npm test` passes 593 native tests across 110 targets, with the same one pre-existing ignored test, and all eighteen Node files. This run preceded the final report-only wet-fraction/cap diagnostic fields and golden assertions; reconstruction arithmetic and physical operators were unchanged. The final source passes all 138 optimized library tests, strict all-target Clippy, formatting and whitespace checks. Three CLI argument/overwrite refusals preserve existing output and leave invalid output absent. The complete final 953,529-byte report repeats byte-for-byte with SHA-256 `1fe17f60c05faa09840977f9e627f1b1fdfa4fde8be992b9da6539655bd354ea`. Pre-existing renderer edits remain untouched and excluded.
