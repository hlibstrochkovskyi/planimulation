# Spherical patch triangular-facet routing

October 15, 2026. Native `spherical-patch-facet-routing-1`, read-only observer
`fine-routing-review-1`. This is a direction/area component for the
[authoritative fine-patch review](fine-terrain-patch.md), **not a coupled erosion
model**, new World family, water solver or completed R3 milestone.

## Model and physical geometry

The [TauDEM D-infinity description](https://hydrology.usu.edu/taudem/taudem5/help/DInfinityFlowDirections.html)
motivates reconstructing descent on triangular facets and partitioning flow
between neighboring directions. We independently implement a bounded spherical
patch adaptation, not external source code or an exact reproduction of TauDEM.
In particular, its usual pit-removal workflow and flat-area procedure are not
imported here.

The existing patch still has eight adjacent routing neighbors. For each active
cell, the router constructs local tangent displacements from initial great-circle
bearings and physical great-circle lengths. Consecutive neighbors define eight
acute facets. Solve the two height-drop equations for the facet's downhill
gradient. A gradient inside the facet cone is a candidate; otherwise the
steepest positive edge remains available. Choose the greatest positive slope,
with fixed iteration order for exact ties. This is a local tangent, piecewise
linear approximation, not an exact continuous spherical flow solution.

A facet candidate requires both selected neighbors to be strictly lower than
the donor. No positive descent invokes the pinned existing physical-flat rule:
equal-height flats reach real equal/lower exits, while enclosed flats retain a
canonical dry terminal. Fixed external collar cells remain terminals. No pit
is filled, breached or assigned a fictitious water outlet.

For a selected two-receiver direction, return two different sets of coefficients:

- Angular **area weights**, proportional to the angular distance from the other
  receiver direction, partition static contributing area.
- Positive **directional-derivative coefficients**, units m⁻¹, satisfy
  `unit_descent = alpha_a * displacement_a + alpha_b * displacement_b`.
  Consequently `slope = sum(alpha_j * (h_i - h_j))` on that facet.

Those coefficients are not interchangeable. A direction vector reconstructed
between grid axes does not imply an off-grid physical node or exact continuum
transport by its discrete branches. Numerical dispersion and spatial convergence
remain separate obligations.

The router owns a copy of bounded builder geometry and cached metrics. It does
not depend on a camera or consume a random stream. It checks settings, dimensions,
unit directions and acute/nondegenerate facets; this is not authentication of
arbitrary caller-modified coordinates against a complete generation recipe.
Geometry remains bounded at 262,144 cells. Each query currently also computes
the old route to reuse its flat handling: that overhead is not hidden.

## Area diagnostics, not conserved water inventory

Only active reference areas enter accumulation. Each branch receives the donor's
accumulated area times its angular weight, in an iterative topological order.
Terminals reconcile with total active area at `1e-12` relative. Directed cycles
refuse. Compensated sums reduce aggregation error; no area correction changes
the output. This diagnostic tolerance is not a seasonal-water conservation rule.

A cell can contribute fractionally to several external/closed terminals.
There is therefore no unique outlet ID or binary closed-catchment label. The
reported closed fraction is total area collected by genuine dry terminals
divided by active area. It is not lake coverage, water volume or discharge.
Neither bed nor water/material stocks advance.

## Read-only retained-bed comparison

```sh
cargo run --locked --release --manifest-path native/Cargo.toml --example fine_routing -- artifacts/fine-landscape-qualified/generated-200.json artifacts/fine-routing-new
```

Arguments are a retained row JSON, a new output directory, and optional
`incision` or `coupled` variant (default `coupled`). The observer checks matching
row/manifest metadata, exact six-column f64-LE format, local field filenames,
finite values and record length. Inputs are byte-bounded and the observer has
a separate 40,000-cell cap. Unknown formats, oversized inputs and existing output
directories refuse; successful comparison JSON is written last.

Both SVG panels show **identical retained physical heights**. Cyan links display
accumulated area above 5 km²; amber segments display directions, using declared
discrete fallback on flats. The old bed's aligned valleys are not expected to
disappear when only routing is recalculated. JSON retains full new graph/area
records, resolved geometry, source identity, diagnostic input FNV fingerprint
and stage times. The fingerprint is not security authentication or a checkpoint.

Six local retained comparisons:

| Existing physical bed | Cells | Old closed fraction | Facet closed fraction | Two-receiver nodes |
| --- | ---: | ---: | ---: | ---: |
| Generated, D=1 | 2,500 | 0% | 0% | 1,837 |
| Generated, D=1 | 10,000 | 0% | 0% | 8,793 |
| Generated, D=1 | 40,000 | 0% | 0% | 38,055 |
| Synthetic, D=1 | 40,000 | 4.79% | 4.73% | 37,542 |
| Generated, no diffusion | 40,000 | 0% | 0% | 15,998 |
| Generated, D=0.01 | 40,000 | 43.33% | 43.50% | 21,668 |

Artifacts are `artifacts/fine-facet-routing-{coarse,medium,generated,synthetic,incision,low-diffusion}`;
`fine-facet-routing-repeat` exactly repeats the generated graph/report except
declared timings. All 24 source physical records remain unchanged during the
five additional-case batch. Generated 250 m direction change is area-weighted
RMS 7.75°, max 22.50°: **change is not an accuracy oracle** on arbitrary terrain.
Terminal-area residuals in the six cases are at most `3.9e-16` relative.

One generated 40,000-cell sample takes about 29 ms for cached metric construction
and 17 ms for routing, versus 14 ms for the old route. Other workloads overlap;
these are stage observations, not isolated laptop or desktop benchmarks. The
report includes memory for neither the entire application nor future coupling.

## Tests and erosion integration gate

624 analytic tangent-plane cases cover three spacings, four chart orientations,
both poles, opposite equatorial locations and thirteen gradient directions.
Central unit-vector error is below `1e-10`, slope error below `1e-11`. This
removes the pinned 22° plane witness's directional quantization, not every source
of grid dependence. Separate tests cover angular weights versus derivative
coefficients, explicit flat exits/closed flats, adjacent downhill acyclic
branches, independent terminal-area and fractional-ancestry reconciliation,
repeatability, owned geometry, malformed shapes and extreme arithmetic refusal.
Observer controls cover metadata, byte layout and path/argument refusals.

The [proposed frozen-characteristic incision formula](fine-terrain-patch.md#relation-to-published-models-and-next-decision)
remains **unimplemented**. A valid-facet counterexample now verifies that its
unconstrained two-receiver update can fall below the higher receiver when the
dimensionless timestep is large. Positive area/derivative weights alone do not
guarantee every branch remains downhill after incision.

[Fastscapelib's eroder documentation](https://fastscapelib.readthedocs.io/en/latest/api_cpp/eroder.html)
separately describes slope-reversal prevention and reports limited-node counts;
multiple-direction support is restricted to the linear case. Stable integration
also does not make contributing area implicit. We have not copied its numerical
limiter or selected its coefficients as a calibration.

Next implement a separately pinned two-receiver incision/preparation candidate
with an explicit receiver constraint or bounded routing/time refinement. Record
every limited node and any accepted-versus-unconstrained change; detach only
actual accepted bed loss into the existing reference-material reservoir. No
unrecorded stock correction or apparent successful erosion through a reversed
branch is acceptable. Compare regenerated actual beds, time/space controls and
the old single-receiver preparation before accepting that candidate. Better
routing on an unchanged bed is not evidence of better generated mountains.

Verification for this component: 31 focused native tests pass across the facet,
geometry, fixed-field, old-preparation and two observer targets. Strict all-target
Clippy, formatting, scoped whitespace, six actual CLI refusals, exact report
repeatability and input immutability checks pass. The preceding full regression
(831 native / 96 Node) predates this new component; it is not claimed as a full
regression of the new router. Concurrent owner UI changes are excluded.
