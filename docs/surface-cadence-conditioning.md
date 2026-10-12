# Temporal and grid conditioning of the planar wave

October 12, 2026. This follows [consecutive paired evolution](surface-paired-evolution.md). Three independent read-only reviews covered the mathematics, primary numerical sources, and observers/tests. The increment adds a bounded diagnostic report, not a new water operator. Roughness, cap, flux construction, donor rules and physical state updates are unchanged.

## What the earlier spatial comparison actually measured

Write `alpha=3/7`, `A=S^alpha`, `h=A*(front-x)_+^alpha` and `v=h^(1/alpha)`. On this manufactured wave, pressure is clipped affine. In exact arithmetic, at the tested widths with identifiable neighboring moments and the fixture's inactive cap:

1. Adjacent exact integrated means recover the affine pressure.
2. Continuous positive face traces give the original exact flux `q=c*h`; support/dry faces give zero.
3. The conservative face difference and exact prescribed exterior flux reproduce the derivative of the exact cell means.

Thus the analytic cell-average trajectory is already a solution of the **semidiscrete** operator. This family can expose time integration, conditioning and finite arithmetic errors, but it cannot identify a nonzero spatial truncation order. It does not prove consistency for non-affine pressure, arbitrary stencils or another geometry. The earlier frozen curved-pressure spatial witness remains a separate check; a non-affine evolving accuracy gate is still open.

For an aligned initially dry face, exact new-owner volume is

```text
V(T) = A*(c*T)^(alpha+1)/(alpha+1)
```

The first explicit step sees zero instantaneous flux despite positive future volume of order `dt^(10/7)`. A left-rectangle approximation to the exact subsequent face flux gives a leading deficit approximately `-dt*A*c^(alpha+1)*T^alpha/2`. For the existing fixture, its magnitude at `dt=.001 s` is `8.79335e-8 m²`; actual evolving deficits are `8.41108e-8 / 8.03076e-8 m²` at widths `1/.5 m`. This is an explanatory quadrature asymptotic, not a bound for the feedback trajectory.

If a comparable volume error occupies one or a few owners, their mean-depth error divides by width. On a fixed domain of length `L`, the resulting all-cell RMS scales approximately as `abs(volumeError)/sqrt(L*width)`. This explains why a smaller cell can have larger RMS at fixed time step while arrival volume and physical front position improve. It is a short-window localization explanation, not a universal spatial exponent.

## Fixed diagnostic cohort

Retain the same domain `[-100,100] m`, final time `.1 s`, wave speed `.1 m/s`, roughness `.04`, cap `100 m²/s` and pressure penalty 1. Sixteen main forward trajectories combine widths `1/.5 m`, physical fronts `0/.995 m` and four steps `.001/.0005/.00025/.000125 s`. One additional aligned trajectory uses width `.25 m`, step `.0000625 s`; one reflected delayed trajectory uses width `.5 m`, step `.000125 s`. All remain within the existing 1,024-cell/2,000-step caps. There are 18 trajectories and 8,400 accepted steps in the final report.

Eight temporal triples compare actual component-aware fields at the same physical end time. Six intergrid pairs restrict two fine owned volumes into one coarse interval, including both high/low components. They compare the same domain, front, direction, method and time; each time pair must retain one grid. Incomplete trajectories yield null comparisons, invalid observations refuse, and zero/nonfinite ratio denominators do not become infinite or zero accuracy claims.

For consecutive temporal halves, the diagnostic vector is `R=2*U(dt/2)-U(dt)`. Implementation applies this to errors against the common exact cell means, avoiding subtraction of large nearly equal reconstructed samples. It records RMS, maximum, signed volume, volume-weighted L1, minimum inferred sample and negative-sample count. **R is never a physical stock, state correction, flux input, fitted final state or selected integration method.** Negative extrapolated observations are allowed and tested without clipping.

`U_middle-U_fine` is an empirical estimate of the fine temporal **error**, not the signed correction to apply. A third time level checks how field differences and extrapolated residuals change. Singular arrival can leave a `dt^(10/7)` term after the leading `dt` term is removed, so a Richardson residual ratio near `2^(10/7)=2.692` is plausible; four is not required and no general second-order theorem follows.

Coarse/fine exact-mean restriction discrepancies are recorded at both initial and final times, never subtracted as a correction. This matters if a diagnostic approaches the analytic primitive's arithmetic floor. Restricted volumes cancel the largest fine high component first: otherwise a tiny fine owner could disappear in an intermediate subtraction. An asymmetric `2^54+1` witness and its reflection verify this case independently of the trajectories.

## Results

