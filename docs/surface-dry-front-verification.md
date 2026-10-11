# Moving wet/dry-front verification

October 11, 2026: `surface-dry-front-report-1` adds an independent dry-capable reference and measures the unchanged [full-gradient candidate](surface-full-gradient-candidate.md) on an initially dry hemisphere. Conservation and bounded refinement pass, but **moving-front accuracy is not qualified**: the candidate still substantially advances low-depth contours beyond the reference. No seasonal model, save schema, desktop selector or default changes.

## Physical hypothesis and exact certificate

Keep the previously declared capped diffusive-wave law, with hydraulic radius approximated by depth:

```text
eta = bed + h
q = -beta * grad(eta)
beta = min(Dmax, h^(5/3) / (n * sqrt(|grad(eta)|)))
h_t = div(beta * grad(eta))
```

[HEC-RAS's governing equation](https://www.hec.usace.army.mil/confluence/hecras/beta/technical-reference/hydraulic-equations/diffusive-wave-equation) supplies the gravity/friction hypothesis, not our cap, reconstruction or discretization. Its scope is gradually varied, friction-dominated flow; inertia-driven dam breaks, ocean waves, wind and Coriolis are not modeled. The following exact profile is a project derivation for this equation, not an external flood calibration or a shallow-water dam-break benchmark.

On a flat line, let `z=(c*t-x)_+`, `K=(7/3)*(c*n)^2` and `h=(K*z)^(3/7)`. On the wet side:

```text
h_x = -(3/7)*h/z
beta*h_x = -c*h
beta = (7/3)*c*z
h_t = -c*h_x = (beta*h_x)_x
```

Depth and flux vanish continuously at the moving edge, giving a weak traveling-wave solution. The gradient is singular there; this is a mathematical verification problem, not a claim that this idealized front is a measured physical flood.

The planar certificate uses `x in [-100,100] m`, `c=0.1 m/s`, `n=0.04 s/m^(1/3)` and `T=100 s`. Its front moves from 0 to 10 m. `Dmax=100 m^2/s` is inactive because the exact wet-domain coefficient remains below 26 m²/s. Initial cell averages and final comparison averages are integrated analytically. The left boundary supplies the exact time-integrated flux `c*h(-100,t)`; the right boundary is dry and closed. There is no volume forcing, fitted final state or mass rescaling.

## Independent dry reference

The new reference imports neither native graph geometry nor the full-gradient stage. Only compensated scalar summation is shared. Equal-width planar cells or equal-area spherical bands use centered face depth, backward Euler, a tridiagonal M-matrix solve and damped Picard iteration. Each proposed state rebuilds its nonlinear equation residual; convergence requires at most `1e-12 m` with at most 256 iterations. Nonfinite/negative depths, failed maximum bounds, nonconvergence or scalar inventory drift above `1e-9` refuse the report. These reference controls do not widen native audit tolerances.

A completely dry interface has zero mobility. Positive depth with zero gradient takes the explicitly capped limit. No dry film, depth clipping, hidden water source or stock repair is applied. The initial Picard guess includes already accounted boundary inflow, so both the linear solution and its convex relaxation have the same inventory. A first implementation omitted that input from the guess: an inventory test caught relative drift `2.371e-10`. The correction changes the guess, not the tolerance. The retained record fingerprints that failed test log.

Eight planar controls retain exact and computed endpoint arrays. At `dt=0.0625 s`, RMS cell-average error falls from 0.2555 mm at 64 cells to 0.09769, 0.09374, 0.03699 and 0.02192 mm at 128, 256, 512 and 1,024 cells. On 1,024 cells, successive time-refinement differences have RMS 16.35, 8.642 and 4.467 micrometres. Endpoint error against the exact wave is **not monotone in time step**: spatial and temporal errors partially cancel. Do not interpret that cancellation as a higher-order scheme or select the smallest endpoint error as the reference. All controls and their errors are retained.

For the sphere, `p=axis.unit_position`, `dA=2*pi*R^2*dp` and:

```text
h_t = R^-2 * d_p [beta * (1-p^2) * d_p h]
|grad h| = sqrt(1-p^2)*|d_p h|/R
h(p,0) = p_+^(3/7)
```

Use `R=1,000 m`, `Dmax=1,000 m^2/s`, `n=0.04`, `T=300 s` and closed polar fluxes. The cap is active near the wet pole and is part of this declared regularized problem, not a performance-only adjustment. These are fixed numerical verification settings, not a generated Earth-like world or calibrated terrestrial defaults. The southern hemisphere is initially exactly dry.

The eight spherical reference controls are `(bands, dt seconds)`: `(256,0.5)`, `(512,1)`, `(512,0.5)`, `(512,0.25)`, `(1024,0.25)`, `(2048,0.25)`, `(4096,0.25)` and `(4096,0.125)`. The last is selected. Initial band averages are analytic. Convex linear interior interpolation and constant polar extension preserve nonnegativity when sampling native centers. Footprint and pole-extension error remain part of measured spatial uncertainty.

Last reference spatial/time contour differences are respectively 0.282/0.237 m at 1 mm, 0.0747/0.123 m at 1 cm, and 0.0919/0.0663 m at 5 cm. These are refinement indicators, not rigorous error bounds. At actual native centers, last reference spatial/time RMS differences are at most 1.037%/1.244% of candidate RMS discrepancy. The reference's maximum scalar inventory drift is `1.145e-13`; all sixteen planar/spherical controls converge, with at most 88 iterations and residual below `1e-12 m`.

## Unchanged native stage and front measurements

Eighteen candidate trajectories use two axes (`[1,0,0]`, normalized `[1,2,3]`), 642/2,562/10,242 regions and shared time steps of approximately 0.1058/0.05289/0.02645 s. Each caller performs one stable frozen stage. Two matched old-directional trajectories use the finest mesh/cadence. Initial point samples multiply actual spherical areas, without fitting continuum mass. Initial quadrature discrepancy is reported, not corrected: for the x axis it decreases from -1.1905% to -0.1704%; for the tilted finest grid it is -0.002864%.

All paired stocks and directed histories are retained. Independent local/global inventory audits run every 64 steps and at completion. Numerical deferrals are recorded attempted requests whose donor stock remains unchanged, not completed transfers or lost water. Maximum candidate local/global relative residuals are `4.470e-16`/`1.293e-17` under unchanged native limits. Accounting success does not imply front accuracy.

Thresholds are 1 mm, 1 cm and 5 cm. Reference crossings are interpolated in the monotone band profile. Native graph edges crossing a threshold interpolate both depth and `p`; signed error uses `R*(acos(p_native)-acos(p_reference))`. This is an adjacency-edge contour proxy, **not exact reconstructed flooded surface geometry or exact PDE support**. A numerical method can produce tiny positive tails, so `h>0` is not an adequate front definition. Wet area is separately measured by actual region areas, and water arriving in the initially dry half is also reported.

At the finest cadence:

| Regions | RMS error, x / tilted, mm | Maximum 1 cm contour error, x / tilted, m |
| ---: | ---: | ---: |
| 642 | 30.819 / 32.111 | 211.34 / 237.54 |
| 2,562 | 21.111 / 23.037 | 122.14 / 133.42 |
| 10,242 | 15.330 / 16.340 | 70.47 / 73.09 |

The reference's 1 cm contour advances 152.48 m. Finest maximum errors at 1 mm are 95.92/97.95 m and at 5 cm are 41.29/44.02 m. All native contour crossings lie ahead of the reference. Candidate spatial RMS ratios are 0.685–0.726; every comparison improves, but this is slow refinement, not an established universal convergence order. Time differences approximately halve in all six groups (ratios 0.499882–0.499952), and the last difference is at most 0.0268% of endpoint RMS error. A smaller caller step does not resolve the observed spatial discrepancy.

The old matched branch is worse: finest RMS 24.080/24.702 mm and maximum 1 cm contour error 90.50/91.63 m. Improvement over that branch is not enough for promotion. Candidate initially-dry-half inventory still exceeds the reference by 19.53%/10.29% at the finest mesh; the x-axis equatorial vertex band and point-sampled initialization also affect that aggregate. No single cause or fitted correction is inferred from it.

## Reproduction, isolation and next action

The [retained scalar record](data/surface-front-validation.json) contains all twenty native cases, all sixteen reference controls, refinement comparisons, numerical retention and source/report fingerprints. Complete arrays remain in ignored `artifacts/`, not save formats. Reference-only output has a separate pin and cannot constitute a native accuracy pass.

```bash
cargo run --release --locked --manifest-path native/Cargo.toml --example surface_front_report -- artifacts/new-front.json
cargo run --release --locked --manifest-path native/Cargo.toml --example surface_front_report -- artifacts/new-dry-reference.json --reference-only
cargo test --locked --manifest-path native/Cargo.toml --lib surface_front
```

Both modes require a new output path, refuse invalid arguments and refuse overwrite. Validation and full-report replay results are recorded alongside the fingerprints. No GUI rerun is claimed; the owner's renderer changes remain untouched and excluded.

The complete 79,883,936-byte report repeats byte-for-byte. Reference-only output contains the same sixteen reference controls, has pin `surface-dry-reference-report-1` and contains no native cases. The rebuilt preceding 43,215,413-byte smooth-wet report retains its exact historic hash. Six new directed tests cover the exact wave, analytic averages, no-film conductance, numerical reference refinement, repeated native arrivals/accounting and bounded refusal. Full `npm test` passes 560 native tests across 105 targets and all 18 Node files, with the same one pre-existing ignored annual-snow test. Final release regression passes 105 library, twelve regional-soil and five regional-surface tests; all-target warnings-denied Clippy, formatting, whitespace and both-mode CLI refusal checks pass. Report-metadata-only additions after the full-suite start were separately covered by final release checks and the actual repeated CLI reports; no physical stage or test case changed in that interval.

Next, use this fixed witness to isolate finite-grid interface-depth and gradient-reconstruction effects before changing a separately pinned candidate. Source-side depth and a gradient spanning wet/dry triangles are hypotheses to test, not diagnosed causes. More annual endurance runs or adjusting roughness to hide this discrepancy are not accuracy fixes. Sharp bed steps, disconnected wet regions and terrain barriers must then be verified independently: [HEC-RAS 5.0.6 release notes, page 10](https://www.hec.usace.army.mil/software/hec-ras/documentation/HEC-RAS_5.0.6_Release_Notes.pdf#page=11) document an earlier dry-cell gradient issue involving assumed hydraulic connection. That motivates this check; it does not establish that our code has the same defect or provide a drop-in patch.

Follow-up: the separately pinned [flat centered-depth comparison](surface-centered-depth.md) now implements `h_face=(h_source+h_target)/2` while retaining the full-gradient denominator, cap, cotangent weights, paired transfers and fixed initial data. Finest 1 cm proxy errors fall to 25.29/30.75 m and dry-cap RMS errors to 6.825/4.834 mm; smooth-wet finest errors also improve, although coarse wet errors slightly worsen. This identifies an important finite-grid contribution, not complete front qualification or a variable-bed/finite-body generalization. The original report and reference remain reproducible; gradient isolation and sharp-bed controls remain next before seasonal selection.

Seasonal selection/persistence remains deferred. Fixed coastal heads, soil/cold coupling, generated-world qualification, long-run drift and physical calibration remain separate gates. Neither this retained front discrepancy nor the preceding smooth wet success completes milestones C/D or ecology readiness.
