# Specific-area transport and incision

October 16, 2026. Opt-in native `facet-specific-transport-1`,
`frozen-facet-specific-incision-1`, and `fine-patch-specific-landscape-1`.
This implements the quantity/width comparison proposed in
[the scaled process review](scaled-process-regime.md). It is not a new World
family, a funded runoff model, desktop integration or completed R3/R4.

## Two physical quantities, not a renamed area array

The existing spherical facet router supplies a frozen downhill direction and
nonnegative directional-derivative coefficients alpha (m^-1). The new candidate
uses their normalized values as flux shares, **not** the old angular partitions:

```text
s_i = reference cell area (m²)
b_i = s_i at active cells, zero at fixed external cells
p_ij = alpha_ij / sum(alpha_i)
C_i = b_i + sum_donors(p_ji C_j)                 m²
W_i = s_i sum(alpha_i)                          m
a_i = C_i / W_i                                m
F_ij = p_ij C_i = s_i alpha_ij a_i               m²
```

This is an independently derived volume-weighted negative transpose of the
existing directional derivative. In particular,
`sum(p_ij displacement_ij) = direction_i / sum(alpha_i)`.
It is not an imported implementation or a claim to reproduce a published solver.
Outgoing flux sums reconcile with collected area locally; terminal collections
reconcile with total active source area. Compensated accumulation and `1e-12`
relative guards report/refuse arithmetic failures without repairing the arrays.

Terminals retain C, but W and a are undefined and serialize as null. An external
collar cell has no local source and collects incoming flux. A real closed sink
remains a closed terminal. Flat routes retain the existing explicit fallback,
not a newly invented positive slope or an outlet through higher bed. Source,
width and edge-flux values describe unit-runoff geometry, not water inventory,
measured discharge or a claim that rainfall funded a lake.

The bounded graph primitive validates finite fields, positive reference areas,
partitions, coefficients, active unique receivers, fixed-terminal semantics and
acyclic topology. Geometry/height provenance remains the caller's obligation.
The coupled driver obtains all of these fields from the same owned patch router.

## Separate incision units and accepted material

```text
E = K_s a^m S               n=1
K_s: meters^(1-m)/year
lc = (D/K_s)^(1/(m+1))
tc = lc²/D
```

Specific pressure is supplied separately to the existing receiver-constrained
implicit kernel. Its result still reports physical C in
`contributingAreaSquareMeters`; no length is stored under that name. Terminal
pressures must be absent and active pressures positive, including at zero K.
Old total-area entry points retain their arithmetic and pins. The new settings
serialize `specificAreaExponent`, not `areaExponent`.

Each preparation stage applies accepted uplift, rebuilds the actual dry route,
incises, and runs stable paired hillslope substeps. Only actual removed bed
prism enters the detached-material bookkeeping reservoir. Hypothetical
receiver-limited loss is separate. This is not deposition, proven outlet export,
an evolved soil layer or a seasonal water advance. The unchanged workload bound
uses `cells * steps * (8 + diffusion_substeps) <= 128000000` for this adapter.

## Controlled plane: one dependence substantially reduced

On the unchanged 20 km plane h=20000-x, query (0,0) using the same four centers.
With m=.4, K_s=2e-4 and a 0.01-year actual incision stage:

| Spacing (m) | Specific area (m) | Accepted incision (m/year) |
| ---: | ---: | ---: |
| 500 | 9749.994 | .00788129 |
| 250 | 9874.993 | .00792203 |
| 125 | 9937.492 | .00794216 |

The finite collar offset shrinks with spacing. Rates differ by .77%, rather than
the old total-area witness's 42% decrease. Different coefficient dimensions mean
the absolute rates of the two laws are **not** a calibration comparison.

This is not universal grid independence. A second explicit witness holds a
concentrated channel's collected C at 1 km² while halving its strip width:
specific pressure doubles. Neither this discrete model nor the plane result
resolves a subgrid physical channel width. An axisymmetric finite-volume ring
control checks varying-width radial collection analytically; it is not an
accuracy oracle for arbitrary two-dimensional curved facets.

## Actual-bed comparison

```sh
cargo run --locked --release --manifest-path native/Cargo.toml --example fine_landscape -- artifacts/NEW_DIRECTORY first-light 128 4800000 1 --specific-regime
```

Report `fine-landscape-review-7` retains all six scaled-review controls and adds
one dry specific-area variant, its six-column physical record, and a separate
complete final routing JSON. Its compact canonical row index binds all report
values. The read-only retained-bed observer accepts version 7 and `specific`
explicitly; version 8 still refuses. This observer compares old/angular routing
on that actual bed, not the new specific-area law or fluxes.

Initial heights, source geometry, physical initial spectrum, uplift, fixed
collar, 20 km extent, duration, D=1 and stage count match the scaled control.
Across resolutions, the outer chart extent stays fixed but the fixed collar's
cell-center position changes; this is not an identical continuous boundary.
The new K_s=500^(-1.4), m=.4 gives lc=500 m and tc=250000 years. It changes both
pressure units and flux shares; it is not an isolated one-coefficient intervention.

128-stage results, area-weighted bed p95 (m) and central-16-km 1 km RMS slope:

