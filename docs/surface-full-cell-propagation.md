# Bounded full-cell propagation

October 12, 2026. This follows [temporal/grid conditioning](surface-cadence-conditioning.md). The new standalone laboratory retains the pressure reconstruction, shared-face law, explicit Euler integration and compensated transactions. It tests physical front movement through whole owners, not a new production water model.

## Fixed gate and work budget

The symmetric domain is `[-32,32] m`, initial front `0`, wave speed `.1 m/s`, final time `11 s`, roughness `.04`, cap `100 m²/s` and pressure penalty 1. Six forward trajectories combine widths `1/.5 m` with steps `.005/.0025/.00125 s`; a seventh reflects the finest width-.5 trajectory. Each trial has at most 128 cells, 8,800 steps and 1,126,400 cell-step updates. The cohort requests 39,600 steps and 4,083,200 cell-step updates.

This is a separately bounded experiment. The old 200 m/.1 s laboratory and its 2,000-step limit are not expanded. Its configuration still describes the old domain/time, even when width and cadence also belong to this new cohort. The narrower domain retains all four existing support/1 mm/1 cm/5 cm contours. Its RMS normalization differs from the old 200 m domain; the two RMS values are not directly comparable accuracy measures.

Exact initial integrated means and instantaneous old-time exterior fluxes are the only physical uses of the analytic wave. The extracted shared transaction receives observed stocks, width, cadence, method and two exterior fluxes, never front, speed, domain position or a future exact state. The old wrapper delegates with its unchanged boundary expressions. Both current and initial owners and counters are validated; proposals commit atomically after the unchanged donor and arithmetic checks. There is no film, slope floor, snapped support, resized request or fallback.

The exact front ends at `1.1 m`, beyond one coarse owner and two fine owners. The common physical face at normal coordinate `1 m` has exact onset `10 s`. Volume beyond this fixed face is observed separately from total initially dry volume. For reflection it is the region `x<=-1`, rather than a region selected using the exact front. First represented arrival has no detection floor and retains both its actual volume and the exact volume at that time.

Initial/first/final observations and common times `5/9.9/9.995/10/10.005/10.1 s` retain all-cell errors, actual owned arrival volume, unresolved cells and independently discovered contours. Missing/nonunique/incomplete roots have null position error. Terminal high/low arrays, compensated boundary counters, ledger maxima and any refusal's last accepted time remain in the complete report. Three-level temporal comparisons require the same physical problem, grid, final time and successive time halves; refused trajectories yield null comparisons.

## Results

All seven trials complete all 39,600 requested steps. The reflected finest trajectory has bit-identical reversed high/low owners and identical boundary counters. No reconstruction or transaction refusal occurs. Maximum local/cumulative relative ledger residuals are approximately `1.110e-16 / 3.132e-17`, below the unchanged laboratory arithmetic bound `256*epsilon`.

Exact final volume beyond the common face is `3.30258343088914e-4 m²` per unit breadth:

| Width, m | Step, s | Actual volume beyond face 1, m² | Final mean RMS, m | First arrival, s |
| ---: | ---: | ---: | ---: | ---: |
| 1 | .005 | 3.2881901600e-4 | 1.86262446e-7 | 10.0100 |
| 1 | .0025 | 3.2954425854e-4 | 9.23954186e-8 | 10.0050 |
| 1 | .00125 | 3.2990330544e-4 | 4.59342927e-8 | 10.0025 |
| .5 | .005 | 3.2871644219e-4 | 2.79341239e-7 | 10.0100 |
| .5 | .0025 | 3.2949055894e-4 | 1.39114855e-7 | 10.0050 |
| .5 | .00125 | 3.2987557858e-4 | 6.93595915e-8 | 10.0025 |

The reflected case has the same finest volume and RMS. At every retained observation at or before `10 s`, the common-face region remains dry. Its measured first-arrival lag decreases from `.01` to `.0025 s`. Fine-grid face `.5 m` similarly has arrival at `5.01/5.005/5.0025 s`. These are observed two-step lags in this fixed cohort, not a universal arrival rule.

