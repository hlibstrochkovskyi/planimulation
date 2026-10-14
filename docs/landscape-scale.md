# Generation-only scale measurements

October 14, 2026. Branch: `feature/multiscale-world-generation`.
This answers the [R2 scale probe](landscape-roadmap.md), not the entire future
simulator's performance budget. Desktop Recipe, IPC and checkpoint limits remain
unchanged. The application's largest selectable world is still 40,962 regions.

## Bounded path

`Surface::build_generation_probe` reuses exactly the supported geometry builder,
but admits at most subdivision 8 (655,362 regions) after validating resolution
and radius. The old `Surface::build` retains its subdivision-6 limit. Directed
tests compare every geometry array on subdivisions 0–3 and reject invalid probe
inputs before allocation; unsupported production recipes still reject.

`native/examples/landscape_scale.rs` calls the lower-level geometry, crust,
continuous-root tectonics, terrain, initial water and drainage builders, followed
by one [dry incision](stream-power-incision.md) pass. A valid subdivision-6
reference recipe supplies existing field parameters; the **experiment's sampling
resolution** is separately labeled, never passed off as a supported larger recipe.
No generated-world checkpoint or IPC frame is exported. No basin analysis,
atmosphere, dynamic water or renderer runs in this probe.

```bash
cargo run --locked --release --manifest-path native/Cargo.toml \
  --example landscape_scale -- artifacts/scale-8.json first-light 8
```

The CLI accepts subdivisions 6–8 and a new report path. Reports contain stage
wall times, cumulative Linux `VmHWM`, retained Vec-capacity bytes by subsystem,
height-field size, geometry invariants, incision settings, and a specified
little-endian FNV-1a 32-bit field diagnostic. The diagnostic is not a cryptographic
world identity or a substitute for comparing complete arrays. Timings/memory
are not part of physical reproducibility.

## Measurements

Target: AMD Ryzen 5 PRO 4650U, 6 cores / 12 threads, approximately 14.84 GiB RAM,
Linux x86-64. Three release process runs per resolution, seed `first-light`,
12 plates, Earth radius, zero old preparation passes, one 10,000-year incision
step. These ran in a normal desktop session, not an isolated benchmark; compilation
or image conversion overlapped some initial runs. Report the observed range,
not a throughput guarantee. Follow-up runs were sequential.

| Regions | Median generation + one pass (s) | Observed range (s) | Generation peak RSS (MiB) | Retained Vec capacities (MiB) | One Float64 height field |
| ---: | ---: | ---: | ---: | ---: | ---: |
| 40,962 | 0.382 | 0.232–0.383 | 34.8–35.5 | 27.44 | 327,696 B |
| 163,842 | 0.962 | 0.933–1.518 | 127.9–128.9 | 108.56 | 1,310,736 B |
| 655,362 | 5.083 | 3.653–6.092 | 503.8–504.3 | 431.88 | 5,242,896 B |

Retained-capacity values are representative of the first sequential follow-up;
they are not process memory, IPC size or future simulation storage. Reports are
only 2,550–2,591 bytes because they contain diagnostics rather than full arrays.
All three physical-field diagnostics agree within each resolution:
`a072e8c2`, `3997a3ac`, `74a761e3` respectively. Geometry count, adjacency/boundary
count and relative sphere-area checks pass in every run. Changing resolution
does not imply identical sampled geography or qualified spatial convergence.

In the 655,362-region median run, costs were:

| Stage | Seconds |
| --- | ---: |
| Surface geometry | 3.466 |
| Crust | 0.298 |
| Tectonics | 0.445 |
| Terrain | 0.734 |
| Initial water | 0.070 |
| Drainage | 0.030 |
| One incision pass, including link extraction | 0.039 |

The retained surface arrays alone occupy 337.0 MiB at that resolution, about
78% of this probe's retained capacity. Region boundary coordinates, capacities,
temporary adjacency/incident structures, and later duplicated display geometry
deserve optimization before adding further full-resolution fields. Cumulative
high-water RSS cannot attribute temporary memory to an individual later stage.

## Decision and next boundary

40,962 is not a demonstrated hardware ceiling for generation. A bounded 655,362
sample physical field is a feasible **candidate** on this machine. Conversely,
these measurements do not justify running every future subsystem at that scale,
transmitting all arrays per tick, or creating whole-world duplicated GPU meshes.

Proceed to the versioned geographic-field family in R3. Keep production limits
until display tiling/LOD, climate/catchment mapping, persistence and complete
application measurements support them. No promise of a two-million-cell desktop
world or interactive high-resolution dynamics follows from this probe.

