# Moving affine-bed reference and method review

October 13, 2026. This review supports the next moving-water test after the [affine-bed storage/rest baseline](surface-topography.md). It does not select a production scheme or assert an accuracy result. Published methods and the project-derived reference below are deliberately separate.

## What the primary sources actually establish

The Manorost–Bastian source below is an arXiv preprint; citing it is not a claim
of peer-reviewed or application-specific validation.

[Manorost–Bastian, sections 2, 3.1 and 4.1](https://arxiv.org/html/2512.16525v1) gives a scalar diffusive-wave model, an implicit upwind finite-volume method, and a separate cut-cell DG method. The finite-volume nonnegativity result has specific hypotheses, including existence of the discrete solution. Its inclined-plane Barenblatt accuracy experiment sets `alpha=gamma=1`, not the Manning exponents. Its implemented cut-cell DG scheme also uses a positive initial depth in nominally dry cells and small-depth flux regularization. Therefore neither its second-order experiment nor its positivity theorem automatically qualifies our capped, explicit, no-film stock transaction. Its numerical comparison still provides a useful warning: an upwind finite-volume flux can be only first-order even when its directional gradient has a second-order consistency expression.

[HEC-RAS, Diffusive Wave Equation](https://www.hec.usace.army.mil/confluence/hecras/beta/technical-reference/hydraulic-equations/diffusive-wave-equation) describes gravity/friction-dominated flow with the hydraulic surface slope driving Manning velocity. It identifies near-flat-surface stiffness and the accuracy tradeoff of slope regularization. It excludes inertia-dependent effects such as rapid dam breaks and hydraulic transitions. This supports the present model scope, not a general shallow-water solver claim.

[HEC-RAS, Hydraulic Property Tables](https://www.hec.usace.army.mil/confluence/rasdocs/hecras/beta/technical-reference/hydraulic-property-tables) separates cell volume/wetted-area relationships from face conveyance geometry. This is a practical precedent for keeping accepted water ownership separate from the geometry used to calculate flow. It does not validate our specific affine inverse or moving front.

## An exact downhill moving reference derived for this project

Use a fixed one-dimensional bed `b(x)=b0-S*x`, with `S>0`, roughness `n>0`, speed `c>0`, and no physical sources. Write

```text
z = x - c*t
h(x,t) = H(z)
eta(x,t) = b(x) + h(x,t)
q(x,t) = c*H(z)
A = n^2*c^2
```

For the uncapped original law

```text
q = -sign(eta_x) * h^(5/3) * sqrt(abs(eta_x)) / n
```

choose the positive profile satisfying

```text
H' = S - A*H^(-4/3)
0 < H < D
D = (A/S)^(3/4)
```

Then `eta_x=-A*H^(-4/3)<0`, and direct substitution gives `q=c*H`. Continuity is exactly `h_t+q_x=-c*H'+c*H'=0`. At the front set `H=0` and `q=0`. Both traces vanish, so extending by zero ahead of the front introduces no nonzero jump in the conserved height or flux. The derivative singularity at the front is integrable. This is a project derivation checked by substitution, not a theorem borrowed from the papers above.

If the front is `xf(t)=xf0+c*t`, its distance behind the front is determined monotonically by

```text
xf(t)-x = F(H)
F(H) = integral_0^H u^(4/3)/(A-S*u^(4/3)) du
```

The denominator must remain positive. Do not evaluate through the normal-depth singularity or silently clamp to `D`. For a wet interval whose left/right profile depths are `HL>=HR`, its physical stock is

```text
integral_interval h dx = J(HL)-J(HR)
J(H) = integral_0^H u^(7/3)/(A-S*u^(4/3)) du
```

Clip the *integration domain* at the analytic support, not accepted simulation stock. An entirely dry reference interval has exactly zero stock. On a partial interval, use the wet endpoint and `HR=0`. These expressions supply integrated initialization and comparisons, rather than treating point samples as physical owner means.

The project cap is inactive only if the actual hydraulic-gradient branch satisfies

```text
q/abs(eta_x) = c*H^(7/3)/A < cap
```

This ratio increases with positive `H`, so a independently bounded maximum depth suffices. The depth gradient `H'` is not the hydraulic gradient; replacing the latter by `abs(H')` would give the wrong cap test. Near the front, `H~[(7*A/3)*(xf-x)]^(3/7)`, which also checks the flat-bed limit as `S` tends to zero.

The reference is a bounded open-strip experiment with exact exterior flux `q=c*H`. It is not a closed planetary solution, a finite-mass solitary pulse, or an inertial flood wave. The unbounded upstream continuation tends to the normal depth `D`. The numerical operator may receive initial integrated means and prescribed exterior interval flux integrals; the interior must not receive the exact front, depth, future stock, or derivative.

## Minimal next method and honest qualification

First test the already separate head/storage baseline against this moving reference, with independent time and spatial refinement. Keep the original donor-stock budgets and compensated paired transactions. Report full integrated-stock error, boundary exchange, and support observations independently. A successful time comparison alone is not spatial accuracy; a conserved total alone is not correct flow. Missing or nonunique contour observations remain missing, not snapped to the known front.

If the baseline does not meet its declared moving accuracy target, use the failure to design a named stock-consistent bed-aware reconstruction. Do not add bed slope to the flat-depth-pressure stencil and assume it remains balanced, and do not quietly substitute the rest baseline as a fallback for pressure extrema. A higher-order scheme must preserve both the supplied stock and resting head; its slope, partial-wetting geometry, and face gradient must belong to one explicit closure. A full DG/nonlinear-solver rewrite is not required before measuring the current baseline.

## Dry ordering audit

The baseline's dry minimum-bed surrogate agrees with the zero-positive-stock limit of its constant-head affine hypsometry. Using the face bed instead would break that limit on a sloping receiver. This supports continuity of the chosen *local closure*, not a physical dry water level or proof of front accuracy. One head per owner assumes instantaneous local equilibration; on a falling receiver, newly accepted stock occupies the low end under that closure even if entry occurred at the high end. That unresolved intracell transient is precisely why a moving benchmark is needed. Face crests still prevent a donor below the represented barrier from transferring water.

General bed extrema/plateaus, disconnected subcell water, two-dimensional full-gradient friction, spherical assembly, seasonal coupling, and runtime remain outside this one-dimensional affine moving reference. Source methods, reference mathematics, and implementation evidence must not be collapsed into one claim of completion.

## Implementation follow-up

The [moving-bed comparison](surface-moving-bed.md) now implements the reference
and a separately named supported affine-head hypothesis. Its measured improvement
and remaining contour/geometry limitations belong to that record, not the cited
source's theorem or accuracy experiments.
