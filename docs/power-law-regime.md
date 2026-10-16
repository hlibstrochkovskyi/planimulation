# Exponent-aware incision and matched external regime

October 16, 2026. Opt-in native patch comparison, not a new desktop World,
accepted Earth calibration, tiled surface or completed R3/R4 milestone.
The preceding [external reference](humid-prehistory.md) produced a smooth plateau
with our settings and a branching landscape with its published setup. This
increment tests its incision-law/coefficient pair without changing initial bed,
uplift, duration, boundary or diffusion within a paired row.

## Implemented numerical model and units

`frozen-facet-power-incision-1` extends the independently derived frozen-direction
primitive to a configurable area exponent, while slope exponent remains one:

```text
E = K A^m S
gamma = dt K A^m sum(alpha)
h_unconstrained = (h_old + gamma weighted_updated_receiver_height) / (1+gamma)
h_new = max(h_unconstrained, highest_updated_selected_receiver)
```

Accepted incision is `h_old-h_new`, not an unconstrained request. The same
receiver constraint, positive DAG validation, stable difference/interval forms,
actual detachment ledger and separate hypothetical prevented-incision records
remain in force. This is not an implicit solve of changing flow directions or
contributing areas. The exponent is finite in [0,1]; negative/nonfinite K,
nonpositive/nonfinite dt, invalid topology, overflow and unrepresentable positive
requests are refused. Inputs are immutable.

With physical area in m² and dimensionless slope, K has units
`meters^(1-2m)/year`. At m=.5 it is year⁻¹; at m=.4 it is m^0.2/year. Public
`PowerSettings` therefore uses `erodibilityCoefficient`, never the old
`erodibilityPerYear` field with incorrect units. Preparation's result is generic
over its typed settings; a common private schedule holds only dt, D and steps.
No units are converted by relabeling a number in an old settings record.

Old incision keeps `frozen-facet-incision-1` and its exact sqrt arithmetic.
The new m=.5 path also uses sqrt, so old physical fields and numerical constraint
results agree exactly. Separate dry/humid preparation pins are
`fine-patch-power-landscape-1` and `fine-patch-humid-power-landscape-1`.
Humid routing/protection retains the preceding explicit fully supplied lake
assumption, including frozen lake levels and protected submerged bed during
incision. Diffusion may still change that bed. No actual water is advanced.
All actual uplift/detachment/hillslope accounting and workload bounds are shared;
neither the material guard nor a stock is corrected to force a successful run.
Constraint metadata records the incision model actually used by each process.

