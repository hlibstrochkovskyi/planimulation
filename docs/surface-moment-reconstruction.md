# Frozen adjacent-moment reconstruction

October 12, 2026. This follows the [planar interval controls](surface-planar-reconstruction.md): their upstream-mean-power slope was biased and made 30 of 72 exact 1 mm locations dry. The new private native experiment infers pressure slope from two adjacent positive cell means, reconstructs **every cell before consulting truth**, and observes neighboring face disagreement. It does not evolve water, select a model, change native initial stocks, or qualify the original spherical witness.

## Inputs and identification

The manufactured wave, parameters, widths, phases and reflected directions are unchanged: `c=0.1 m/s`, `n=0.04`, `Dmax=100 m²/s`, `S=(7/3)*(c*n)^2`, widths 4/2/1/0.5 m and nine phases. There are 72 fixed-grid cases on `[-100,100] m`. Input means come from the existing analytic wave primitive, not from point samples or the reconstructed integral. The distinction matters at a singular front; [Clawpack's initialization documentation](https://www.clawpack.org/user_routines.html) explicitly distinguishes integrated cell averages from smooth center-value approximations.

Within the clipped-affine pressure ansatz, write `h=v_+^alpha`, `alpha=3/7`, `p=alpha+1=10/7`. Orient two positive means as upstream `mU>mD`. Let `P` be pressure at the upstream wetmost endpoint, `B=abs(slope)*width`, and `t=B/P`. Then:

```text
r = mD/mU
r = ((1-t)^p - (1-2*t)_+^p) / (1-(1-t)^p)
```

For `t>=1/2`, the downstream interval is partly wet. Its inversion is explicit: `delta=(r/(1+r))^(1/p)`, `t=1-delta`. Compute these through `log`/`log1p`/`expm1` and retain downstream pressure `P*delta` separately rather than subtracting `P-P*t`. For `t<1/2`, bounded bisection solves the observed relative mean decrease `(mU-mD)/mU`. A positive power series near `t=0` avoids cancellation between nearly equal integrals. Recover `P` from the upstream mean, then slope from `B/width`.

Both reconstructed means must have relative residual at most `256*epsilon`, independently of the native water ledger. The local owner's peak is fitted to its own mean with the previously checked integral; the difference from the pair-inferred peak remains a diagnostic. No actual stock is changed. Equal positive means give a constant profile within this ansatz; unrepresentable pressure or excessive residual refuses. Near-uniform relative slope accuracy is conditioned by the rounded difference of the inputs, not by stock residual alone.

This inverse is our derived scalar experiment, not an imported algorithm or a uniqueness theorem for arbitrary physical water profiles. Two adjacent stock totals identify only the chosen affine-pressure shape. A positive/zero pair and an all-zero pair do not determine a slope or latent support.

## Stencils, contours and face observations

The reconstruction accepts only means and a common width. It receives no exact slope, speed, front, threshold location or prescribed wet upstream stencil. A monotone interior owner chooses its single wetter neighbor. Extrema and mixed plateaus refuse rather than borrow truth; constant neighborhoods are handled separately. A boundary owner uses its only adjacent owner if the pair is identifiable. A dry owner has zero physical depth/flux but no inferred latent support.

Contour discovery scans isolated in-cell roots of all fitted profiles first. Truth is used afterward for position and matched-point flux comparisons. Constant level sets, including dry support or a plateau exactly at the threshold, are non-isolated and are not encoded as one root. Exact float interval classification is retained without snapping, extrapolation or deduplication. Missing roots are not assigned zero error.

Each shared face reports the two reconstructed depths and two one-sided constitutive fluxes in the **same global direction**. These are observations, not a single numerical flux or conservative transfer. Local integral agreement cannot certify their agreement. The law remains the existing capped pressure expression; a zero pressure or gradient yields zero flux without a film or slope floor.

## Measured result

All 6,814 positive wave owners are fitted and all 6,686 dry owners remain dry; none is unresolved in this manufactured monotone cohort. Maximum local and pair relative stock residuals are approximately `3.055e-15` and `1.543e-15`.

All 216 positive-threshold controls discover exactly one root. Maximum absolute position errors are `3.11e-15 m` at 1 mm, `2.66e-15 m` at 1 cm and `3.06e-13 m` at 5 cm. No reconstructed matched positive location is dry. At 1 mm the largest absolute flux error is `5.34e-17 m²/s` against `0.0001 m²/s`. This removes the earlier mean-power bias **in the manufactured affine-pressure family**, not in general evolving water.

Support has one discovered root in 56 cases and no root in all 16 phase-0/phase-1 boundary cases. Those fits put support infinitesimally beyond their wet owner's interval while the next owner is physically dry. No snapping is applied. In the wave cohort, 16 wet/dry faces retain a positive one-sided depth against a dry owner; the largest face depth/flux mismatches are about `6.59e-9 m` and `6.59e-10 m²/s`. A tiny pressure error near support can become a larger depth error under the `3/7` power. Positive-contour success is not complete support identification.

Two deliberately separate failures remain visible:

- **Smooth non-affine pressure:** `v=S*(6-x+0.025*x²)` on `[-4,4] m`. Independent composite Simpson integration with 4,096 panels supplies each positive mean. At `x=0`, neighboring fits have depth jump `+4.803831557e-5 m` and flux jump `-6.255163960e-5 m²/s`, despite small local/pair stock residuals. Different neighbor pairs represent different local shapes.
- **Tiny partial endpoint arithmetic:** means `[1,1e-30,0] m` at width 1 m pass their two-moment residuals, but `t` rounds to 1. The upstream owner's downstream pressure subtracts to zero, while the downstream owner retains a positive peak and about `1.42857e-9 m` face depth. The reflected test reverses flux and preserves the discrepancy. Stable pair moments alone do not provide a stable common endpoint representation.

The [validation record](data/surface-moment-reconstruction-validation.json) retains per-width/threshold coverage, missing support cases, the counterexamples and fingerprints. The complete report includes every owner and interior face. These errors must not be compared numerically with the earlier spherical observer errors as if geometry, inputs and resolution were matched.

## Decision and next gate

The stock-based scalar inverse passes the affine positive-contour representation control. **It is not selected as a production reconstruction.** Neighbor compatibility, tiny endpoint arithmetic, general stencil coverage and one shared conservative flux remain open. No seasonal, checkpoint, protocol, default, desktop or physical-law change follows.

Independent source review identifies two bounded next candidates, not implementations:

1. A first-order shared upwind face baseline using stock depths. [Manorost–Bastian, section 3.1](https://arxiv.org/html/2512.16525v1) supplies a related finite-volume construction; its gradient regularization and implicit nonnegativity conditions do not qualify our capped adaptation. Treating means as point depths also retains representation bias.
2. A reconstruction-compatible pressure face direction combining the mean reconstructed slope with a pressure-jump penalty, then choosing one donor depth. This is a proposed adaptation of that paper's section 3.2 direction/upwind structure, **not its published scheme or theorem**. Its coefficient, dry traces and refusal policy require explicit tests. Averaging the two measured fluxes is not an accuracy argument.

The uncapped flat law belongs to doubly nonlinear diffusion with diffusion parameters `p=3/2`, `m=13/3` (distinct from the integral exponent `p=10/7` above). [Andreianov–Bendahmane–Karlsen](https://arxiv.org/pdf/0901.0816) provide a relevant finite-volume framework, not a drop-in solver: their convergence theorem concerns their own discretization, and a depth-dependent mobility cap is not their gradient-only constitutive assumption.

Next, specify and measure one face operator's frozen cell-average time derivative against the exact wave, including dry arrival and smooth non-affine controls. Only then attempt short paired planar evolution with stock accounting and spatial/time refinement. Donor-budget time-step bounds protect inventory algebraically, not diffusion stability. Triangle/spherical geometry, variable bed, barriers, initial sample-stock compatibility and seasonal integration remain subsequent gates. Do not rerun the expensive sphere merely because these local fits work.

## Reproduction

```bash
cargo run --locked --manifest-path native/Cargo.toml --example surface_moment_reconstruction_report -- artifacts/new-moment-reconstruction.json
cargo test --locked --manifest-path native/Cargo.toml --lib moment
```

The standalone CLI requires exactly one new output path, computes the complete bounded report before file creation, and refuses overwrite/extra arguments. Its version is `surface-moment-reconstruction-report-1`, not a model/checkpoint selector. Six new directed tests cover full/partial/nearly uniform inversion, dry/invalid/ambiguous inputs, complete 72-case repeat and contour coverage, reflected slopes/fluxes, non-affine face mismatch and tiny partial endpoint mismatch. The existing polynomial-moment test also matches the `moment` filter. Final regression details and complete report repeat are recorded separately. No GUI or new evolving-front run is claimed.

The final source passes all 144 optimized library tests, seven `moment`-filtered tests, strict all-target Clippy, formatting and whitespace checks. Three CLI argument/overwrite refusals preserve existing output and leave invalid output absent. The complete 11,474,860-byte report repeats byte-for-byte and also matches the optimized example output, with SHA-256 `9f0af5e4bfcac7e49500bc166f3dc1a6dd05ad9e2e7c308b366a1c335b3f21ad`. The previous planar report still has its original SHA-256 `1fe17f60c05faa09840977f9e627f1b1fdfa4fde8be992b9da6539655bd354ea`. Its helper visibility was broadened only within this private module, and an excessive-precision test literal was normalized to the identical `f64` value for current Clippy. Pre-existing renderer edits remain untouched and excluded.

Full `npm test` passes 598 native tests across 111 targets, with the same one pre-existing ignored test, and all eighteen Node files (88 tests). That run compiled 143 library tests before the last report-only tiny-partial control, its directed/reflected/plateau assertions and non-isolated-root documentation. Moment inference and constitutive arithmetic were unchanged; the final-source optimized and directed runs above include the additional test and assertions. No failed physical trial is converted into a successful accuracy qualification.
