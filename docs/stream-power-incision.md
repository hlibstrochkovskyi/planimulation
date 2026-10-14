# Dry stream-power incision: first landscape increment

October 14, 2026. Branch: `feature/multiscale-world-generation`.
Implemented: a standalone native primitive and generated-field comparison.
Not implemented: a new selected world generator or desktop erosion controls.
See the [landscape roadmap](landscape-roadmap.md) for the revised priorities.

## Model and adaptation

`native/src/landscape.rs`, model pin `stream-power-incision-1`, implements the
detachment-limited stream-power law for fixed area exponent `m=0.5` and slope
exponent `n=1`:

```text
dh_i/dt = -K sqrt(A_i) (h_i - h_receiver) / L_i
gamma_i = K dt sqrt(A_i) / L_i
h_new_i = (h_old_i + gamma_i h_new_receiver) / (1 + gamma_i)
```

`A_i` is upstream reference-sphere contributing area in square meters, `L_i`
is the actual donor-to-receiver link distance in meters, height is meters, and
`K` has units `1/year` for these exponents. Prescribed uniform effective runoff
and rock response are absorbed into `K`; there is no computed rainfall input.

The receiver DAG is frozen for one step. Accumulate areas upstream-to-downstream,
then update heights downstream-to-upstream, using already accepted downstream
heights. With an optional base level, substitute the greater of the accepted
receiver height and that level; do not raise a bed already below it. Self-receivers
are fixed terminals. This includes closed depressions; no hidden filling or
artificial outlet is introduced. A later rebuilt graph may change dry terminals
when surrounding physical terrain has been incised.

The linear-slope backward-Euler step is independently derived from the method
described by [Landlab's FastscapeEroder documentation](https://landlab.csdms.io/generated/api/landlab.components.stream_power.fastscape_stream_power.html).
No external implementation is copied. This restricted primitive is not the full
FastScape framework or the complete terrain synthesis of
[Cordonnier et al.](https://www.cs.purdue.edu/cgvlab/www/publications/cordonnier2016large/).
An implicit step does not guarantee transient accuracy at a large timestep.

## Accounting and failure behavior

Every accepted height loss contributes `(old - new) * local_area` to an explicit
off-grid reference-area-equivalent material reservoir. This is not modeled
sediment transport, seabed deposition, density-dependent mass or topographic
volume on a changing sphere. The reservoir is an output increment; a future
world-preparation integration must own and persist it. Nothing is added to the
old seasonal-water or world-checkpoint formats.

The kernel owns its returned arrays and never mutates inputs. It rejects malformed
shapes, non-finite parameters, uphill links, cycles, accumulation overflow and
unrepresentable positive coefficients/material volumes. The caller owns actual
adjacency; generated-world adapters extract distances only from matching neighbor
links. A positive incision smaller than the representable absolute bed change
leaves the bed unchanged, books zero actual loss and increments an explicit
`unresolvedPositiveIncisionNodes` counter. There is no stock/budget repair.

The primitive accepts at most one million nodes, with iterative traversal and
linear working storage. This bound is not a supported desktop resolution.

## Reproducible physical-field comparison

From the repository root, use new output paths:

```bash
cargo run --locked --release --manifest-path native/Cargo.toml \
  --example landscape_preview -- \
  artifacts/landscape-report.json artifacts/landscape-atlas.svg first-light 6 8
```

The example accepts subdivisions 3–6 and 1–32 passes. It generates the existing
`continuous-plates-1` bed with zero old smoothing passes, then applies incision
with `K=1e-5/year`, `dt=10,000 years`, and initial water level as fixed base level.
Each pass rebuilds drainage on the changed physical bed with the initial wet mask
as fixed terminals. After preparation, initial water is recomputed with the
original **resolved volume**, not refitted to 71% coverage. This is a dry prehistory
comparison between initial states, not an ongoing wet-world update.

The JSON records all parameters, model pins, original/candidate height arrays,
material history and independent total bed-loss check. Timing fields are explicitly
non-deterministic. The side-by-side SVG uses actual regional heights and water
depths; colors are not biomes. Polygon projection is a display approximation,
especially at poles, not extra physical resolution. Existing output files are
never overwritten; the two files are diagnostics, not a transactional save.

Four fixed seeds at subdivision 6, eight passes:

| Seed | Initial dry-area mean incision (m) | Maximum incision (m) | Closed dry terminals, before → after | Relative material residual |
| --- | ---: | ---: | ---: | ---: |
| `first-light` | 131.294 | 1280.856 | 59 → 23 | 0 |
| `4836187e` | 137.767 | 1093.222 | 76 → 30 | 0 |
| `landscape-02` | 105.869 | 838.490 | 73 → 36 | 1.277e-16 |
| `landscape-03` | 155.954 | 911.932 | 82 → 34 | 0 |

Approximately 99.7–99.8% of initial dry area incurs some incision in these cases.
This is **not a calibrated channel/hillslope separation**. Relative resolved-water
volume differences are below `1.7e-14` under the existing water-fitting arithmetic;
represented wet coverage is unchanged in this cohort. A second complete
`first-light` JSON comparison matches after excluding only declared timings.
Fixed wet beds and the original base-level stop introduce no new coast-forming
process here. Unchanged coverage is expected, not a shoreline-evolution result.

The atlas was inspected at subdivisions 4 and 6. The result changes drainage and
relief, but retains the old smooth continent shapes and broad mountain envelopes.
It therefore does **not** close the Earth-like geography milestone. Raising
incision strength cannot substitute for better continental structure, broader
relief scales and finer physical sampling. These examples are initial process
checks, not evidence of geological realism or spatial convergence.

## Verification scope

Eight kernel tests cover independent single-link/chain solutions, base level and
zero forcing, fixed-outlet ODE time refinement, invalid/cyclic graphs, exact
repeats, dyadic relabeling, explicit material loss, extreme coefficients,
sub-precision incision and a 100,000-node chain. A generated-sphere integration
test repeats eight passes over four seeds, checks unchanged inputs/terminals,
independent cumulative material accounting and retained water volume. These tests
do not qualify evolving rainfall, tectonic uplift, deposition or long-term ecology.