## Subsequent uplands-family probe

After the [depression and incision review](depression-geometry.md), the same
bounded example adds explicit `--uplands` selection. Observer report version 2
records the pinned initial family and initial static dry drainage. The structural
composition has an independent supported-grid comparison against `World::generate`
for both families, with zero old smoothing. Production recipe validation still
rejects subdivision 8; reference parameters remain a valid level-6 recipe.

```bash
cargo run --locked --release --manifest-path native/Cargo.toml \
  --example landscape_scale -- artifacts/new-uplands-scale-8.json first-light 8 --uplands
```

Three fresh sequential release processes per resolution, in a normal desktop
session, retain `artifacts/uplands-scale-l{6,7,8}-r{0,1,2}.json`. Initial runs
overlapped a small regression probe/Clippy; these are observed process costs,
not isolated performance guarantees. New reports group crust/tectonics/relief
under `initialStructuralFields`; old report-1 stage measurements above remain
historical observations, not silently rewritten results.

| Regions | Median generation + one pass | Observed range | Generation peak RSS | Retained Vec capacities |
| ---: | ---: | ---: | ---: | ---: |
| 40,962 | 0.465 s | 0.289–0.512 s | 35.48–37.13 MiB | 27.63 MiB |
| 163,842 | 1.144 s | 1.013–1.778 s | 127.91–128.67 MiB | 109.31 MiB |
| 655,362 | 4.057 s | 3.992–6.510 s | 501.75–503.55 MiB | 434.88 MiB |

All three field diagnostics repeat at each resolution: `a969cdda`, `087276ee`,
`485049bc`. The supported level-6 encoded height fields also agree with the
independent one-pass uplands `landscape_preview` report. The original scale
probe's level-6 field pin remains `a072e8c2`. Hashes are noncryptographic
diagnostics, not proof of full-state equivalence or platform portability.

### More samples do not repair the model

For this **one** zero-smoothing seed, the unchanged initial bed analysis shows:

| Regions | Closed dry minima | Initial dry area reaching closed minima | Maximum bed elevation |
| ---: | ---: | ---: | ---: |
| 40,962 | 240 | 47.56% | 4,162.32 m |
| 163,842 | 731 | 57.54% | 4,466.62 m |
| 655,362 | 1,438 | 61.68% | 4,645.32 m |

These drainage measures are taken **before** the one incision pass. Coarser
sampling conceals parts of the field's topological complexity; finer sampling
does not magically open physical outlets. Boundary propagation and sampled
drainage also change with grid refinement even though continental columns are
mesh-independent. This is not a convergence proof, a twenty-seed dense cohort,
or a permanent endorheic measurement. It does demonstrate why more UI regions
alone are insufficient to fix landscape generation.

The laptop can generate these structural fields. The next geography choice
must address structural drainage/relief and retained depressions, while future
LOD and coarse-climate coupling address rendering and dynamics. No higher
desktop setting, supported full dense World, basin hierarchy, climate, dynamic
water, checkpoint or GPU performance is claimed by this extension.

Verification: the new example composition test passes for both supported
families; strict all-target Clippy and formatting/whitespace checks pass. Five
malformed/unsupported/existing-output runs refuse without creating a report.
This example-only extension follows the preceding full regression run of 703
native and 96 Node tests; it receives its own focused test, not a second claimed
full 704-test run. Owner renderer files remain untouched.

## Coupled sedimentary preparation probe

The subsequent [delivered-material preparation](sedimentary-landscape.md) now
has a separate summary-only `sedimentary_scale` probe. It retains the valid
level-6 reference recipe and explicit 40,962–655,362 sampling metadata, without
instantiating a dense World. Its supported fields/physical metrics match the
full World-based review. At 655,362 samples, coupled prehistory takes
28.025–32.135 s and the sequential control/candidate process peaks near 503 MiB;
full two-variant reporting takes 37.286–42.215 s. Physical summaries, history
and bed diagnostics repeat independently of timing/RSS. This is generation
evidence, not climate/hydrology/desktop or isolated allocator qualification.

Sediment delivery/aggradation lowers first-light's static closed dry area from
the export control's 51.53% to 35.82% on the dense grid; at 40,962 it changes
26.24% to 12.33%. The retained resolution dependence remains significant. Do
not increase the desktop limit or claim converged geography on that evidence
alone. Additional dense depression diagnostics are explicitly omitted; actual
deposition continues to use the physical hierarchy. Three focused example
controls, strict Clippy/formatting and six sampling-output refusals pass after
the preceding full 763-native/96-Node regression run.
