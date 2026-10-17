# Landscape and scale reset

October 14, 2026. Active development branch: `feature/multiscale-world-generation`.
This changes implementation priorities, not the agreed product principles in
[DESIGN.md](../DESIGN.md). Proposed capabilities below are not implemented unless
explicitly marked. Existing water experiments remain available, but further
standalone water-solver refinement is not this branch's immediate focus.

## Product acceptance before numerical depth

The next product milestone is an inspectable Earth-like natural world in both
the flat atlas and globe: varied large-scale landforms, structured mountain
systems and drainage, documented climatic approximations, and a measured laptop
budget. Numerical invariants remain necessary, but passing them is not evidence
of plausible geography. A terrain screenshot is not evidence of correct water
dynamics either.

Preserve the Rust core, desktop boundary, reproducible streams and versions,
physical units, explicit material/water exchanges, and compatible existing saves.
Do not change old seeds or model pins in place. Do not replace the technology
stack without a measured bottleneck that requires it.

## Proposed scale responsibilities

| Layer | Proposed representation | Coupling obligation |
| --- | --- | --- |
| Geological structure | Continuous seeded fields and structural graphs | Same seed has stable physical feature locations across sampling resolutions |
| Physical terrain | Finer authoritative height field/tiles | Slopes, depressions, connectivity and storage affect downstream models |
| Atmosphere/climate | Coarser spherical fields | Fluxes use physical areas; downscaling is a stated approximation |
| Hydrology | Catchments, river/lake graph and selected spatial patches | Shared transfers and aggregate stocks reconcile across scales |
| Ecology | Regional state with terrain-conditioned local properties | Fine display cannot invent biomass or productivity |
| Civilization | Settlements and route/market graphs | Traversal and production consume authoritative environmental data |
| Display | Texture/mesh tiles and view-dependent LOD | Camera detail never changes physical state |

These are design targets, not a claim that multiple coupled grids or LOD exist.
Aggregation must preserve stock sums and relevant connectivity. Averaging heights
alone cannot represent a valley, spill threshold or disconnected lake. World
identity will need separate terrain, climate and display resolutions rather than
one global `subdivision`. Its exact schema is not selected yet.

## Ordered work and exit conditions

### R1 — Known landscape process and visible comparison

Implement a small native detachment-limited stream-power incision primitive
using downstream-first implicit updates for the fixed `m=.5, n=1` case. This is
an independently derived adaptation of the literature, not copied external
source code. Preserve physical depressions and fixed terminals. Explicitly
account for removed reference-area-equivalent bed material in a separate
reservoir; do not silently discard it or call it modeled deposition.

Tests: known single-link and chain results; time refinement against a fixed-outlet
ODE; no incision at zero forcing; base-level behavior; malformed/cyclic graphs;
determinism, immutable inputs and material accounting. Then compare actual
generated fields and drainage in a side-by-side atlas over fixed seeds. State
which geographic defects remain. Do not spend weeks trying to prove a universal
erosion theorem before inspecting those worlds.

Exit: the primitive and a reproducible physical-field comparison work, with
explicit limitations. This does not yet select a new desktop world generator.

### R2 — Generation-only scale measurements

Define a separate bounded generation path to measure 40,962, 163,842 and 655,362
terrain samples without claiming that the current atmosphere, wire protocol or
checkpoint limits support them. Measure wall time, peak process memory, output
size and stage costs on the target laptop. Investigate large temporary geometry
arrays, duplicated view buffers and serialization before increasing UI limits.

Exit: retained measurements identify feasible targets and actual bottlenecks.
These counts are candidate workloads, not promised supported product settings.

### R3 — New geographic field family

Replace broad smooth-only landforms with an explicitly versioned family:
mesh-independent structural origins, a broader controlled scale spectrum,
geologically conditioned local relief, and drainage-informed preparation.
Evaluate continental shape separately from mountain/valley quality. Additional
noise alone is not fluvial erosion or plate history.

Compare at least twenty fixed ordinary seeds and directed archipelago,
supercontinent and low-relief controls. Retain physical-field metrics and images:
area-weighted hypsometry, connected landmass distributions, roughness at physical
scales, coast representation, drainage hierarchy and depression distributions.
These are diagnostics, not a mandate to force every world to resemble Earth.

Exit: both views show a materially better family and inspector provenance agrees
with it. Old recipes/fingerprints remain intact; a new version is opt-in until
reviewed. Climate/biome colors must not conceal an inadequate physical bed.

