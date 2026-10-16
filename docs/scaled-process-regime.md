# Scaled process regime and the remaining flow-width problem

October 16, 2026. Headless `scaled-process-regime-1`, report
`fine-landscape-review-6`. This implements the concrete control proposed in
[the exponent-aware review](power-law-regime.md), not an accepted new World or
completed R3/R4. No production water, climate, recipe or renderer model changes.

## Implemented profile

```sh
cargo run --locked --release --manifest-path native/Cargo.toml --example fine_landscape -- artifacts/NEW_DIRECTORY first-light 128 4800000 1 --scaled-regime
```

The profile uses a 20×20 km chart, 40/80/160 cells per side (500/250/125 m), m=.4,
D=1 m²/year and K=500^(-1.8) m^0.2/year. Its characteristic length is 500 m,
time 250000 years and total duration 19.2 characteristic times. For 64/128/256
stages, dt/tc is .3/.15/.075. This is a declared process-scale test, not a claim
that choosing four samples per characteristic length resolves every process.

Only this flag permits 256 stages; previous modes retain their 128-stage CLI
limit. D must be exactly 1 for this controlled profile. The continuous physical
seed field defaults to a 10 m amplitude envelope and .25 length scale, with
250–2000 m correlation lattices. Fully resolved field identity is retained.
Fixed generated collar heights keep the legacy boundary perturbation, while
synthetic collar height stays zero. Prescribed uplift and geological source
selection are unchanged; this is not an isolated one-parameter intervention.

All six preparation variants remain. The underlying incision, diffusion,
geometry and accepted-material code is unchanged. Actual CFL/work checks,
without relaxed budgets, accept the 125 m cases: stable diffusion substeps are
25/13/7 for 64/128/256 stages. The last humid preparation's nominal work is
124518400 units, below the existing 128000000 bound.

Report 6 retains the compact canonical row index and raw six-column f64 format.
The read-only observer explicitly accepts it and rejects unknown future formats;
its 40000-cell and indexed-row byte bounds do not grow. Atlas labels use actual
extent. Common diagnostics use central **16 km** transects at 125 m query
intervals, with eight intervals per 1 km difference. They do not clamp 40 km
queries onto a 20 km domain. These remain derived diagnostics, not conservative
coupling maps or new physical samples. Previous 50 km modes retain their exact
central-40-km/250-m diagnostics and computations.

## Actual-bed results: adequate scale ratios are not sufficient

First-light, 128 stages, humid candidate, all heights in meters:

| Case | Spacing | Bed p95 | 1 km transect RMS slope | Actual closed catchment area |
| --- | ---: | ---: | ---: | ---: |
| Synthetic | 500 | 858.697 | .1079 | 0% |
| Synthetic | 250 | 1206.127 | .1454 | 0% |
| Synthetic | 125 | 1598.529 | .1870 | 0% |
| Generated | 500 | 4283.200 | .1265 | 0% |
| Generated | 250 | 4671.845 | .1639 | 0% |
| Generated | 125 | 5077.164 | .2112 | 0% |

All 36 dry/humid power results across three spacings and three stage counts
have zero final actual closed area. Yet actual maps retain broad rounded forms
and inadequate branching. Eliminating sinks is not the geographic quality gate.
The 125 m generated humid candidate has 2670 limited-node stages out of 3195392
active stages; prevented prism is .01787% of detached material, with a maximum
local prevented thickness of 18.86 m. This does not prove the limiter harmless,
but it does not support attributing all broad forms to large limiter pressure.

64→128→256-stage bed RMS differences (m), 125 m sampling:

| Case | Dry | Humid |
| --- | ---: | ---: |
| Synthetic | 23.79 → 14.80 | 23.21 → 20.91 |
| Generated | 40.62 → 18.22 | 42.97 → 19.18 |

The generated humid final maximum difference remains 85.36 m. Time refinement
improves some measures, but neither those values nor strongly changing spatial
quantiles establish convergence. The fixed collar also changes physical
cell-center position with spacing, as in previous experiments.

The external Fastscapelib control uses the **exact recorded** K, initial heights,
uplift, 160×160 nodes, 125 m spacing and 128 stages. Its synthetic unweighted
p95/maximum is 1591.239/1902.471 m; generated is 5119.075/5433.179 m. Actual
reference maps are similarly broad. It uses planar single routing, MST sink
resolution and ADI diffusion, not an exact oracle for our facet/lake model.
This control does not justify a claim that more sampling or another initial
noise reroll will produce the required landscape.

Local artifacts: `scaled-regime-{review,repeat,64,256}` and legacy compatibility
`scaled-regime-old-control`. Final external inputs/results are
`fastscape-scaled-{synthetic,generated}-exact-v2.json`; version 2 fixes only
the reference runner's transect support, preserving both complete physical beds
exactly. It records the same central-16-km/125-m queries as the native observer
and reports null when a smaller input cannot support a 1 km statistic. Earlier
`*-exact.json` records retain correct physical beds but their out-of-domain
clamped transect metrics are not used as the matched diagnostic. The earlier
`fastscape-scaled-synthetic.json` used a manually rounded K and is not the exact
matched control. Actual synthetic/generated native and reference images were
inspected. No candidate is promoted on these results.

## New controlled witness: total area contains a sampling width

