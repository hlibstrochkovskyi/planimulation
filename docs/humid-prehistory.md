# Potential-lake geological preparation and external reference

October 16, 2026. This is a bounded, opt-in model comparison, not a new World
family, real rainfall/lake inventory, desktop default or completed R3 milestone.
It follows the negative [initial-spectrum control](physical-seed-spectrum.md).

## Explicit model assumption

During this candidate's geological prehistory, impermeable depressions are
assumed to have enough long-term water supply to transmit catchment flow across
potential lakes. This strong assumption is not suitable for every arid region.
It does not use or modify the conservative seasonal water state. Contributing
area is not measured discharge; uniform effective runoff is absorbed into K.

`potential-patch-surface-1` independently derives the minimax geometric field:

```text
W(i) = min over paths from i to the real perimeter (max actual bed along path)
potential depth(i) = W(i) - actual bed(i)
```

Each perimeter node starts at its own actual height. A deterministic minimum
heap propagates bottleneck levels; equal heap levels use ascending node ID.
No epsilon gradient, bed filling or breaching is introduced. W is no lower than
bed and has adjacent nonincreasing escape paths. Its depth/area/reference prism
are **potential geometry only**, never funded water. Input geometry follows the
same bounded builder contract as the existing patch. Finite shape/topology,
depth and prism checks precede a returned result; failure does not mutate input.