### R4 — Display and scale coupling

Introduce bounded tiles/LOD and canonical picking, then explicit terrain-to-climate
and catchment mappings. Benchmark interaction while the native model advances.
Run conservative restriction/prolongation tests, seam/pole controls, complete
checkpoint continuation and generated-world cohorts before introducing ecology.

Exit: detail and simulation rates can differ without changing the world through
camera movement. Unsupported coupling remains rejected, never approximated
silently. The chosen performance budget is documented with measurements.

## First model scope

The initial incision primitive models dry geological preparation, not a resolved
water flood, rainfall simulation, sediment transport/deposition, evolving plate
motion or ongoing ecology. Uniform effective runoff is absorbed into a declared
erodibility coefficient. Its timestep is geological preparation time, not the
seasonal clock. Faster implicit integration does not make large timesteps
accurate for transient landscapes. Continental-shape improvements and terrain
refinement remain later work.

## Primary sources and adaptation boundaries

- [Cordonnier et al. (2016)](https://www.cs.purdue.edu/cgvlab/www/publications/cordonnier2016large/): combined uplift and fluvial erosion with a structural stream graph and subsequent terrain synthesis. Its planar construction is not a ready spherical planet implementation.
- [Landlab FastscapeEroder](https://landlab.csdms.io/generated/api/landlab.components.stream_power.fastscape_stream_power.html): documents Braun–Willett stream-power integration, dependency on drainage accumulation, and transient timestep limitations. We derive the linear-slope case rather than porting its implementation.
- [FastScape processes](https://fastscape.readthedocs.io/en/latest/processes.html): modular landscape-process reference, not a complete climate/civilization engine.
- [Mapgen4](https://www.redblobgames.com/maps/mapgen4/): useful causal/visual comparison and implementation ideas; artistic map generation is not long-running conservative planetary dynamics.

External code integration requires a separate license/compatibility review.

## Current branch status

R1's [native primitive and physical-field comparisons](stream-power-incision.md)
and R2's [bounded generation-only measurements](landscape-scale.md) are implemented.
The four comparison seeds retain inadequate smooth continent shapes; incision
alone is not accepted as Earth-like geography. Generation at 655,362 samples is
measured, not selected as a supported desktop setting. R3's first
[opt-in multiscale geographic family](multiscale-geography.md) is now available
in both desktop projections, with twenty paired ordinary seeds, three controls,
independent morphology metrics and a crust-only dense-sampling probe. It improves
coast/landmass variety but retains broad relief and many initial dry sinks.
The subsequent [continental-uplands v2](continental-uplands.md) varies initial
columns while retaining v1 structural fields. Its paired cohort improves regional
elevation and reduces closed-catchment attraction, but retains many minima.
The [depression-geometry review](depression-geometry.md) now distinguishes leaf
first-merge depth from eventual water/ocean escape and measures unique storage
prisms. Remaining v2 depressions have smaller footprints but greater depth;
blanket filling/breaching is not selected. The existing accounted incision
preview now accepts the uplands family for the next actual-bed comparison.
Structural mountain/valley refinement, drainage-informed preparation and finer
physical sampling remain open; R3 is not complete and the default is unchanged.

The subsequent [structurally forced preparation](landscape-evolution.md) adds
prescribed active-belt displacement before rebuilt downhill routing and existing
incision. Twenty ordinary cases retain more high terrain than incision alone;
depth/stock ledgers and time/space controls are retained. It is a native review
component, not a new desktop default. Dense closed catchments and local time-step
rerouting remain significant. The following
[delivered-material preparation](sedimentary-landscape.md) routes accepted
erosion and pending stock on physical DAGs, deposits within rate/material/first
spill bounds, and remobilizes active deposits on later steps. It owns separate
dry pending and per-body wet external inventories. Actual-bed comparisons now
test this coupling without changing old models or installing artificial outlets.
Spatial sampling, transient sensitivity and broad initial forms still require
review; this is not a completed Earth-like/default promotion.

No new default generator, higher desktop
resolution, multiple-grid coupling, LOD, calibrated erosion or biomes are claimed.
The owner's existing renderer changes are outside this work and must not be
included in branch commits without separate authorization.

## Verification history

The initial R1/R2 increment passed full `npm test`: 665 native tests across 122 targets and 88 Node tests,
with one pre-existing ignored native test. Strict all-target Clippy, formatting,
whitespace and seven diagnostic CLI refusals pass. The final release scale smoke
preserves the recorded subdivision-6 physical-field diagnostic. Owner renderer
file hashes are unchanged and those files are excluded from this increment.
This verifies the bounded implementation and existing compatibility, not the
future geographic family or complete high-resolution desktop simulation.

The subsequent first R3 candidate passes full `npm test`: 684 native tests across
125 targets and 92 Node tests, retaining the same pre-existing ignored test.
Both actual desktop projections, numeric inspector, recipe save/reopen/regenerate,
and a candidate seasonal checkpoint continuation pass. All 23 paired geography
comparisons and three dense crust-only sampling controls complete. Strict
all-target Clippy, formatting, whitespace and four new-output CLI refusals pass.
Owner renderer hashes remain unchanged. Retained morphology limitations prevent
default promotion or claiming the complete Earth-like geography milestone.

The continental-uplands increment passes 690 native tests across 126 targets and
96 Node tests, retaining the same one ignored test. The 23 paired worlds repeat
their candidate fingerprints; frozen-terminal and fixed-water-volume controls
retain the reported drainage improvement and remaining minima. Both candidate
desktop families pass save/reopen and inspector checks. Structural terrain and
explicit geological preparation remain the next R3 work, not a higher-resolution
or climate-calibration promotion.

The depression-geometry and uplands-incision review passes 703 native tests
across 127 targets and 96 Node tests, with the same ignored native test. All
23 original geography pairs retain both fingerprints. Twenty accounted uplands
incision previews and an exact repeated first-light report complete, but broad
smoothing and eight increased-storage cases prevent preparation-model promotion.
Strict Clippy, formatting, whitespace and five new-output refusals pass. This
closes the depth/storage characterization step, not R3 or the natural-world goal.

The subsequent uplands-family generation probe repeats 40,962–655,362 workloads
with explicit sampling metadata and unchanged production limits. Laptop costs
remain feasible for generation, but one retained seed's initial closed dry area
increases with finer sampling (47.56% → 61.68%). More samples alone are therefore
not selected as the landscape fix. The focused probe/core-composition test,
old field pin, Clippy and five refusal checks pass; see the updated scale record.

The structurally forced preparation increment passes 722 native tests across
130 targets and 96 Node tests, retaining the ignored native test. All 92 complete
preparation records repeat exactly apart from declared timings. Both source
ledgers, 16/32/64-step controls, actual bed/receiver atlases and 40,962–655,362
sampling controls are retained. Strict Clippy, formatting, whitespace and eleven
new-output refusals pass. Relief retention improves, but dense closed catchments
and a 200.75 m local time-refinement difference prevent a full R3/default claim.
Future sediment delivery must be derived from actually accepted erosion, not
introduced as unrecorded depression filling.

The delivered-material increment passes full `npm test`: 763 native tests across
134 targets and 96 Node tests, retaining the same ignored test. All 69 complete
initial/old/new records repeat their raw physical arrays, histories and water
diagnostics; twenty ordinary candidates reduce median closed dry area from
23.64% to 10.98% and potential storage from 437614 to 151883 km³. Actual maps,
finite pending material, remobilized deposits and split wet collectors are
retained. Step controls show material/routing sensitivity despite nearly stable
p95, so this is not timestep-qualified R3/default completion. Strict Clippy,
formatting, whitespace and preparation CLI refusal controls pass. Owner renderer
files remain untouched and excluded.

The subsequent sedimentary sampling probe qualifies only bounded prehistory
generation: 655,362 samples complete at roughly 503 MiB cumulative process RSS
and 28–32 s coupled preparation. Independently reconstructed physical summaries,
history and bed field diagnostics repeat; supported-level metrics match the full
World-based review. Three additional focused tests and output-refusal controls
pass after the preceding full regression. Dense static closed area still reaches
35.82% for the retained seed; transient and structural-detail limitations remain.
No new desktop limit, dense checkpoint, climate coupling or complete R3/R4 is
claimed from this measurement.

The next [physical-length relief review](physical-relief-review.md) separates
belt-width and bounded initial-detail hypotheses in a 23-case full factorial.
Candidate noise gives mixed coarse results and increases the retained dense
seed's closed share for both widths; neither field nor narrow width is promoted.
Read-only actual-bed hillshade and canonical raw-field rendering now inspect
655k physical data without rerunning prehistory. Dense views require bounded
individual documents, not a giant SVG. Broad forms/source-envelope structure
remain visible; structural-distance/orientation and drainage-aware relief are
the next investigation, not claiming that added noise solved valleys.

The retained point-source control now measures persistent graph-distance excess
against an analytic spherical reference for both degree-five and degree-six
sources. Dense near-field mean excess remains 15.34% / 10.16%, rather than
converging away with sample count. The following [geodesic-belt review](geodesic-belts.md)
implements bounded direct point/arc envelopes and retains 23 three-way cases,
exact repeated physical records and a 655362-sample comparison. This separates
distance from source-origin effects, but sampled boundaries still depend on
the terrain grid and dense closed share remains roughly 34–36%. The old model
stays frozen; no new desktop/default promotion or completed R3 is claimed.

The subsequent [fixed geological sources](fixed-geological-sources.md) separate
subdivision-5 structural origins from target terrain sampling, retaining weighted
partition/motion/crust streams and explicit identity. Shared initial directions
and source records agree exactly between 40962 and 655362 samples. Twenty ordinary
worlds retain median closed dry share 9.78%, but the dense first-light share
remains 34.35% and actual maps still have broad forms. Fine physical landforms,
not another global source reroll, are the next work. Full regression passes
800 native tests across 142 targets and 96 Node tests, with the existing ignored
native test retained, plus strict all-target Clippy/formatting/whitespace checks.

## Fine physical landform increment

The global 655362-region footprint is approximately 28 km. This cannot represent
a sub-kilometer valley floor and neighboring ridge as separate authoritative
heights. Correct regional prism accounting does not prove resolved landform
geometry. It is also not evidence that whole-cell stream-power equations are
intrinsically invalid.

[Cordonnier et al., sections 6.2–6.3](https://cs.purdue.edu/homes/bbenes/papers/Cordonier16CGF.pdf)
normally use a 50×50 km domain with 10000–160000 samples (roughly 500–125 m
spacing). Their detailed rendering additionally synthesizes landform primitives;
it is not simply a dense display of the raw coarse graph. Their lake connections
and slope corrections are separate assumptions, not conservative water/material
operations that can be silently imported here.

[Theodoratos et al., sections 2.1–2.3](https://esurf.copernicus.org/articles/6/779/2018/)
study uplift, stream-power incision and linear hillslope diffusion together:

```text
dh/dt = U - K sqrt(A) |gradient(h)| + D Laplacian(h)
characteristic length = sqrt(D/K)
characteristic height = U/K
characteristic time = 1/K
```

These give a declared process scale against which to test physical sampling.
Linear diffusion is a simplified gentle-soil hillslope law, not every mountain
process. Missing diffusion and insufficient sampling are actionable hypotheses,
not proven unique explanations of our global maps.

Implement a bounded generation-only **authoritative fine patch** first, around
50–100 km across with recorded 250–1000 m spacing, location/orientation, source
identity and boundary assumptions. Geographic inputs sample global unit
directions; areas/distances and face transmissivities use declared physical
geometry. Camera movement must not generate or change the bed.

The [hillslope-transfer primitive](hillslope-transport.md) now implements paired
shared-face exchanges with focused controls. The subsequent
[authoritative fine-patch review](fine-terrain-patch.md) implements bounded
spherical geometry, fixed-field point sampling and coupled dry preparation.
This integration uses declared geometry and a monotonicity-preserving
timestep. Never clamp after an
unstable step. Test constant fields, unequal areas, material balance, maximum
principle, analytic refinement, invalid workload and immutable failure. Then
combine prescribed uplift, rebuilt actual downhill routing and existing
incision, retaining actual heights and separate accepted displacement/incision
export/lateral-transfer diagnostics. Initially do not double-count transport in
the global sedimentary pending/active inventories.

The retained 250–1000 m actual-bed comparisons do not yet meet R3 visual quality:
eight-direction routing alignment, parameter-dependent smoothing/closed basins
and time/space sensitivity remain. Neither lower diffusivity nor longer age is
selected as a fix. These results motivated the directional-error witness and
direction-flexible formulation below before further parameter searches.
Fine-patch output is not yet a tiled world, desktop model or climate coupling.
The subsequent [triangular-facet component](facet-routing.md) recovers continuous
descent directions and partitions static area, retaining physical dry sinks.
Its analytic controls and six read-only bed comparisons do not yet replace
erosion/preparation. An explicit two-receiver slope constraint was the selected
integration gate; neither unchanged-bed overlays nor accounting prove R3 quality.
The following [receiver-constrained facet preparation](facet-landscape.md) now
implements actual uplift/incision/hillslope coupling and sparse limiter traces.
Old physical records repeat exactly; new actual-bed and 32/64/128-step controls
still expose broad forms and transient/spatial sensitivity. R3 is not complete;
no new World pin, desktop limit, tiles or climate coupling are claimed.
The following [physical initial-spectrum review](physical-seed-spectrum.md)
holds sources, rates and boundary bed fixed. Its continuous 1–8 km initial
field is independent of sampling, but neither a 1 m nor 100 m envelope solves
the retained broad forms. A separately declared humid-prehistory routing
comparison is proposed next; potential filled geometry must never be confused
with actual bed changes or funded seasonal water stocks.
The following [humid-prehistory and external reference review](humid-prehistory.md)
now implements that separate candidate with protected submerged bed and distinct
actual/proxy diagnostics. It changes some catchments but does not fix the broad
forms. Fastscapelib itself also generates a broad plateau with our retained
synthetic settings, unlike its published mountain setup. A matched external
process-regime comparison is the next investigation, not further noise rerolls,
default promotion or completion of R3/R4.
The following [exponent-aware preparation](power-law-regime.md) now tests that
law/coefficient pair with explicit units, old-model compatibility and actual
bed/clock controls. Lower, more dissected surfaces replace the high plateau,
but many sinks, strong spatial/time sensitivity and a weak-D ledger refusal
prevent promotion. A concrete bounded process-resolved regime is specified next;
global World/tiles/climate coupling remains unimplemented by these experiments.
The following [scaled process comparison](scaled-process-regime.md) now samples
125 m with a declared 500 m process length and smaller relative timesteps. The
retained beds remain broad, also in an exact external control. A plane witness
shows that total-area pressure still includes a sampling width. Specific-area
transport/units and channel–hillslope coupling are the next model-form review,
not a new resolution cap, blindly changing K or claiming completed R3/R4.
The following [specific-area transport comparison](specific-area-transport.md)
implements distinct source-area, transport-width and pressure fields, with a
separate incision law. The plane width effect and actual-bed spatial quantile
sensitivity decrease substantially, while concentrated-channel width remains
unresolved. A declared domain-scale channelization control produces additional
branches but still not adequate landform variety. Neither quantity correctness
nor this parameter control completes R3 or selects a World/default change.
The [external irregular-mesh comparison](irregular-mesh-reference.md) now checks
regular, jittered and Poisson triangulations with the same external processes.
Grid-direction order drops strongly, but D=1 broad forms remain; uncapped pure
incision instead creates excessively steep peaks. A blanket native mesh rewrite
is not selected as the solution. The next bounded quality candidate combines
structural drainage with explicit authoritative landform/hillslope construction,
before expanding framework or changing production World storage.
The subsequent [drainage-conditioned construction](drainage-landform-construction.md)
now implements native channel-profile constraints, a lower graph-slope roof,
accepted material debits and actual-bed rerouting. Independent retained
unstructured comparisons preserve nearly all selected structural channel links
and produce more dissected local relief; plain slope envelopes remain broad.
A fully native regular-patch control still exposes strong grid-direction forms.
Thus a direction-flexible native fine-generation path and inspectable physical
output are the next implementation priority, not default promotion or another
general parameter search. The component is not calibrated geological history;
global geographic family acceptance and R4 coupling remain open.
Full regression for the initial fine-patch increment passes 831 native tests across 147 targets
and 96 Node tests, with the pre-existing ignored test. The subsequently added
plane-direction witness passes in the ten-test preparation target. Strict
all-target Clippy, formatting, raw-field repeatability and CLI refusals pass.
The owner-reported angular globe coast intersections are tracked separately in
[the shoreline review](water-surface.md#open-visual-review-angular-coast-intersections).

Start with a controlled mountain domain and explicit external fixed-height
boundaries, not fake ocean cells or forced drainage from interior sinks. Compare
equal extents/durations at 1000/500/250 m, longitudinal/transverse profiles and
fixed-distance roughness. Retain an actual generated mountain patch and both
initial/final maps before promotion. A successful patch is not global water/
climate coupling: that needs material-stock restriction, explicit boundary fluxes,
channel connections and storage-aware aggregation, not averaged heights alone.
