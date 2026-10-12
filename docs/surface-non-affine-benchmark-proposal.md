# Proposed non-affine evolving benchmark

October 12, 2026. This records the original proposal following the [bounded full-cell gate](surface-full-cell-propagation.md). The [subsequent implemented reference and evolving laboratory](surface-non-affine-evolution.md) now exercise this cohort and record its bounded qualification and remaining limitations. The proposal preserves the current capped physical law rather than proposing a new solver.

## Why another exact family

The clipped-affine traveling wave is semidiscrete-exact under its qualified stencils and inactive cap. It tests temporal integration and ownership but cannot measure nonzero spatial truncation error. A genuinely curved pressure profile with independent analytic evolution can expose that remaining error.

[Audrito–Vázquez, Section 2](https://arxiv.org/pdf/1601.05718) describes the compact-support Barenblatt family for pure doubly nonlinear diffusion, independently of the reaction problem studied elsewhere in that paper. The uncapped flat project law maps to `h_t=kappa*Delta_p(h^m)` with `p=3/2`, `m=13/3`, `kappa=1/(n*sqrt(m))` and slow-diffusion parameter `m*(p-1)-1=7/6`. The one-dimensional source similarity exponent is `3/8`. This provides a family to investigate, not a stability theorem for our reconstruction. The physical coefficient below is derived directly instead of importing a differently normalized coefficient.

## Direct physical-law derivation

For positive normal coordinate `x`, define

```text
alpha = 3/7
b = 3/8
k = 7*n²/64
tau = t+t0 > 0
R(tau) = R0*(tau/t0)^b
v = (k/tau²)*(R³-x³)
h = max(v,0)^alpha
q = b*x*h/tau
```

Wet pressure has `v_x=-3*k*x²/tau²`. The uncapped Manning pressure flux gives `q/h=sqrt(3*alpha*k)*x/(n*tau)=b*x/tau`, fixing `k` in the original physical law. Also `v_tau+b*x*v_x/tau=-(7/8)*v/tau`. Hence `h_tau+b*(h+x*h_x)/tau=0`, because `alpha*(7/8)=b`. This verifies continuity on wet support. Both depth and flux vanish at the front; their singular derivatives are locally integrable. The oracle must verify these identities independently in code rather than infer truth using reconstructed moments.

Pressure is cubic, not affine. The full symmetric solution has an extremum at zero, where the cap changes the uncapped fixture. Restricting to a positive monotone half-profile avoids this issue without claiming qualification of extrema or of a capped whole-line solution.

## Bounded candidate, not a selected cohort yet

Use roughness `.04`, cap `100 m²/s`, age `t0=10 s`, initial front `R0=10 m`, domain `[2,14] m` and duration `1 s`. The front moves to about `10.363877 m`. All existing support/1 mm/1 cm/5 cm contours remain inside this domain.

Exact wet diffusivity is `beta=7*(R³-x³)/(24*tau*x)`. The maximum is at the left boundary and remains about `14.47–14.65 m²/s`, below the cap. This checks applicability of the exact family, not explicit reconstruction stability. A conservative bound using maximum `R` and minimum age is below `16.24 m²/s`.

Candidate widths `1/.5/.25 m` give 12/24/48 cells. Three common temporal steps `.001/.0005/.00025 s` permit matched time controls with at most 4,000 steps per trial. Fix the final cohort/work limits before evolving it. An additional predeclared wet-interior window `[3,9]` could localize boundary/front errors, but full-domain results must remain primary. A fixed cadence or apparent scalar error ratio alone must not be reported as spatial order.

## Independent integrated-state oracle first

Do not initialize using center samples. For each analytic cell integral, restrict its integration interval to wet support. Factor `R³-x³=(R-x)*(R²+R*x+x²)` to avoid cancellation. The change of variable `R-x=z^7` gives

```text
h*abs(dx) = 7*(k/tau²)^alpha*z^9*(R²+R*x+x²)^alpha*dz
x = R-z^7
```

The transformed integrand is smooth at the support endpoint. A bounded quadrature can compare successive refinement levels, but agreement alone is not a certified error bound. Before evolution, independently check integrated means against a distinct higher-precision integral/algorithm, continuity-based cell derivatives, conservative restriction and reflected geometry. Record the measured oracle floor separately from solver errors. No analytic pressure or slope may enter interior inference; exact flux enters only exterior prescriptions and observers.

Only after that oracle gate should the unchanged transaction evolve this cohort. Retain all owned high/low components, boundary ledgers, full-domain errors, discovered contours, arrival traces and last accepted refusal state. Cubic-pressure moments can have incompatible face traces or evolve into unidentified stencils even from monotone exact data. Such failure is a result to diagnose, not permission for a film, floor, support snap, fallback or tuned state correction.
