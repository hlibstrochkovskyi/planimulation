# Physical-length detail and belt-width review

October 14, 2026. Implemented native initial-condition component
`physical-relief-spectrum-1`, a controlled four-variant observer and read-only
field renderer. **Not selected for production.** The default and existing
geographic families remain unchanged. This studies unresolved R3 relief, not
geological folding, climate/ecology or completion of the natural-world goal.

## Why test physical scales?

The frozen geographic detail uses angular frequencies 7.7/15.7/31.7 on a unit
sphere. At Earth radius their lattice correlation-length proxies are about
827/406/201 km. These are not exact wavelengths or spectral cutoffs. Increasing
the physical mesh therefore does not by itself introduce kilometer-scale relief.
The existing tectonic envelopes also use a 300 km default graph attenuation
length. Width and fine detail are different hypotheses, so evaluate them separately.

[Geiss's procedural-terrain chapter](https://developer.nvidia.com/gpugems/gpugems3/part-i-geometry/chapter-1-generating-complex-procedural-terrains-using-gpu)
discusses octave frequencies, amplitudes and coordinate-based synthesis. This
supports procedural initial-condition experiments, not calling noise geological
evolution. Our CPU sphere sampler, bounds, physical units and controlled coupling
are independently implemented. No external source or texture is copied.

## Declared candidate field

Reuse the frozen rotated 3D value-noise primitive without changing its algorithm;
new channels are `physical-relief-spectrum-1.detail.{i}`. Old stream behavior and
world fingerprints remain unchanged. Sample unit directions, never map pixels.

| Lattice correlation length | Base weight |
| ---: | ---: |
| 800 km | .25 |
| 400 km | .30 |
| 200 km | .25 |
| 100 km | .15 |
| 50 km | .05 |

Frequency is `radius / correlation_length`. The declared physical-grid footprint
proxy is `delta = sqrt(reference_sphere_area / region_count)`, not camera zoom.
Each band uses heuristic footprint retention:

```text
x = clamp((length/delta - 2)/2, 0, 1)
retention = x*x*(3 - 2*x)
```

Suppress short bands; do not renormalize their weights into larger coarse relief.
At 40,962 samples the 200/100/50 km bands are suppressed. At 655,362 the 100 km
band is partially retained and the 50 km band remains suppressed. This is not
an exact footprint integral or proof of perfect anti-aliasing: value-noise and
nonlinear ridge transforms have higher-frequency content.

From continentality `c` and supplied initial convergence contribution `C`:

```text
strength = c * C/(1000 m + C)
ridge(n) = 2*(1 - abs(n))² - 1
shaped(n) = (1 - strength/2)*n + (strength/2)*ridge(n)
detail = amplitude * (.3 + .7*c) * sum(weight * retention * shaped(n))
```

The 1000 m strength scale and these bands/weights are stated hypotheses, not
measured Earth coefficients. Ridged shape grows with supplied continental
convergence; no missing plate strength is invented. The convex blend and
non-renormalized positive weights bound detail by the requested meter amplitude.
The field replaces **initial** detail, before water fitting and prehistory. It
does not add unaccounted ongoing material. Same seed/settings/direction gives
the same value; changing recorded physical sampling footprint changes the field
approximation. This is not a promise of identical prepared beds across meshes.

## Controlled full factorial

`relief_landscape` compares both 300/100 km belt widths with both old/candidate
detail. Keep the same v2 columns, continuous plate-root streams, prescribed
prehistory parameters (`K=1e-6/year`, H=2000 m, 32×25000 years) and aggradation
cap. Width changes structural relief and its derived source; it is not a pure
camera-width change. Candidate detail replaces only that initial contribution.

The wide old case resolves 71% water coverage once. Other initial worlds retain
that resolved volume, not independently refitted coverage. An identical initial
bed reuses its exact initial water snapshot: a null intervention must not be
perturbed by a second volume fit's numerical tolerance. Changed beds fit their
own initial water to the original stock. These are initial-world comparisons,
not one world's dynamic intervention or lake redistribution.

```bash
cargo run --locked --release --manifest-path native/Cargo.toml \
  --example relief_landscape -- artifacts/NEW_DIRECTORY 6

cargo run --locked --release --manifest-path native/Cargo.toml \
  --example relief_landscape -- artifacts/NEW_DIRECTORY 8 first-light
```

No seed: the same twenty ordinary seeds plus fragmented/broad/low-relief controls.
Sampling 6–8 is probe metadata, not a supported dense Recipe/World. Every case
retains resolved reference parameters, detail/prehistory pins and settings,
initial/prepared metrics, per-step stocks/observations, water stock and eight
region-major f64-LE physical columns. The raw columns are initial bed, prescribed
uplift rate, prepared bed, active sediment thickness, pending material volume,
cumulative erosion/deposition and accepted uplift displacement. This is a
diagnostic record, not a complete resumable checkpoint. New outputs only;
manifest written last, distinguishing partial failures.

### Retained evidence and non-promotion

`artifacts/relief-factorial-l6/manifest.json` completes all 23 four-way cases.
Ordinary medians at the same physical sampling:

| Diagnostic | Old detail / 300 km | Candidate / 300 km | Old / 100 km | Candidate / 100 km |
| --- | ---: | ---: | ---: | ---: |
| Static closed dry share | 10.9787% | 9.3298% | 11.4701% | 7.7016% |
| Closed minima | 10.5 | 9 | 12 | 11 |
| Dry height p95 | 1456.90 m | 1447.46 m | 1089.58 m | 1079.36 m |
| Maximum dry height | 3550.65 m | 3491.70 m | 2488.07 m | 2520.35 m |

Candidate detail reduces closed share in 11/20 wide cases and 14/20 narrow
cases, not every world. Narrowing lowers the upper relief markedly and does not
generally improve drainage by itself. Medians are not a realism score.

For first-light at 655,362 samples, the four static closed shares are
35.82/38.86/32.82/36.46%; minima 154/206/170/215; p95
1719.57/1707.64/1169.68/1150.26 m. Candidate detail **increases** dense closed
catchments relative to both corresponding old-detail controls. Retained:
`artifacts/relief-factorial-dense-l8/manifest.json`. More noisy relief is not
selected as a drainage solution, and 100 km is not promoted as a default width.

The old wide control reproduces every previous prepared bed, active sediment and
pending value exactly. All four low-relief raw fields are byte-identical after
the null-water control. Earlier three-way and four-way ordinary physical records
agree for their shared variants; shading/observer changes do not alter the bed.
An earlier null comparison differed by about `1.43e-10 m` because the same water
stock was refitted; that observer confound is now removed, not repaired in water.

## Actual-field views and bounded documents

Hillshade uses least-squares east/north slopes from actual geodesic neighbor
distances. Wet display surface uses its recorded level, not underwater bed slope.
Lighting is NW at 45 degrees with **display-only 20× slope exaggeration**, stated
in the view. It modifies colors, never heights, water, inventory or routing.
Colors are not biomes; blue links are static bed receivers, not discharge.

A combined 655k multi-panel SVG exceeded the converter's million-XML-element
limit, including with its unlimited option. This is diagnostic display evidence,
not failure of physical generation. New level-8 reviews write separate per-variant
SVGs; supported smaller reviews retain the four-panel atlas. This bounds document
size, but is not interactive tiles/LOD or a GPU budget qualification.

The read-only renderer can inspect existing records without repeating preparation:

```bash
cargo run --locked --release --manifest-path native/Cargo.toml \
  --example relief_render -- artifacts/NEW_VIEW.svg \
  artifacts/COMPLETED_REVIEW/selected-wide-300.json
```

It validates completed record/schema/column/count metadata and finite fields,
rebuilds canonical geometry, reconstructs final initialization water, and requires
exact equality of the recorded final level and full geography metrics before
drawing. It does not regenerate heights, advance a tick or restore a World.
This is not validation/resumption of every hidden stock or historical process.
Individual 655k maps render with the converter's ordinary limits; no safety-limit
bypass is needed. Retained single views:
`artifacts/relief-dense-{wide,detail}-single.svg` and corresponding PNGs.

Dense maps still show broad forms and source-envelope structure. Further relief
work must investigate structural distance/orientation effects and drainage-aware
forms rather than treating a denser noise field as river-valley generation.
Default selection, fine physical tiles, atmosphere/catchment mapping, checkpoint
support and both desktop projections remain open R3/R4 obligations.

## Follow-up: measured graph-distance bias

The unchanged envelope propagates the strongest exponentially attenuated source
along graph shortest paths. That is a documented proxy, not isotropic spherical
distance. More mesh edges do not automatically supply arbitrary travel directions.
The read-only `envelope_metric` control inverts a unit point source's profile,
`distance_graph = -width * log(profile)`, and compares actual node values against
analytic great-circle distance. Both source locations remain shared across meshes;
the global comparison excludes distances below 500 km, and the near-field band
is fixed at 500–1500 km. Radius is Earth-like, attenuation length 300 km.

```bash
cargo run --locked --release --manifest-path native/Cargo.toml \
  --example envelope_metric -- artifacts/NEW_REPORT.json 8 20
```

Measured area-weighted means of relative distance excess:

| Source / region degree | 2562 samples | 40962 samples | 655362 samples |
| --- | ---: | ---: | ---: |
| Source 0 / degree5, global | 8.6464% | 8.6950% | 8.6987% |
| Source 0 / degree5, near field | 14.3751% | 15.2529% | 15.3351% |
| Source 20 / degree6, global | 6.2561% | 6.3184% | 6.3216% |
| Source 20 / degree6, near field | 8.3862% | 10.0602% | 10.1555% |

Dense global p95 excess is 17.15% / 12.87%, respectively. These controls establish
persistent graph-metric discrepancy, including a regular degree-six source; they
do not attribute every real-world ridge or closed catchment to that discrepancy.
Retained: `artifacts/envelope-metric-v2-l{4,6,8}-s{0,20}.json`. No physical field
or old source law is edited to obtain this evidence.

[Kimmel–Sethian](https://math.berkeley.edu/~sethian/2006/Papers/sethian.kimmel.geodesics.pdf)
describe distance propagation on triangulated manifolds beyond edge-only paths.
For our undeformed reference sphere, direct great-circle geometry is available;
[CGAL's sphere documentation](https://doc.cgal.org/6.1/Triangulation_on_sphere_2/index.html)
describes spherical triangulation and Voronoi duals, including numerical
construction limitations. Neither external implementation is copied.

Next hypothesis: derive explicitly versioned belt envelopes from spherical
boundary/source geometry with bounded direct-distance queries, retain the same
initial crust/velocity streams, and compare actual source fields before further
incision. Exact geometric construction, degeneracies, query bounds and supported
performance must be tested. That replacement is **not implemented** by this
distance-control example and is not silently installed in old worlds.

## Verification

Five sampler tests cover bounded/repeatable values, declared footprint behavior,
structural controls, shared directions at a fixed footprint, seam/pole proximity,
zero amplitude and invalid inputs. The observer checks controls/arguments and
exact null-water reuse. Display tests retain immutable fields, exactly flat
lighting and refinement toward an independent analytic spherical derivative.
That derivative is approximate on irregular stencils, not exact planar calculus.
The initial level-4 error was `2.11e-5`; refinement through level 6 meets the
declared `1e-5` gate without changing the gradient algorithm.

Full `npm test` passes 777 native tests across 137 targets and 96 Node tests,
retaining the pre-existing ignored native test. The subsequent shared-display
extraction, full-factorial layout and read-only renderer receive focused observer,
five renderer and old-family regression checks. They are not presented as a new
full 782-test run. The subsequent distance-control example also passes its focused
analytic-distance/bounded-argument test. Strict all-target Clippy, formatting and
whitespace pass. Six observer/render binary refusals preserve old outputs.
Owner renderer files retain their hashes and are excluded from this work.