[Barnes et al.'s Priority-Flood paper](https://rbarnes.org/sci/2014_depressions.pdf)
describes the minimal depression-free surface and edge-priority construction.
The implementation here derives the minimax search, also related to our existing
global escape analysis; no external implementation source was copied.

`humid-patch-routing-1` routes triangular facets over W, with the existing
equal-height plateau solver and no forced slope. Full potential-surface ancestry
supplies contributing area. Actual-bed facet routing is independently retained.
Zero closed proxy area is guaranteed by construction, **not proof of successful
actual-bed drainage or adequate climate/water supply**.

For incision only, nodes with W greater than bed become fixed terminals at W.
Run the existing constrained incision primitive on this cut graph with the full
proxy areas. Dry nodes can incise toward the frozen lake surface. Submerged
nodes neither incise nor receive a hypothetical prevented-incision inventory.
Project accepted dry heights back onto the actual bed; submerged actual heights
remain unchanged. Explicit terminal checks reject an unexpected proxy change.
This is a representation projection, not a correction of mass or water errors.
Outlet sills can erode; potential levels are recomputed at the next outer stage.
This frozen-stage lag is an approximation, not an implicit moving-lake solution.

`fine-patch-humid-landscape-1` shares actual uplift, accepted incision accounting
and paired hillslope transport with the dry driver. Hillslope diffusion may
change submerged bed: protection applies to stream-power incision only. Accepted
detached material enters the same bookkeeping reservoir, not modeled deposition
or sediment proven to have exited the patch. Potential storage never enters
the actual prism ledger. Stage histories and final diagnostics record potential
geometry separately. Nominal workload is bounded by
`cells * steps * (12 + diffusion_substeps) <= 128000000`, with the existing
262144-cell, 256-stage and diffusion-substep bounds. This is a workload estimate,
not an exact heap-operation count or measured interactive performance budget.

## Headless comparison

```sh
cargo run --locked --release --manifest-path native/Cargo.toml --example fine_landscape -- artifacts/NEW_DIRECTORY first-light 64 4800000 1 --humid
```

`--humid [AMPLITUDE_METERS]` uses the same continuous physical seed field as
`--spectrum`, defaulting to 1 m, and adds the humid candidate to all three old
variants. Sources, rates, initial bed and fixed collar are identical within a
paired row. `fine-landscape-review-4` preserves the six-column raw format. The
fifth atlas panel shows actual bed, never W as terrain; its cyan links use the
explicitly labeled humid proxy. Closed-basin metrics always use actual bed.
The read-only routing observer accepts version 4 and `humid` records, but still
analyzes actual bed rather than silently treating it as a funded water surface.
Versions 1–3 retain their own identities and computations.

First-light, generated 250 m patch, D=1 m²/year, 4.8 Myr, 64 steps:

| Process | Actual p95 bed (m) | 1 km transect RMS slope | Actual closed area |
| --- | ---: | ---: | ---: |
| Dry facet | 6169.381 | 0.07402 | 0% |
| Humid facet | 6169.381 | 0.07458 | 0% |

Only five potentially submerged nodes occur at most during the generated
250 m run, with maximum stage potential depth 4.67 m. This routing intervention
therefore does not explain its broad landforms. The synthetic domain is more
sensitive to early seed-lake connectivity, but still has a broad plateau.
With a 100 m initial envelope and D=0.01, synthetic actual closed area falls
from 47.78% to 0.99%; generated share falls only from 46.61% to 44.00%.
Four final generated potential-wet cells can attract 44% of active area: a small
lake footprint and a large closed watershed are not contradictory quantities.
Weak-D aligned valleys and unresolved process lengths remain, not a selected fix.

At D=1, 32→64→128 steps give generated actual-bed RMS differences
46.77→24.80 m, with final maximum difference 168.69 m. Synthetic RMS differences
are 120.61→105.34 m, with a final local difference of 1059.94 m. Drainage-network
sensitivity and frozen potential levels are not timestep-qualified by these
controls. A small prism residual does not establish geometric accuracy.

Local ignored artifacts are `humid-prehistory-{review,repeat,amplitude-100,
low-diffusion,32,128}`. Actual synthetic/generated maps were inspected. Results
do not promote this hypothesis as the missing Earth-like terrain solution.

## External numerical reference — implemented, separate from production

The optional [reference runner](../scripts/reference-fastscape.py) executes
Fastscapelib 0.3.0 and NumPy 2.2.6 in a separate Python 3.12 environment. It emits
JSON to stdout and reads a bounded six-column physical record on stdin for the
retained case. Neither package is a production dependency. Fastscapelib is
published under GPL-3.0; no library implementation has been ported or linked into
the native application by this work. Any future integration needs its own review.
The reference reader supports 4–512 cells per side, 1–256 steps, finite settings,
bed magnitudes up to 100 km and nonnegative uplift up to 0.1 m/year with a fixed
perimeter; it rejects malformed, extra-length and nonfinite records. These are
reference-runner limits, not new production parameter ranges.

```sh
# Optional uv-managed environment; no global package changes.
uv run --script scripts/reference-fastscape.py published > artifacts/fastscape-published.json
uv run --script scripts/reference-fastscape.py retained --cells 200 --spacing 250 < artifacts/humid-prehistory-review/synthetic-200-facets.f64le > artifacts/fastscape-retained-synthetic.json
# Optional independent runner validation, including malformed-input refusals:
uv run --script scripts/test-reference-fastscape.py
```

The published control follows the numerical setup in
[Fastscapelib's mountain example](https://fastscapelib.readthedocs.io/en/latest/examples/mountain_py.html).
The retained control takes our exact initial heights and uplift values but uses
planar distances/areas, single routing, MST depression resolution and ADI
diffusion. Different lake/gradient/integration assumptions prevent claiming
pointwise equivalence or using one result as an exact solver oracle.

| External control | Unweighted bed p95 (m) | Maximum bed (m) | 1 km transect RMS slope |
| --- | ---: | ---: | ---: |
| Our synthetic input, m=.5, K=1e-6, D=1 | 2400.408 | 2400.890 | 0.08463 |
| Our generated input, same coefficients | 6166.697 | 6224.708 | 0.07314 |
| Published numerical setup | 148.389 | 205.709 | 0.01942 |

Actual reference maps were inspected. The external solver also produces a broad
smooth plateau with our synthetic settings, while its published setup produces
a branching mountain landscape. This is evidence against explaining the defect
solely as a bug unique to our implementation, not verification of all our code.
Our generated control also retains broad forms. These comparisons do not isolate
one coefficient: the published setup changes exponent, coefficient, diffusion,
forcing, initial conditions, dimensions and duration. K has exponent-dependent
units; do not copy its numeric value into the fixed m=.5 model and call it the
same physical law.

The next investigation should use a **matched external numerical regime**, not
another initial-noise reroll or a claim that filling solves mountains. Declare
the incision exponents/units, domain and characteristic scales, initial relief,
boundary and time integration explicitly; compare actual beds before selecting a
new family. The [scaling analysis](https://esurf.copernicus.org/articles/6/779/2018/)
provides a framework, not universal parameter values or guaranteed convergence.
Production promotion still requires R3's seed/cohort review and R4's actual
terrain/climate/storage coupling and bounded display detail.

Two additional external controls retain our synthetic initial heights, uplift,
duration, dimensions and boundary while switching the incision law together to
the reference's m=.4 and K=2e-4 m^0.2/year. With D=1, unweighted p95 is 196.069 m
and maximum 277.935 m; with D=0.01 they are 66.921 m and 92.678 m. The D=1 actual
map has a dissected low-relief surface rather than the original high plateau.
These isolate the incision-law/coefficient pair from the initial-field change;
they do not isolate m from K or qualify our own multiple-direction integration.
Retained reports are `fastscape-reference-strength{,-low-diffusion}.json` under
`artifacts/`. This is a useful next native comparator, not calibrated mountain
height or a selected parameter set. Low relief alone is not the product goal.

## Verification

The subsequent [exponent-aware regime comparison](power-law-regime.md) now
implements the next incision-law/coefficient control while holding bed/forcing
fixed. It removes the high plateau but retains actual sinks and unresolved
time/space sensitivity. A process-resolved physical-scale comparison, rather
than promoting the copied coefficient values, is the next concrete control.

Ten new controls cover independent threshold-connectivity minimality on sixteen
small terrains, known nested sills, immutable inputs/failures, exact flat/plane
no-ops, vertical-datum invariance, overflow/malformed refusals, downstream proxy
area transmission, submerged-bed protection, lowered dry sills, exact no-pit dry
incision equivalence, zero forcing and coupled determinism/prism decomposition.
The existing preparation workload control additionally checks humid preflight.
Observer tests cover the new flag, known format and future-version refusal.

The repeated default comparison preserves all 24 raw records and complete JSON
data except declared timings. All 54 old variant records across the default,
100 m amplitude and weak-diffusion controls match their preceding raw/report
records, source files, geometry and initial-field identities exactly. Across 144 retained physical records, all
values and collars are valid, maximum actual-bed decomposition discrepancy is
`1.73e-11 m`, and maximum relative prism residual is `1.67e-16`. These are bounded
implementation checks, not Earth calibration, convergence or product completion.
Full native regression passes 865 tests across 151 targets with one pre-existing
ignored test. Strict all-target Clippy, formatting and scoped whitespace pass.
Eight actual native output-refusal controls and version-4 retained-bed routing
inspection pass. Published and retained external controls repeat their complete
reports exactly; the optional runner also has a constant-field control and nine
malformed-input refusals. Its external ADI constant-field control uses a declared
64-ULP height envelope: the external solve is not an exact flat no-op. Emitted
reference heights are not rounded or corrected. Native test counts do not include the optional Python
checks. Node/UI tests were not rerun against the owner's concurrent UI rewrite;
that work is outside this native increment and remains untouched.
