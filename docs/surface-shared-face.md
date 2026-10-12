# Frozen shared-face flux and single-step accounting

October 12, 2026. This follows [adjacent-moment reconstruction](surface-moment-reconstruction.md). The new private native experiment computes **one flux per interior face**, observes its cell-average divergence, and applies independently initialized frozen Euler transfers. It is not a multi-step trajectory, a native spherical operator, a selected seasonal model or a moving-front qualification. Product principles and physical parameters remain unchanged.

## Operator hypotheses

The first-order comparison uses cell means as depths, with `g=(meanRight-meanLeft)/width` and upwind depth `hUp=max(meanLeft,meanRight)` on this flat domain:

```text
q = -sign(g)*min(Dmax*abs(g), hUp^(5/3)*sqrt(abs(g))/n)
```

This is a conservative baseline, not an unbiased reconstruction of physical face depth. The related finite-volume construction in [Manorost–Bastian, section 3.1](https://arxiv.org/html/2512.16525v1) uses different geometry, gradient regularization and implicit conditions; its nonnegativity theorem does not qualify this explicit capped adaptation.

The pressure hypothesis uses the existing all-cell stock inference, with physical traces `vLeft=max(rawPressureLeft,0)`, `vRight=max(rawPressureRight,0)` and reconstructed pressure slopes. Use zero slope where the physical trace is dry. Define:

```text
alpha = 3/7
K = sqrt(alpha)/n
G = (sLeft+sRight)/2 + sigma*(vRight-vLeft)/width
donor = left if G<0, right if G>0
hUp = vDonor^alpha
q = -sign(G)*min(alpha*Dmax*hUp*abs(G)/vDonor,
                 K*hUp*sqrt(abs(G)))
```

For zero `G` or zero donor pressure, flux is zero before division. A dry donor does not borrow a wet receiver's trace. Any unresolved owner refuses the complete reconstructed operator; the stock baseline is a separately reported method, not an implicit fallback. Geometry, parameters and observer shapes are validated. Nonfinite or underflowed positive flux branches refuse, including a `0*infinity` intermediate that could otherwise become `NaN` and silently disappear through floating-point `min`: [Rust documents its single-NaN behavior](https://doc.rust-lang.org/std/primitive.f64.html#method.min).

This face direction/upwind structure is inspired by [the same paper's section 3.2](https://arxiv.org/html/2512.16525v1), **not its published DG scheme**. In particular, its friction denominator uses the unpenalized averaged gradient, whereas our hypothesis evaluates the original flat pressure law at the penalized `G`. No epsilon gradient, positive film, cut-cell evolution or theorem is imported. Continuous positive traces and equal slopes reduce to the original capped law; directed tests check both cap regimes against its independent depth-gradient expression.

Penalties 0/1/2 are all retained. An independent smooth-wet monotone expansion motivates 1: away from zero gradient and support, the upstream-pair stencils give `G=v'(x)+(sigma-1)*(width/2)*v''(x)+O(width²)`. This is a local consistency argument, not a positivity/stability proof or an optimum fitted to final errors. A manufactured affine pressure cannot distinguish these penalties.

## Inputs, derivative and boundary contract

The wave cohort retains 72 combinations of widths 4/2/1/0.5 m, nine front phases and reflected signs. Exact integrated means are unchanged. Roughness is `0.04`, cap is `100 m²/s`, and exact wave speed is `0.1 m/s`. Its cap remains inactive; the uncapped exact traveling wave would not be an exact reference if its cap were deliberately activated.

Interior flux construction receives stocks, width, physical parameters and the fixed penalty only. Exact values enter the manufactured inputs, the **two exterior prescribed fluxes**, and subsequent observers, never an interior gradient, donor, stencil or fitted coefficient. With global signed fluxes:

```text
meanRate[i] = (q[i]-q[i+1])/width
sum(width*meanRate) = qDomainLeft-qDomainRight
```

Compare rates with the exact analytic face-flux difference, not a finite difference of future means. All cells remain in global metrics; maximum dry-owner rates are separate from RMS diluted by dry cells. Missing support roots and raw endpoint arithmetic from the preceding reconstruction are not snapped away.

The smooth control retains `v=S*(6-x+0.025*x²)` on `[-4,4] m`, five widths 1/.5/.25/.125/.0625 m, and independent Simpson integration with 4,096 panels per mean. Exact instantaneous flux uses the original depth-gradient law. The full domain and a separately declared fixed interior window `[-2,2] m` are both observed. No future analytic solution is claimed for this control. Closed uniform, dry, tiny-partial and ambiguous-stencil controls have no fabricated interior truth: their accuracy-comparison fields are null.

## Frozen single-step accounting

For each method/case, independently initialize two Euler steps of .001/.0005 s. These are **not consecutive steps**. Fluxes, including exterior fluxes, remain frozen at their initial values. Future-wave comparison therefore includes Euler/boundary-time truncation; exterior forcing is not secretly replaced with the exact time-integrated flux.

Per-unit-breadth initial stock is `mean*width`. The existing normalized two-component stock retains sub-ULP amounts. One volume `qFace*dt` is computed once and applied with opposite signs to its owners. Before any credit, each owner's complete old-stock outgoing budget includes both interior and exterior outflows. No incoming credit finances that test. Requests are not resized; any representability, overdraw, positivity or local/global ledger failure discards the local proposed result and leaves inputs unchanged.

Accounting is checked locally and globally under a separate laboratory `256*epsilon` relative arithmetic limit, without changing native audit tolerances. Boundary inputs and outputs are explicit, with both signs tested. A test transfers one unit between stocks `2^54`: high components remain unchanged and low components become `[-1,+1]`. A two-outflow overdraw refuses even when each individual face request would fit. Uniform/dry closed steps preserve their actual stocks, and reflection reverses flux signs while reversing rates and stepped owners.

## Results and limits

All 288 wave method/cases have two accepted, accounted steps: 576 independent one-step results. For `sigma=1`, maximum local/global relative ledger residuals are `1.10e-16`/`2.29e-17`; divergence telescopes to zero in this same-build cohort. That proves these transfers' accounting, not evolving accuracy.

The stock baseline has maximum instantaneous mean-rate error `1.327e-3 m/s`, including premature dry-owner input. The pressure hypothesis has maximum `4.756e-10 m/s`, dominated by the retained aligned-interface roundoff. Its maximum initially dry-owner rate is likewise `4.756e-10 m/s`, not identically zero. Ordinary wet faces are substantially more accurate, but no roundoff correction or support snapping is applied.

Smooth `sigma=1` results:

| Width, m | Signed relative flux error at x=0 | Interior rate RMS, m/s | Full-domain rate RMS, m/s |
| ---: | ---: | ---: | ---: |
| 1 | -1.177e-3 | 2.415e-7 | 2.031e-6 |
| .5 | -2.957e-4 | 6.098e-8 | 7.328e-7 |
| .25 | -7.416e-5 | 1.535e-8 | 2.630e-7 |
| .125 | -1.857e-5 | 3.853e-9 | 9.383e-8 |
| .0625 | -4.646e-6 | 9.652e-10 | 3.334e-8 |

Interior rate RMS and fixed interior face error decrease by approximately four per width halving. Full-domain RMS ratios are only 2.77–2.81 (roughly order 1.5); global maximum ratios are 1.87–1.98 (roughly order 1). Exact imposed exterior flux and the boundary owner pair leave weaker boundary/stencil derivative errors. Those cells remain in the global results. Stock baseline and penalties 0/2 are retained; do not advertise global second order or extrapolate these smooth observations to support, extrema or variable bed.

Two important negative controls remain:

- Tiny partial means `[1,1e-30,0]` still have disagreeing face traces. With `sigma=1`, the selected left donor trace is zero, so the shared flux is zero despite a positive opposite trace. The reflected control behaves analogously. One specified flux removes duplicate transfers; it does not repair endpoint reconstruction.
- At width 1, phase 0 and `dt=.001 s`, exact arrival into initially dry owners is `1.71056e-8 m²` while computed arrival is only `2.90837e-13 m²` from the raw roundoff trace. Halving time gives exact arrival `6.35472e-9 m²`. Correct instantaneous dry derivative can coexist with positive future arrival of order `dt^(10/7)`; this almost-missing first-step arrival is explicitly retained, not a successful wetting test or universal quadratic time error.

The [validation record](data/surface-shared-face-validation.json) retains per-width/method maxima, both time steps, all smooth refinement comparisons, controls and fingerprints. Complete stock/rate/flux arrays remain in the ignored report.

## Decision and next gate

The `sigma=1` hypothesis is suitable for a **bounded short planar multi-step trial**, not production selection. Frozen affine derivative consistency, smooth interior improvement and one-step accounting are supported. Boundary treatment, tiny endpoint blocking, general stencil coverage and moving-front/time accuracy remain open. Donor-budget safety is not a diffusion stability certificate.

Next run a small, fixed moving-wave cohort with spatial/time refinement, rebuilding stocks and fluxes each step, recording any unresolved reconstruction or numerical refusal and comparing actual dry arrival and contours. Do not fit a final state, inject a film, silently fall back, or change roughness to hide an error. Triangle/spherical reconstruction, variable bed, barriers, initial sample-stock compatibility, seasonal coupling and calibration remain later gates. No expensive spherical rerun is justified yet.

## Reproduction and validation

```bash
cargo run --locked --manifest-path native/Cargo.toml --example surface_shared_face_report -- artifacts/new-shared-face.json
cargo test --locked --manifest-path native/Cargo.toml --lib surface_front::face
```

The CLI requires exactly one new output path, refuses overwrite/extra arguments, and computes the complete bounded report before creating the output. `surface-shared-face-report-1` is a laboratory report version, not a model/save pin. Seven new tests cover cap/zero/gradient-jump identity, extreme arithmetic refusal, boundary signs, sub-ULP paired transfer, aggregate donor refusal, reflected rates/stocks, unchanged uniform/dry inventories, all 72 cases and 576 steps, smooth interior/global distinction, tiny blocking and unresolved/invalid observer shapes. Validation results are recorded separately; no GUI or new evolving-front run is claimed.

The final source passes all 151 optimized library tests, twelve regional-soil and five regional-surface integration tests, all seven directed tests, strict all-target Clippy, typecheck, release native build, and all eighteen Node files (88 tests) against the rebuilt binary. Formatting and whitespace checks pass. This increment does not rerun the entire `npm test` suite; the previous full-suite evidence and its source delta remain in the preceding validation record. Three CLI refusal checks preserve existing output and leave invalid extra-argument output absent.

The complete 28,439,953-byte report repeats byte-for-byte and matches optimized output with SHA-256 `74ef32915c5a922c79fd7b1300c522d028f63e653b7182a1ef3bf95604044016`. Rebuilding the previous moment report preserves its SHA-256 `9f0af5e4bfcac7e49500bc166f3dc1a6dd05ad9e2e7c308b366a1c335b3f21ad`. Pre-existing renderer edits remain untouched and excluded.
