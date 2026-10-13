# Integrated non-affine evolution gate

October 12, 2026. This implements the [cubic-pressure benchmark proposal](surface-non-affine-benchmark-proposal.md) after the [full-cell propagation gate](surface-full-cell-propagation.md). It adds independent reference/diagnostic laboratories, not a new selected water operator. The pressure reconstruction, face law, donor rules and compensated physical transaction remain unchanged.

## Independent oracle before evolution

The monotone Barenblatt half-profile has `tau=10+t`, `R=10*(tau/10)^(3/8)`, `k=7*n²/64`, `v=(k/tau²)*(R³-x³)`, `h=(v_+)^(3/7)` and `q=(3/8)*x*h/tau`. Its physical coefficient is verified directly against the original Manning flux, not imported from a differently normalized equation. [Audrito–Vázquez, Section 2](https://arxiv.org/pdf/1601.05718) supplies the compact-support similarity-family context; it does not certify our discrete operator.

The oracle integrates actual cell means using `R-x=z^7`, removing the support derivative singularity. Narrow transformed intervals form their span with `log1p/expm1`, rather than subtracting independently rounded seventh roots. The initial 4,096-panel Simpson limit refused its unchanged `64*epsilon` successive-agreement criterion. The explicit reference-only limit is now 16,384 panels; neither its criterion nor fixture tolerances are relaxed. A centered derivative probe also required time refinement near a recently crossed face; three probe sizes (two halvings) verify its truncation behavior before retaining the original fine-probe bound.

A separate `mpmath==1.3.0` incomplete-beta oracle evaluates all 336 means at 80/100 decimal digits and checks the original flux and continuity independently. Rust's final 336 means agree with the 100-digit oracle to maximum absolute difference `1.08048e-16 m` and maximum relative difference `1.96726e-14`. All independently dry means are exactly zero. The relative maximum belongs to a tiny positive mean; successive quadrature agreement alone is not a certified accuracy bound. Twelve rounded fixtures enter validation only, never initialization corrections or reconstructed pressure. Independent partition/restriction and mean-derivative controls are retained separately.

The analytic cap stays inactive on `[2,14]`: the exact maximum is about `14.6521 m²/s`; the retained conservative upper bound, dropping the negative cubic term, is `16.2339 < 100`. This validates applicability of the exact family, not explicit stability. The symmetric extremum at zero is deliberately outside this domain.

## Fixed experiment

Nine forward trajectories combine widths `1/.5/.25 m` and steps `.001/.0005/.00025 s`; a tenth reflects the finest trajectory onto `[-14,-2]`. Final time is `1 s`. Work limits are 48 cells, 4,000 steps and 192,000 cell-step updates per trial; the cohort requests 25,000 steps and 780,000 cell-step updates. These do not expand previous laboratory limits.

Exact integrated means initialize owners once. Exact instantaneous exterior fluxes enter the unchanged transaction at each old time. Interior reconstruction receives only accepted observed means, width and the existing method; no analytic pressure, slope, front, speed or future state is supplied. All high/low components and boundary counters persist. No film, floor, support snapping, fallback, mass adjustment or resized request is introduced.

Full-domain mean error remains primary. A fixed additional whole-owner window `[3,9]` isolates wet-interior behavior; the common initially dry region is `normal x>=10`. Observations retain initial/first/quarter/half/three-quarter/final states, independent all-cell contour discovery, actual arrival magnitude, refusal reasons and accepted times. Invalid/missing/nonunique/incomplete roots have null errors. Refused trajectories cannot enter comparisons at the requested final time.

Three temporal triples compare component-aware fields. Six nested-grid pairs restrict high/low owned volumes, cancelling the largest fine high first. Four Richardson pairs compare observer-only vectors `2*error(dt/2)-error(dt)` across grids; these are never stocks or solver inputs. Initial/final oracle restriction floors remain visible without being subtracted from physical results.

## Results and decision

All ten trajectories complete all 25,000 requested steps. Maximum local/cumulative relative ledger residuals are `1.1091e-16 / 5.8598e-17`, below the unchanged arithmetic bound `256*epsilon`. The reflected finest result has bit-identical reversed high/low owners and identical boundary counters. All final contours are uniquely observed with complete reconstruction.

At the finest common cadence `.00025 s`:

| Width, m | Full-domain mean RMS, m | Fixed wet-interior RMS, m | Support position error, mm |
| ---: | ---: | ---: | ---: |
| 1 | 1.35359705e-4 | 5.18266503e-5 | +31.1805 |
| .5 | 4.95392662e-5 | 1.36447879e-5 | +11.8498 |
| .25 | 2.17725528e-5 | 3.54673499e-6 | +3.6702 |

Full-domain RMS improves by factors `2.732 / 2.275`; wet-interior RMS by `3.798 / 3.847`. Temporal field-difference ratios range about `1.967–2.001`. Total RMS can increase when time is refined: temporal/spatial errors partly cancel, so scalar monotonicity is not a time-convergence criterion. Observer-only Richardson intergrid RMS changes by about `2.55e-11 / 2.40e-9 m` between its two temporal pairs, much less than retained intergrid differences of approximately `9.88e-5 / 3.41e-5 m`. A three-grid/two-time-pair witness supports spatial improvement here, not a universal spatial-order theorem or unrestricted long-time stability.

This closes the planned **non-affine evolving spatial-refinement gate in this bounded monotone family**. The former affine-only evidence is no longer the sole accuracy witness. General extrema/plateaus, wet/rest/barrier behavior on variable bed, spherical reconstruction/assembly, seasonal integration, calibration and production runtime remain separate gates. No product/model/save pins or desktop selections change.

Follow-up: the [affine-bed head/storage baseline](surface-topography.md) now exercises bounded rest and interface blocking independently. It does not extend this flat-pressure candidate, erase its stencil refusals or qualify moving variable-bed accuracy.

## Contour limitations remain explicit

At the finest cadence, the 5 cm contour errors are approximately `+31.42/-1.50/-2.93 mm`: this contour does not improve monotonically. Six independent exact-integrated-mean probes at times `0/1` use the identical root observer after all physical runs. At final time their 5 cm errors are `+44.25/+1.38/-2.25 mm`; thus this nonmonotonic behavior also exists without an evolving-state error. These probes are not evolved states, additive error corrections or replacements for reported physical contours. Moment inversion and root extraction are nonlinear, so subtracting these probe errors does not establish a solver-error decomposition.

At initial aligned support, all three exact-mean probes lack an isolated support and 1 mm root even though all cells reconstruct: six explicit missing-root observations. They remain null, not snapped to the known front. At final time all probe roots are discovered. Such representation/phase dependence must remain visible before any general front or production qualification.

## Reproduction and final-source checks

```bash
cargo run --locked --manifest-path native/Cargo.toml --example surface_barenblatt_reference_report -- artifacts/new-oracle.json
cargo run --locked --manifest-path native/Cargo.toml --example surface_non_affine_report -- artifacts/new-non-affine.json
cargo test --locked --release --manifest-path native/Cargo.toml --lib non_affine
cargo test --locked --release --manifest-path native/Cargo.toml --lib barenblatt_reference
```

The [validation record](data/surface-non-affine-validation.json) retains case summaries, all comparison/probe outcomes, independent-oracle evidence and fingerprints. Full physical arrays remain in ignored complete reports. Both CLIs require a new output path; six refusal checks preserve existing output and leave invalid extra-argument paths absent.

Final source passes all 173 optimized library tests, twelve regional-soil and five regional-surface integration tests (190 total), strict all-target Clippy, typecheck, formatting and whitespace checks. The native release executable is rebuilt; all eighteen Node files pass (88 tests). The complete `npm test`/all-native-target suite and GUI are not rerun here.

The 294,303-byte complete report is identical across debug, optimized and repeated optimized execution, SHA-256 `637bbf678092f9fca352c9de5c1548d3baf5a4e0eb979cdb3f2c352912577bba`. The standalone oracle matches the embedded reference and has SHA-256 `d4a6dce300a814fd9c986db385551e1480bd3deba97f590045e4ea0ff7f8bfda`. Rebuilt old full-cell output preserves SHA-256 `4cac6607331ac7b9535d86766f80cbb97afac56a3a3c3b9d914255ab5a0c5128`. Pre-existing renderer edits are untouched and excluded.