[Fastscapelib's reference](https://fastscapelib.readthedocs.io/en/latest/api_cpp/eroder.html)
supports the general stream-power law and warns about transient sensitivity
despite implicit height integration. The integration here remains independently
derived and constrains only selected receivers, not every possible neighbor.

## Reproducible comparison and physical inspection

```sh
cargo run --locked --release --manifest-path native/Cargo.toml --example fine_landscape -- artifacts/NEW_DIRECTORY first-light 64 4800000 1 --reference-law
```

`--reference-law [AMPLITUDE_METERS]` adds m=.4, K=2e-4 m^0.2/year to the existing
continuous-spectrum experiment, with both dry and humid routing. All four prior
variants remain as controls. Sources, initial fields, collar, uplift, D, duration
and step count are identical within each row. This changes the law/coefficient
pair together; it does not isolate m from K or reproduce the entire published
setup. The result is `fine-landscape-review-5` with the same six raw f64 columns.
The read-only routing observer accepts the known new format and `power` /
`humid-power` fields, rejecting unknown future versions.

Version 5's manifest indexes per-row files using a named canonical JSON/FNV-1a
diagnostic binding instead of duplicating every full history. Timings belong to
that complete report binding and therefore change its checksum on a repeat;
this is not physical World identity or security authentication. The reader
checks the complete canonical row, local filename, unique membership and index
pin. Altered histories, duplicate records and unknown index formats are refused.
Its legacy JSON limit remains 16 MiB; indexed version-5 rows have an explicit
64 MiB limit while the manifest retains 16 MiB. The physical observer cap is
still 40000 cells, not a larger production protocol. This addresses the measured
49 MiB duplicated manifest / 21 MiB row from the preliminary stronger-law run;
silently disabling input bounds was not selected.

The combined seven-panel atlas uses one elevation range. A separate `*-power.svg`
compares only the two new actual beds with a clearly printed common range;
otherwise their lower relief is obscured by the old plateau's much larger range.
Both use actual heights and 1× slope lighting, not display-generated mountains.
Humid cyan links use potential-lake ancestry and are labeled as an assumption,
not discharge or funded water. Closed-area metrics always use actual bed.

First-light, 250 m spacing, 4.8 Myr, 64 steps, D=1 m²/year:

| Case/process | Bed p95 (m) | Maximum bed (m) | Actual closed catchment area |
| --- | ---: | ---: | ---: |
| Synthetic, old humid m=.5 | 2400.145 | 2400.906 | 0% |
| Synthetic, dry m=.4 | 168.213 | 220.155 | 48.28% |
| Synthetic, humid m=.4 | 144.856 | 176.045 | 26.52% |
| Generated, old humid m=.5 | 6169.381 | 6226.041 | 0% |
| Generated, dry m=.4 | 3485.242 | 3523.849 | 59.16% |
| Generated, humid m=.4 | 3472.256 | 3514.400 | 44.68% |

Actual maps show more dissected
surfaces rather than the high broad plateau, but smooth broad ridges, sampling
alignment and many actual-bed sinks remain. A lower height alone is not quality.
Generated 250 m humid limited-node updates are 1.741% of active-node stages;
prevented prism is 0.147% of detached material, with a maximum local prevented
incision of 119.09 m. Its small aggregate fraction does not erase local effects.

The optional external runner with the same initial heights/uplift and new law
gives synthetic unweighted p95/maximum 196.069/277.935 m and generated
3575.200/3626.879 m. It uses planar single routing, MST sinks and ADI diffusion,
not our spherical facet/cut-lake integration. These are comparable orders of
relief, not pointwise agreement or external verification of our solver.
Reference reports are `fastscape-reference-strength.json` and
`fastscape-power-generated.json` under ignored local artifacts.

## Resolution, time and retained refusal

For this n=1 equation, balancing the incision and diffusion timescales gives:

```text
lc = (D/K)^(1/(2m+1))
tc = lc^(1-2m)/K = lc^2/D
hc = U tc                         only for uniform U
```

At m=.5 these reduce to the familiar sqrt(D/K), 1/K and U/K. The observer
computes the time using the first form to avoid squaring a tiny lc. D=0 or K=0
has no such positive competition scale and is labeled undefined. These derived
diagnostics do not establish that the numerical graph resolves a landform.
See the [scaling analysis and its general-exponent appendix](https://esurf.copernicus.org/articles/6/779/2018/).

At m=.4, K=2e-4 and D=1, lc=113.497 m and tc=12881.507 years. The finest current
chart spacing is 2.203 lc, and a 64-stage step is 5.822 tc. Thus copying the
reference coefficients is **not** spatial or temporal qualification. D=0.01
has lc=8.788 m, even farther below current sampling. The published attractive
map is not a guarantee that its diffusion scale is resolved at our resolution.

32→64→128-stage native bed RMS differences (m), 250 m sampling, D=1:

| Case | Dry m=.4 | Humid m=.4 |
| --- | ---: | ---: |
| Synthetic | 34.30 → 36.58 | 46.08 → 30.42 |
| Generated | 39.18 → 49.52 | 44.56 → 45.02 |

The generated humid final maximum difference remains 148.74 m; its actual closed
share changes 55.75% → 44.68% → 79.67%. This is not convergence. With 1000→500→250 m
spacing, synthetic humid p95 also changes 54.46→70.88→144.86 m. Do not select
a candidate merely because it lowers one quantile or makes a more attractive map.

Two D=0.01 controls abort in the generated 50×50-cell dry power preparation with
`Accepted hillslope bed does not reconcile with requested exchange.` Their three
synthetic rows complete, but no final manifest is written: these are **partial
failed cohorts**, not valid completed-world ensembles. Do not quietly ignore the
failed generated case, relax the guard, invent material or claim the exact cause
is established from the error string alone. The failure needs a localized
accepted/requested transfer diagnostic before any numerical-policy change.

Final indexed local artifacts are `power-law-indexed`,
`power-law-indexed-repeat`, `power-law-indexed-32` and `power-law-indexed-128`.
Earlier `power-law-review`/`power-law-repeat` retain the same computations before
the separate pair atlas and dimensional-scale metadata; `power-law-qualified`,
`power-law-qualified-repeat`, `power-law-32` and `power-law-128` additionally
predate compact indexing. Those preliminary local version-5 layouts are not
claimed as a supported published format. Partial failures
are `power-law-low-diffusion` and `power-law-low-diffusion-confirmation`.

## Follow-up control — now implemented separately

The settings below motivated the subsequent
[scaled process review](scaled-process-regime.md). That record contains the
actual numerical/visual results; these parameters are not a qualified World.

Before another source/initial-noise reroll, test a process-resolved regime using
the already implemented typed primitive. A bounded candidate is a 20×20 km chart
at 500/250/125 m (40/80/160 cells per side), m=.4, D=1 and a declared lc=500 m:

```text
K = D / lc^(2m+1) = 500^(-1.8) meters^0.2/year
tc = 250000 years
T = 4800000 years
steps = 64 / 128 / 256
dt/tc = .3 / .15 / .075
finest spacing/lc = .25
```

This keeps a meaningful physical process length above the finest grid instead
of selecting higher sample counts in isolation. The existing continuous seed
field can use a recorded .25 length multiplier (250–2000 m correlation lattices)
and a 10 m envelope, held constant across resolutions. This is a new initial
condition, not an isolated exponent intervention. Generated boundary heights and
uplift still come from the same fixed geological sources. Actual geometry/CFL
work preflight must accept each run; acceptance is recorded in the follow-up.
Inspect the physical beds first. If the resolved regime remains inadequate,
retain that negative result and compare routing/landform representation directly.
Do not substitute another conservation proof or silently fill real depressions.
R3's seed/cohort and both-view review and R4's real scale coupling remain open.

## Verification scope

The subsequent [scaled process review](scaled-process-regime.md) implements this
proposed profile at 125 m and 64/128/256 stages. It retains broad forms despite
zero final sinks; an independent plane witness exposes a remaining sampling-
width dependence in the total-area pressure. A separately pinned specific-area
transport comparison is proposed next, not a silent units change or more noise.

New primitive tests use independent 32^(2/5)=4 and m=0 link results, exact old
m=.5 field/constraint equivalence, horizontal/vertical unit rescaling, a fixed
graph exponential refinement, malformed/overflow controls and receiver-cap
accounting. Coupled tests cover exact old half-power dry/humid histories, typed
serialized units, protected lake bed, determinism, actual displacement/prism
decomposition and invalid exponent refusal. Observer tests cover new CLI/format
choices, future-format rejection and independent n=1 competition-scale fixtures.

The default experiment repeats all 36 raw records and complete report data except
declared timings. All 24 old variant raw/report records match the preceding humid
control exactly. Across 144 complete physical records, all values/collars are
valid, actual-bed decomposition discrepancy is at most `1.73e-11 m` and relative
prism residual at most `1.67e-16`. All 654029 sparse limited-node updates reconcile
with their cumulative fields. Full native regression passes 873 tests across
151 targets, with one pre-existing ignored test. It precedes the final compact
report index; final focused tests cover that observer-only change separately.
The final seven-target focused suite passes 59 tests, including compact-index
binding/refusals and power-workload preflight. Strict all-target Clippy,
formatting and scoped whitespace pass. Both 64/128-stage indexed actual-bed
records pass read-only routing inspection; legacy version-4 input still works.
Nine actual CLI output-refusal controls pass without creating/overwriting data.
Independent FNV checks bind all indexed reports, and every indexed computation
matches the preceding retained physical/report data. The final default manifest
is 1640 bytes rather than the preliminary 49 MiB duplicated report. Timings and
their derived report bindings are not compared as physical state. These
numerical controls do not qualify a production
terrain family, and no Node/UI regression is claimed for the owner's concurrent
UI work.
