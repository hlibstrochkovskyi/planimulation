# Variable-bed head and face-crest source review

October 13, 2026. This records the source review for the [bounded head/storage laboratory](surface-topography.md) after the [non-affine evolving gate](surface-non-affine-evolution.md). It does not select a production operator or extend the qualified flat-pressure candidate. No variable-bed accuracy result is implied by this source review.

## Directly relevant primary sources

[Manorost–Bastian, section 3.1, equations 6–8](https://arxiv.org/html/2512.16525v1) supplies a diffusive-wave, not full momentum, finite-volume reference. It represents bed and head by cell constants. Its interface bed is the maximum of the two **cell-center bed values**; upwind depth is the positive part of the larger head minus that interface bed. Its friction gradient is regularized, and its time discretization is implicit. The nonnegativity theorem assumes the specified scheme, homogeneous flux boundaries, no sources/sinks and existence of a discrete solution. These conditions do not prove properties of our explicit capped adaptation.

[HEC-RAS hydraulic property tables](https://www.hec.usace.army.mil/confluence/rasdocs/hecras/beta/technical-reference/hydraulic-property-tables) provides an official diffusive-wave implementation precedent for separate cell volume/wetted-area and face area/conveyance relationships with water-surface elevation. This supports separating storage geometry from transfer geometry. It is not a validation of our one-dimensional formula or explicit transaction.

## Minimal laboratory adaptation

The proposed geometric closure keeps accepted integrated depth stocks as physical state. A positive cell volume determines one constant head `eta` by

```text
V = integral_cell max(eta-bed(x),0) dx
```

For a monotone affine bed, this integral has an explicit partial/full-wetting inverse. In a partially wet owner, `meanDepth+meanBed` is generally not that inverse. A zero stock is `Dry`, with no inferred physical head. One head per owner assumes finite-cell local hydrostatic equilibration; it does not resolve intracell transient head variation.

At an interface, use the maximum of the two **adjacent affine-bed endpoint values**, not their center values. This is a deliberate geometric adaptation of the paper's crest construction. For two wet owners of equal width:

```text
crest = max(leftBedAtFace,rightBedAtFace)
g = (etaRight-etaLeft)/width
donor = left if g<0, right if g>0
hFace = max(etaDonor-crest,0)
q = -sign(g)*min(cap*abs(g), hFace^(5/3)*sqrt(abs(g))/n)
```

Return zero before positive-branch arithmetic when drive or donor depth is zero. For gradient ordering only, a dry owner uses its minimum bed as a numerical surrogate, with zero depth offset. The physical column remains `Dry` with no head and cannot donate. This convention preserves the positive-volume limit: the inferred wet head tends to the minimum bed as volume tends to zero. Using the face bed instead would introduce a drive discontinuity on a sloping receiver. The interface crest still determines donor wetting, so no transfer occurs below it; both dry owners have zero transfer. The surrogate is a project closure, not an extra physical water level or the paper's affine-bed rule.

Positive-part depth expresses wetted geometry, not a positive film or a correction to owned stock. Retain representability refusal, one signed face amount, complete old-stock donor budgets and compensated paired transactions. No epsilon gradient, legacy drive clipping or silently substituted operator is borrowed.

## Why the flat-pressure inference is insufficient

The existing pressure is `v=h^(7/3)`, whereas physical water-surface head is `eta=bed+h`. On positive depth,

```text
eta_x = bed_x + (3/7)*v^(-4/7)*v_x
```

A resting lake has constant head but generally nonconstant depth pressure. Its depth can have extrema or mixed plateaus that the current monotone moment stencil refuses. Adding bed slope to the existing flat face law does not make its inferred profile a stock-consistent resting-lake reconstruction. The geometric baseline remains a separately named hypothesis, not a fallback inside that candidate.

## Bounded evidence and remaining limitations

The immediate controls may check repeated closed rest and below-crest blocking, frozen positive over-crest direction/transfer, reflection and local/global ownership ledgers. Include rising and falling dry receivers alongside tiny positive-volume receivers to expose the minimum-bed ordering convention and its zero-volume limit. A flat, fully wet two-owner control with an independently verified active cap has `delta_t=-2*cap*delta/width^2`; its exponential solution checks time integration, not variable-bed PDE accuracy. No exact evolving variable-bed solution, spatial order or general stability follows from these controls.

Use monotone affine bed within each owner and put ridge crests at interfaces. One head per owner cannot represent independently filled subcell pools separated by an internal ridge. Endpoint geometry cannot infer an unrepresented interior barrier; such topology requires a different partition or explicit connected-component representation.

Floating inverse geometry can recover slightly different rounded heads from manufactured equal-head stocks. Exact representable rest fixtures and general rounding observations must remain distinct. Do not introduce head snapping, a zero-gradient tolerance, state correction or a film to conceal that limitation. Donor-budget safety is not a diffusion stability theorem. General wetting on variable bed, shoreline accuracy, extrema, spherical assembly, seasonal coupling and production runtime remain separate gates.
