# Opt-in multiscale geographic fields

October 14, 2026. Implemented candidate `multiscale-geography-1` on
`feature/multiscale-world-generation`. This is the first visible R3 increment,
not a calibrated Earth model or completion of the [landscape roadmap](landscape-roadmap.md).
The desktop default remains `terrain-prep-1`; all older recipes keep their paths.

## Use and identity

Select **Multiscale geography · experimental** in the desktop's Generation model
selector. Both map projections use the same native fields. Save/reopen and form
regeneration preserve the selected family, including existing continuous-root
and legacy recipes. Selecting another family creates a different initial world;
it does not migrate an existing simulation.

The candidate uses the existing resolved Recipe parameters and an explicit new
model pin. No field is silently added to an old recipe. Protocol-10 array meanings,
contribution bounds and the version-2 resolved-field export remain unchanged.
Seasonal checkpoint restore regenerates the selected candidate from its recorded
recipe. Same-version/environment reproducibility remains the promise; no new
cross-platform bitwise guarantee is made.

## Continental structure

`native/src/geography.rs` independently implements smooth 3D **value noise**:
deterministically hash the eight integer-lattice corner values into `[-1,1]`,
then interpolate with the quintic fade `6t^5 - 15t^4 + 10t^3`. Each channel has a
seeded orthonormal rotation and translation. Convex interpolation and normalized
positive octave weights bound the potential. This is not Perlin's gradient-noise
implementation or copied reference code. The smooth interpolation is motivated
by [Perlin's author paper](https://mrl.cs.nyu.edu/~perlin/paper445.pdf); the bounded
value-noise choice and lattice hash are our implementation choices.

Sample unit directions, not latitude/longitude. Three independent low-frequency
channels form `w(p)`. The spherical adaptation is:

```text
q = w(p) - dot(w(p), p) * p
p_warped = normalize(p + 0.32 * q)
potential = normalized_sum(weight_i * noise_i(p_warped, frequency_i / scale))
```

`q` is tangent; it cannot cancel a valid unit direction. There is no projection
seam or pole branch. The six frequency/weight pairs are `(1.7,.52)`, `(3.5,.25)`,
`(7.1,.12)`, `(14.3,.06)`, `(28.7,.03)`, `(57.5,.02)`. Each octave and warp channel
uses a dedicated `multiscale-geography-1.*` random stream. Lower frequencies
dominate; smaller scales add shape variation rather than independent cell noise.

These normalized fBm-style octaves and domain warping are procedural synthesis,
not a mantle model. [NVIDIA's terrain chapter](https://developer.nvidia.com/gpugems/gpugems3/part-i-geometry/chapter-1-generating-complex-procedural-terrains-using-gpu)
describes the underlying octave/coordinate-warp techniques; our sphere adaptation,
constants, state ownership and CPU implementation are distinct choices.

### Fixed reference fitting

Fit the requested continental-dominant fraction on a fixed subdivision-5 unit
reference sphere, independent of the requested mesh/radius. Retain its threshold
in `crust.threshold`; use the existing width-0.12 transition and crust endmembers.
For exactly 0%/100%, thresholds use global bounds `+1.12`/`-1.12`, preserving exact
extreme material states at every sampled direction. Crust potential,
continentality, thickness and density are identical at shared directions across
requested resolutions. Graph-derived plate envelopes and prepared bed are not.

Fixed fitting is reproducible, **not exact continuous-area quadrature**. Its mean
equivalent Earth-radius region width is about 223 km, while the smallest declared
continental correlation-length scale at scale 0.5 is about 55 km. Dense-sample
coverage discrepancies are therefore recorded, not refitted away. Actual mesh
coverage can differ from the user target, especially on coarse worlds.

## Structurally conditioned relief

Keep the continuous plate-root path, original crust-buoyancy baseline and static
convergence/divergence envelopes. Independently rotated value-noise bands
`(6,.55)`, `(12.5,.30)`, `(25.5,.15)` form a normalized ridged factor:

```text
r = weighted_mean((1 - abs(noise_i))^2)
convergence = original_envelope * (0.35 + 0.65*r)
divergence = original_signed_envelope * (0.65 + 0.35*r)
detail = detailAmplitudeMeters * (0.3 + 0.7*continentality) * signed_detail
```

Signed detail bands are `(7.7,.55)`, `(15.7,.30)`, `(31.7,.15)`. The positive
modulation floors keep structural envelopes from disappearing; they never
amplify the old physical bounds or invent plate relief when motion/strength is
zero. Detail remains within the recipe's absolute meter bound. The initial bed
still reconstructs as baseline + convergence + divergence + detail + preparation.

Existing bounded dry smoothing and its accounted transfers remain the preparation
stage. The separate [stream-power primitive](stream-power-incision.md) is **not**
injected into this world or mislabeled as that smoothing's material transport.
Persistent incision/deposition provenance needs a separate integration contract.

## Quality review and sampling limits

Run the non-cherry-picked comparison with new output paths:

```bash
cargo run --locked --release --manifest-path native/Cargo.toml \
  --example geography_cohort -- artifacts/new-geography-cohort 6 --dense-crust
```

It retains 20 ordinary paired seeds and three directed scale/low-relief controls,
recipes, native-compatible fingerprints, actual-field atlases and read-only
metrics. Both families use identical requested parameters, including four old
smoothing passes and 71% initial water coverage. Their inferred water volumes
may differ: these are different initial conditions, not an intervention on one
wet world. Atlas colors use common fixed height/depth ranges, never fake biomes.

The [diagnostics](../native/src/geography_metrics.rs) report area-weighted dry
hypsometry, connected landmass areas, adjacency coast crossings, neighbor-scale
roughness and static catchment/depression statistics. Crossings are not coastline
length; roughness depends on the explicitly reported neighbor scale; receiver
graphs are not active river discharge. None is a universal realism score.

At 40,962 regions an Earth-radius equivalent width is about 112 km. Neither this
sampler nor interpolation resolves kilometer-scale valleys or fjords there.
Domain warping and ridge transforms also create frequencies beyond the declared
input bands; octave cutoff is not an anti-alias guarantee. Rotating lattice axes
does not establish perfect statistical isotropy. Finer physical sampling and
bounded display LOD remain necessary.

Remaining physical limitations include broad graph-dependent mountain envelopes,
positive/symmetric convergence without modeled subduction polarity or trenches,
no geological age/uplift history, no new river-incision/deposition integration,
and no biome classification or climate calibration. A richer initial field is
not satisfactory scientific validation of those processes.

## Verification

Kernel and integration controls cover interpolation joins, bounded/reproducible
sampling, orthonormal rotations, seam/pole proximity, shared-direction crust,
exact extremes, monotone crust fraction, stopped/zero relief, contribution
identity, independent plate streams, and invalid recipes. Diagnostic tests use
hand-traced unequal-area graphs and independent spherical receiver walks.
Node integration checks native repeat/export and complete seasonal checkpoint
continuation. `scripts/test-geography-desktop.ts` exercises the real app, both
projections, inspector values, save/reopen/regenerate and legacy family selection.
The level-6 desktop smoke passes with candidate fingerprint `71acb77a`, default
fingerprint `d6044c91`, legacy level-6 fingerprint `ae2d23a8`, and no renderer
errors. The candidate's level-3 regression pin is `0727ddc8`. These pins describe
the exact recorded recipes, not scientific quality.

## Retained cohort findings

Completed all 23 level-6 pairs, without rejecting or replacing seeds. The local
manifest is `artifacts/geography-cohort-l6/manifest.json`; the CLI above regenerates
the entire review. Three individual paired atlases and the 20-pair overview were
inspected, alongside actual desktop flat/globe views. Finer coast variation is
visible, but broad relief envelopes and large smooth interiors remain visible.

Medians over the twenty ordinary worlds (mean of the middle two observations):

| Read-only sampled diagnostic | Continuous-root baseline | Candidate |
| --- | ---: | ---: |
| Connected dry landmasses | 3 | 24.5 |
| Largest landmass / dry area | 76.65% | 53.07% |
| Wet/dry adjacency crossings | 1,852.5 | 4,495.5 |
| Dry height above initial water, p50 | 492.62 m | 304.47 m |
| Dry height above initial water, p95 | 1,886.50 m | 1,337.19 m |
| Dry-neighbor RMS slope | 0.001496 | 0.001336 |
| Closed dry terminal count | 70 | 223 |
| Initial land area routed to closed dry terminals | 54.25% | 52.39% |
| Maximum dry receiver hops | 27 | 18 |

Increased fragmentation/crossings are not automatically better geology. Ridged
modulation reduces high relief in this parameter cohort; overall neighbor-scale
RMS slope is lower, not proof of more realistic mountains. The extra dry sinks
and still-large initially closed drainage fraction are important unresolved
landscape/drainage limitations. They describe the initial potential graph, **not**
a measured long-term endorheic fraction: filling/spill can change connectivity.
Do not hide these results with an attractive palette or arbitrary sink repair.

Directed candidate controls yield 119 landmasses and 15.95% largest share at scale
0.5, versus 12 and 91.94% at scale 2 for `first-light`. The low-relief case gives
23 and 32.84%. This is sensitivity evidence for this seed, not guaranteed scenario
outcomes on other seeds.

The optional 655,362-sample crust-only probe retains the same fitted threshold
and measures strict continental-dominant area fractions of 37.9264%, 37.8003%,
38.0713% for scales 0.5, 1, 2 versus the 38% target. The maximum observed difference
is about 0.200 percentage points, never refitted away. This is one seed at three
scales, not a quadrature-accuracy qualification for all worlds. Smooth mean
continentality is a separate quantity. All 2,562 shared level-4/6 crust locations
match exactly; no final-bed convergence claim follows.

The cohort ran alongside build/test workloads. Its per-stage duration fields are
observations, not a replacement for the dedicated scale benchmark. R3 remains
open: improve structural relief and drainage-informed preparation with explicit
provenance/accounting, then review finer physical sampling and the same cohort.
The candidate is available for exploration; it is not promoted to the default.

Final verification: full `npm test` passes 684 native tests across 125 targets
and 92 Node tests, with one pre-existing ignored native test. The added native
regression pin is separately rechecked after insertion. Strict all-target Clippy,
formatting, whitespace, four cohort CLI refusals and the rebuilt level-6 desktop
smoke pass. The application build succeeds with the existing Vite advisory for a
renderer chunk above 500 kB; no dependency or package upgrade was needed.

```bash
npm run build
node --import tsx scripts/test-geography-desktop.ts
```

The owner's pre-existing `map.ts`/`style.css` edits remain unchanged and outside
this commit. New selector/provenance behavior is confined to other renderer files.