| Case | Spacing | Total-area p95 | Specific-area p95 | Specific-area RMS slope |
| --- | ---: | ---: | ---: | ---: |
| Synthetic | 500 | 858.548 | 815.070 | .10243 |
| Synthetic | 250 | 1218.690 | 844.238 | .10303 |
| Synthetic | 125 | 1599.520 | 865.871 | .10517 |
| Generated | 500 | 4280.180 | 4220.184 | .11565 |
| Generated | 250 | 4671.820 | 4245.788 | .11400 |
| Generated | 125 | 5078.102 | 4269.940 | .11997 |

Spatial quantiles are less sensitive, but this does not establish convergence.
All these final actual routes have zero closed collection; that is not the
quality gate. Actual maps retain broad rounded forms and insufficient branches.
The finest generated candidate limits 3475 node stages, with maximum prevented
thickness 19.34 m; prevented prism is .01320% of actual detached material.
No candidate is promoted on these results.

At 125 m, 64→128→256-stage bed RMS differences are 48.35→28.91 m
(synthetic), but 48.23→54.19 m (generated). The generated final maximum
difference grows from 225.55 to 287.96 m. Thus improved spatial p95 does not
establish time convergence or a fully resolved erosion history. These are
unweighted differences on the actual 125 m nodes, not interpolated transects.

Separate SVGs use a common height range for the old/new comparison. A detail
atlas shows the candidate twice, without/with unit-runoff links, using its own
explicit height range and the same 1x physical-slope lighting. Different legend
ranges must not be mistaken for additional physical relief or improved accuracy.

## Channelization control and research boundary

[Bonetti et al., Channelization cascade (2020)](https://pmc.ncbi.nlm.nih.gov/articles/PMC6983405/)
derive a domain-scale channelization index, C_I=K_s l^(m+n)/(D^n U^(1-n)).
Their branching transitions depend on exponents and boundaries; their numerical
approximation uses D-infinity area divided by grid spacing, not our transpose
flux shares. Reported thresholds cannot be imported as acceptance tests here.

For our n=1, C_I=(l/lc)^(m+1)=174.938. Thus resolving lc alone does not specify
a densely branching regime. The `--channelization-regime` flag holds every input
above fixed except K_s=340/20000^1.4. It records profile
`specific-channelization-340-1`, actual competition scales and C_I=340 in report 7.
This is one declared process-regime intervention, not a new random reroll,
calibration to Earth or proof of the cited paper's threshold.

128-stage finest actual maps show additional branches, but retain overly regular
and broad forms. The synthetic p95 is 354.640 m; generated p95 is 3710.416 m.
Both final actual closed shares are zero. More erosion and additional branching
alone are not accepted as adequate landscape variety. Time controls, realistic
structural/boundary forcing and channel–hillslope representation still matter.
At C_I=340 the 128→256-stage bed RMS differences are 33.83 m (synthetic)
and 42.55 m (generated), with maxima 157.45 and 180.91 m. These are not a
time-convergence qualification either.

## Verification record

Final artifacts are `specific-area-{final,final-repeat}` and
`channelization-340-final`, including the inspected candidate-range detail
atlases. Local initial artifacts are `specific-area-{review,repeat,64,256}`;
channelization time controls are `channelization-340-{review,256}`. Earlier report-7 artifacts
precede the additional scale/profile metadata and detail atlas; their physical
calculations and six-column schema are unchanged. They remain retained evidence,
not edited retrospectively or complete World/checkpoints.

The repeated 42 raw records and complete normalized row reports agree exactly,
including six complete routing reports. All 36 older scaled-control physical
records and normalized variant reports remain exact. Across those 42 records,
all fields are finite and collars unchanged; maximum bed decomposition error is
`2.10e-11 m`, maximum reported material-ledger residual `1.41e-16`, and maximum
new local/terminal area residual `3.18e-16`. Timing observations are not isolated
performance benchmarks because regression work ran concurrently.

All 152 Cargo test executables pass when run in three recorded partitions:
888 passed tests and one pre-existing ignored test. The partition journals and
per-executable logs are in `artifacts/specific-area-native-part-{0,1,2}`; the
compiled executable inventory is `specific-area-test-targets.jsonl`. This is
full native test coverage, not a successful claim about the interrupted initial
single-command run. Final focused checks separately pass 72 tests across eight
targets plus five humid-compatibility library tests. Strict all-target Clippy,
formatting and scoped whitespace pass. No Node/UI regression is claimed.

Focused controls cover analytic unequal-area/split budgets, radial rings,
directional first moments, parallel and concentrated-flow limits, incision
units/known solutions, explicit terminal pressures, immutable refusal,
repeatable coupled material accounting, real closed basins, separate settings
and aggregate-work preflight. Nine actual new-output CLI refusals create no
files; an existing-output refusal preserves the prior manifest. Final version-7
specific-bed readback succeeds. Eighteen final canonical raw-row hashes verify
independently. Final full normalized rows, 42 physical records, six routing
records and four atlases per row repeat exactly; those raw fields also retain
the preliminary run and all 36 older controls. Across 168 initial temporal/
repeat records the maximum decomposition error is `2.46e-11 m` and reported
ledger residual `1.41e-16`; 210 additional final/channelization records are finite
with unchanged collars and errors no larger than the values above. These
observations do not calibrate Earth or prove the full natural-world goal.