Initial face-zero arrival is different: moment/endpoint arithmetic produces a first-step volume of about `2.97e-13–1.45e-12 m²`. That tiny trace registers immediately under a floor-free observer and is much smaller than exact volume at the same first-step time. It is retained, not clipped or described as accurate arrival timing. The separate common-face control supplies the meaningful full-owner crossing check.

All four final contours are unique with complete reconstruction. Largest final support errors are `.3258/.3614 mm` at the coarsest step for widths `1/.5`; finest errors decrease to `.0803/.0896 mm`. Temporal field-difference RMS ratios are `2.02037 / 2.01027`, and volume/contour errors also decrease. These measurements support bounded temporal refinement and full-cell transport in this manufactured family.

## What this does and does not establish

This closes the planned **bounded full-cell propagation** gate: water moves into later owners, cumulative accounting survives thousands of rebuilt steps and arrival/profile error decreases with time refinement. It does not establish unrestricted long-time stability, planet runtime scaling, production readiness or a spatial convergence order. Clipped-affine pressure remains semidiscrete-exact under this fixture's identified stencils and inactive cap; a [proposed non-affine evolving benchmark](surface-non-affine-benchmark-proposal.md) defines the next accuracy gate. Its oracle and evolution are not implemented yet.

On the exact wave, maximum physical diffusivity coefficient `beta=(7/3)*c*(front-x)` is about `7.72333 m²/s`, below the cap. Principal fixed-depth diffusivity is `beta/2`. Its worst constant-central-diffusion reference number is approximately `.07723`; that is **not** a reconstructed nonlinear-Jacobian stability theorem. Conversely `cap*dt/width²=2` at the coarsest fine-grid step does not prove instability when the fixture's cap is inactive. [HEC-RAS's diffusive-wave documentation](https://www.hec.usace.army.mil/confluence/hecras/beta/technical-reference/hydraulic-equations/diffusive-wave-equation) describes the explicit diffusion restriction; it does not certify this reconstruction.

General extrema/plateaus, non-affine evolving accuracy, variable bed/rest/barriers, spherical assembly, seasonal coupling and calibration remain open. No product/save/model pins or desktop selection change.

Follow-up: [integrated non-affine evolution](surface-non-affine-evolution.md) now supplies genuine curved-pressure spatial-refinement evidence on a separate bounded monotone problem. It does not retroactively turn this affine-pressure fixture into a spatial-order test or qualify the remaining geometry/rest/barrier gates.

## Reproduction and validation

```bash
cargo run --locked --manifest-path native/Cargo.toml --example surface_full_cell_report -- artifacts/new-full-cell.json
cargo test --locked --release --manifest-path native/Cargo.toml --lib full_cell
```

`surface-full-cell-report-1` and its new-output-only CLI are laboratory interfaces. The [validation record](data/surface-full-cell-validation.json) retains case summaries and final-source evidence; ignored complete reports retain all physical arrays. No GUI or serialized checkpoint support is claimed.

Final source passes all 164 optimized library tests, twelve regional-soil and five regional-surface integration tests (181 total), strict all-target Clippy, typecheck, formatting and whitespace checks. The native release executable is rebuilt and all eighteen Node files pass (88 tests). Three CLI refusal checks preserve the existing report and leave invalid extra-argument output absent. The full `npm test`/all-native-target suite and GUI are not rerun here.

The complete 225,845-byte report agrees byte-for-byte between debug, optimized and repeated optimized runs, SHA-256 `4cac6607331ac7b9535d86766f80cbb97afac56a3a3c3b9d914255ab5a0c5128`. Rebuilt old paired/cadence reports preserve SHA-256 `8684c711e4009578b6e0c76df67a0d06123531eb6ba02c9cbf57da11296aae58` and `3aaeb44851096be45d61f6970c2528c7ebe240cb010b76803a2d07ba4e01dc6e`. Directed checks cover shared-core/legacy-wrapper bit equality, malformed owners/counters, observer geometry, finite norms, temporal mismatch/refusal and fixed-region reflection. Pre-existing renderer edits are untouched and excluded.
