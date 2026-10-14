# Next non-affine-bed conveyance benchmark

October 14, 2026. **Proposal only: not implemented or qualified.** This is a
finite next geometry check after the [moving affine-bed comparison](surface-moving-bed.md),
not another production model, an application feature, or a replacement for the
remaining advancing/receding-front and spherical gates.

## Source-free exact steady flow over a bed crest

Choose positive depth `h(x)=H0+a*x` on `[0,L]`, `a!=0`, and a constant rightward
discharge `Q>0`. For original uncapped Manning flow with roughness `n`, define

```text
A = n^2*Q^2
b(x) = -a*x + 3*A/(7*a) * (h(x)^(-7/3) - H0^(-7/3))
eta(x) = b(x) + h(x)
b'(x) = -a - A*h(x)^(-10/3)
eta'(x) = -A*h(x)^(-10/3)
```

Then `b(0)=0`, Manning flux is exactly `Q`, and `h_t+q_x=0` without interior
sources. Water moves while the profile remains steady. This is a project
derivation checked by differentiation and direct substitution into the law
reviewed in [the source record](surface-moving-bed-source-review.md).

Proposed fixed parameters:

| Quantity | Value |
| --- | ---: |
| `H0` | `.025 m` |
| `a` | `-.005` |
| `L` | `4 m` |
| `n` | `.04` |
| `Q` | `.002 m²/s` |
| cap | `100 m²/s` |
| end time | `.1 s` |
| owner widths | `.5/.25/.125/.0625 m` |
| candidate time controls | `.001/.0005 s` |

Depth stays in `[.005,.025] m`. The reference bed has one crest near
`x=1.58655439484 m`, with elevation `.00361998631 m`; hydraulic head nevertheless
decreases everywhere. The crest formula is
`hCrest=(A/abs(a))^(3/10)` and `xCrest=(hCrest-H0)/a`.

The analytic maximum mobility is `H0^(10/3)/(n^2*Q)`, approximately
`1.427743 m²/s`, below the cap. A continuum linearized estimate gives
`D*dt/dx²≈.18275` at the finest width and larger proposed step. This only sizes
the experiment; it does not prove stability or positivity of either explicit
discrete reconstruction. Refusals must remain recorded.

## Numerical contract and finite decision

Supply exact initial cell means `h(center)` and actual bed endpoint samples to
both named methods. Never supply exact head, gradient, slope or future interior
stock. Endpoint-interpolated affine owner beds approximate the nonlinear
reference terrain; their representation error belongs to spatial refinement.

Use exact prescribed boundary flux `Q` at both ends with the unchanged stock
transaction. Input and output counters each equal `Q*T`, not zero: their net
cancellation must not conceal either counter's error. Maintain the existing
owner, step and cell-step work bounds and the original physical moment/ledger
criteria.

Before evolution, measure:

- Interior face discharge defects against `Q`.
- Cell-average divergence defects against zero.
- Bed interpolation error and represented crest height/location.
- The actual active cap branch and donor budgets.

Then run the short matched trajectories, comparing integrated owner means to
the stationary exact profile. Separate time and mesh effects; retain incomplete
trajectories instead of comparing their last state at unequal final times.
Independently verify constants, stock means, crest and original flux at high
precision before using this reference for a decision.

The decision is whether both the frozen spatial defect and evolved field error
improve under the prescribed terrain/mesh refinement, with conserved ownership
and a smaller temporal effect. A failure identifies a concrete geometry or
conveyance problem; it does not justify fitting stock or changing the physical
law to the fixture. A success establishes only this bounded fully wet
non-affine-bed witness.

## Explicit limits

The true crest lies inside an owner at the proposed widths. This is fully wet
conveyance, **not** proof of an unresolved dry barrier or intracell disconnected
pools. A short steady-profile test does not establish long-time stability,
arbitrary bed topology, moving shoreline accuracy, spherical full-gradient
assembly, calibration, seasonal coupling or acceptable production runtime.