On an unchanged 20 km chart plane h=20000−x, route each grid and inspect the
same point (0,0) by four-center interpolation. The fixed collar contributes no
source. In this parallel-flow setting, total routed area behaves as
`A ≈ spacing * upstream_length`, while terminal reference area is conserved.
An integration test now preserves the following limitation of the existing
model, rather than claiming the model is resolution-independent:

| Spacing (m) | Area at common point (m²) | Area/spacing (m) | Accepted incision (m/year) |
| ---: | ---: | ---: | ---: |
| 500 | 4874997.831 | 9749.996 | .0946641 |
| 250 | 2468748.684 | 9874.995 | .0721128 |
| 125 | 1242186.696 | 9937.494 | .0547902 |

The last column is an actual 0.01-year `incise_power` step with m=.4, K=2e-4
and the same unit plane slope, not the scaled profile's different K. Halving
spacing approximately halves area per sampled flow strip and multiplies this
erosion rate by .76. `A/spacing` instead approaches the 10 km upstream distance,
with the known collar offset. There is no contradiction with conserved total
catchment area: a narrower strip represents less supply.

This is one measured mechanism for resolution sensitivity, not a proof that
every remaining defect has one cause. The length diagnostic **must not replace
physical area under the old field name, coefficient units or model pin**.
Dividing area by pixel width on one plane is not a general hydrologic solution.

[Litwin et al. (2025), equations 7–8](https://esurf.copernicus.org/articles/13/277/2025/)
distinguish specific area from total area and make grid-width dependence explicit.
They also discuss the consequences of applying hillslope diffusion within
channels while tracking only surface elevation. Thus resolving the diffusion
length alone is not a complete channel–hillslope representation. This is a
model-form issue to compare, not permission to discard sediment or tune a
pixel-dependent K invisibly.

## Next candidate — proposed, not implemented

Before committing to a new mesh or more parameter searches, compare a separately
pinned specific-area transport law on the existing physical geometry. One
independently derived discrete candidate uses the existing nonnegative facet
derivative coefficients alpha (m^-1), rather than their angular area partitions:

```text
s_i = physical reference area at node i                  m²
source_i = s_i for active nodes, zero at fixed collar    m²
p_ij = alpha_ij / sum_j(alpha_ij)                        dimensionless
C_i = source_i + sum_donors(p_ji C_j)                   m²
W_i = s_i sum_j(alpha_ij)                               m
a_i = C_i / W_i                                        m
F_ij = s_i alpha_ij a_i                                 m²
sum_out(F) - sum_in(F) = source_i (nonterminals)         m²
```

Fixed external nodes have zero local source and collect terminal flux; actual
closed terminals remain explicit, not forced outlets. W/a are undefined as
transport widths/pressures at terminals and need a declared convention, while
C remains a valid collected-area budget. These are unit-runoff geometry fields,
not actual water inventories. Pressure/source units and flux shares must be
reported separately from old total-area/angular routing.

The motivation is the negative volume-weighted transpose of the existing
directional derivative: its flux shares satisfy
`sum(p_ij * displacement_ij) = direction_i / sum(alpha_ij)`.
This is an independent candidate discretization, not a copied implementation
or a claim to reproduce the cited paper. Whether its curved/convergent behavior
is adequate remains unproven.

Use a new incision law `E = K_s a^m S`, with K_s in m^(1−m)/year for n=1.
Do not pass a length into an API/record claiming square meters. Its competition
length is `(D/K_s)^(1/(m+1))`, so a declared 500 m control would use
`K_s = D/500^(m+1)` and retain the same tc=250000 years. Compare flux-budget,
plane/direction and radial controls, then actual unchanged-bed and coupled
20 km maps before selecting it. Resolving its units will not automatically fix
sediment handling, all channel-width effects or R3 quality.

An irregular-mesh comparison remains an alternative, not a selected native
rewrite. [Landlab's Voronoi/Delaunay grid](https://landlab.csdms.io/generated/api/landlab.grid.voronoi.html)
provides a possible external reference. Merely using triangles is not evidence
of acceptable mountains. No Landlab dependency or TIN implementation is added
by this increment. Global field synthesis and R4 storage-aware coupling remain
separate required work.

## Verification scope

Profile controls check exact declared length/time ratios, seed parameters,
per-profile stage/D bounds, preservation of old controls and interior affine
transects with actual 1 km differences. The plane witness checks physical
terminal-area accounting alongside the observed sampling dependence. Reader
tests accept version 6 only with the known complete-row index and reject future
versions. The final focused/CLI validation is recorded below.

Final validation: 62 focused native tests across seven targets, strict all-target
Clippy, formatting and scoped whitespace pass. Five optional external-runner
tests cover published repeatability, constant-field roundoff, malformed input,
small-domain null metrics and an independent affine 20 km transect. Earlier full
native regression predates this observer/witness increment and is not counted as
a new full run. No Node/UI regression is claimed for concurrent owner UI work.

All 36 repeated raw/report records agree except declared timings; all 36 old
control records remain exact. Across 144 complete physical records, values and
fixed collars pass, maximum bed decomposition error is `2.46e-11 m` and maximum
reported relative prism residual is `1.31e-16`. Version-6 256-stage bed readback
passes. Eight actual new-output refusals create no files; an existing-output
refusal preserves the prior manifest exactly. The compatibility run retains all
36 old raw/report records. Core equations and previous production limits are
unchanged; none of these checks is a complete R3 or natural-world acceptance.