All 18 trajectories complete with normalized stocks and accounted boundaries. The reflected trajectory retains bit-identical reversed high/low owners and equal boundary counters. Successive temporal field-difference RMS ratios are approximately `2.029–2.110`. Leading-error cancellation substantially reduces the remaining error, but measured Richardson residual ratios are about `2.72–2.75`, not four. Those ratios are observations in this singular family, not a selected higher-order solver.

At width 1, aligned-front mean RMS decreases from `8.26410e-9` to `9.96858e-10 m` across the four steps. At width .5 it decreases from `1.10430e-8` to `1.33260e-9 m`. Fixed-step fine-grid RMS remains larger, but the same trend now has a measured temporal explanation rather than being interpreted as failed spatial consistency.

After restricting the fine grid, aligned intergrid RMS differences at steps `.001/.0005` are `3.68001e-10 / 1.80404e-10 m`; the corresponding Richardson difference is `7.19330e-12 m`. At `.00025/.000125`, it is `1.16557e-12 m`. Delayed-crossing Richardson intergrid RMS decreases from `1.05094e-11` to `1.87138e-12 m`. These are much smaller than the original time-contaminated differences but still do not establish a spatial order for a semidiscrete-exact family.

The coupled diagonal holds `cap*dt/width²=.1`:

| Width, m | Step, s | Final mean RMS, m |
| ---: | ---: | ---: |
| 1 | .001 | 8.26410e-9 |
| .5 | .00025 | 2.68769e-9 |
| .25 | .0000625 | 8.58918e-10 |

This is improved **coupled space/time** behavior, not independent spatial convergence. The diagonal's finest point has no finer temporal partner within this cohort; do not imply its time uncertainty is separately certified.

## Physical sources and decision

[HEC-RAS's diffusive-wave documentation](https://www.hec.usace.army.mil/confluence/hecras/beta/technical-reference/hydraulic-equations/diffusive-wave-equation) explains the slope-dependent diffusion coefficient, explicit time-step restriction and implicit preference. [Manorost–Bastian](https://arxiv.org/html/2512.16525v1) likewise discusses explicit cost and uses its own implicit/cut-cell formulations. Neither source supplies a stability theorem for this reconstructed explicit hypothesis.

Directly differentiating the current uncapped law at fixed depth gives principal diffusivity `beta/2`; on the exact wave `beta=(7/3)*c*(front-x)`. A constant-coefficient central-diffusion time bound is only a reference scale. `cap*dt/width²` and donor non-overdraw do not bound this reconstruction's nonlinear Jacobian. No gradient floor, dry film, fallback or scalar fitting is introduced.

The earlier fixed-time RMS warning is now explained and bounded in this test family; it is not a reason by itself to replace the physical law. This closes the short-wave temporal-conditioning investigation, not sustained propagation or production selection. Next use a bounded full-cell/common-face crossing with explicit work/stability limits and matched time controls. Genuine spatial accuracy requires an independently defined non-affine evolving benchmark. Spherical geometry, terrain/rest/barriers, seasonal integration and calibration remain separate gates.

## Reproduction and validation

```bash
cargo run --locked --manifest-path native/Cargo.toml --example surface_cadence_conditioning_report -- artifacts/new-cadence.json
cargo test --locked --manifest-path native/Cargo.toml --lib cadence_controls
```

The new-output-only CLI and laboratory `surface-cadence-conditioning-report-1` do not change product/save/model pins. The [validation record](data/surface-cadence-conditioning-validation.json) retains case summaries, all eight temporal and six intergrid comparisons, diagonal observations and fingerprints. Complete physical stock arrays remain in ignored reports. No GUI, new spherical evolution or serialized checkpoint support is claimed.

Final source passes all 160 optimized library tests, twelve regional-soil and five regional-surface integration tests, all four directed cadence tests, strict all-target Clippy, typecheck, formatting and whitespace checks. The release native executable is rebuilt and all eighteen Node files pass (88 tests). Three CLI refusals preserve existing output and leave invalid extra-argument output absent. The entire `npm test`/all-native-target suite is not rerun here; earlier full-suite results are not relabelled as final-source evidence.

The complete 667,657-byte report repeats byte-for-byte and matches optimized output, SHA-256 `3aaeb44851096be45d61f6970c2528c7ebe240cb010b76803a2d07ba4e01dc6e`. Rebuilding the previous paired-evolution report preserves its SHA-256 `8684c711e4009578b6e0c76df67a0d06123531eb6ba02c9cbf57da11296aae58`. Pre-existing renderer edits are untouched and excluded. The source change to the old private laboratory only broadens helper visibility; its physical arithmetic and report contract are unchanged.

Follow-up: [bounded full-cell propagation](surface-full-cell-propagation.md) now crosses later owners over 11 seconds with improving temporal error and retained ledgers. It uses a separately declared smaller domain and does not replace this short-cohort result or establish spatial order.
