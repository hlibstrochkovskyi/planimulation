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
