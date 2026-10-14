# Structurally forced dry landscape preparation

October 14, 2026. Implemented: native kernel
`uplift-stream-power-preparation-1`, controlled physical-field atlases, time-step
controls and bounded finer-sampling reports. Not implemented: a selected desktop
preparation family, sediment deposition, reconstructed plate history or complete
Earth-like terrain. Old recipes, wire layouts and fingerprints remain unchanged.

## Why add a source rather than increase erosion?

The previous [uplands incision review](depression-geometry.md) lowered high
terrain broadly. Erosion alone has no positive process to maintain active
mountain belts. Known landscape-evolution models combine imposed uplift with
erosion; [Fastscapelib's mountain example](https://fastscapelib.readthedocs.io/en/latest/examples/mountain_py.html)
also uses diffusion and sink resolution, which are not transplanted here.
[Cordonnier et al.](https://www.cs.purdue.edu/cgvlab/www/publications/cordonnier2016large/)
combine uplift, erosion, lake-overflow connectivity and detail synthesis. Their
attractive output is not produced by a lone incision law or a ready spherical
generator. No external implementation is copied.

For our fixed exponents, the characteristic height is `U/K` and time is `1/K`.
This motivates comparisons of uplift/erosion balance rather than independently
tuning unrelated numbers. [Theodoratos et al.](https://esurf.copernicus.org/articles/6/779/2018/)
describe characteristic scales for this restricted landscape law, including
diffusion. Their analysis does not calibrate our planet or structural proxy.

## Explicit synthetic forcing

Start with the unchanged `multiscale-geography-2` initial bed, zero old dry
smoothing and the original resolved water stock. From initial convergence `C`:

```text
g[i] = C[i] / 6000 m
U[i] = K * H * g[i]                       [m/year]
U[i] = 0 at original wet terminals
```

`H` is a prescribed characteristic height, not extra crust thickness. The
normalization uses an existing relief contribution, not measured uplift,
geological age or evolving plate motion. `g` is not forced into [0,1]; recipe
relief scale and actual initial contribution determine it. The helper validates
finite nonnegative inputs but does not certify arbitrary supplied arrays as
legitimate model provenance. Production must obtain them from owned generation.

The controlled default uses `K=1e-6/year`, `T=800,000 years`, 32 steps of 25,000
years, and `H=0,500,1000,2000 m`. Nominal `K*T=.8` matches the previous eight-step
incision review, but more frequent graph reconstruction changes its trajectory.
These are stated initial-condition choices, not calibrated Earth coefficients.

## Step order and accounting

`native/src/landscape_evolution.rs` performs:

1. Apply prescribed vertical displacement to the actual bed, measuring the
   representable accepted height increment, not just requested `U*dt`.
2. Rebuild the existing physical downhill drainage on that raised bed. Spatially
   varying source rates can invalidate old receiver edges or lift an old sink.
3. Call the unchanged downstream-first implicit stream-power incision kernel.
   Original wet beds and base level remain fixed; a dry self-receiver does not
   incise. No minimax witness is installed as a physical flow direction.
4. Retain separate per-region cumulative displacement/removal and per-step
   ledgers; independently reconstruct final signed height-prism change.

For a frozen receiver graph, the source-then-implicit step corresponds to:

```text
gamma = K * dt * sqrt(contributing_area) / receiver_distance
h_new = (h_old + U*dt + gamma*h_new_receiver) / (1 + gamma)
```

Graph selection after displacement makes the full process a first-order
source/routing/incision scheme, not a simultaneous hydrodynamic solve. A stable
implicit update does not imply accurate large-step transient evolution.

Accounting uses reference-sphere areas:

```text
sum(area * (final_bed - original_bed))
    = accepted_displacement_prisms - exported_erosion_material
```

The displacement-prism integral is **kinematic bookkeeping, not newly created
rock, mantle extraction, sediment or density-dependent mass**. Incised material
has a separate off-grid reference-area-equivalent reservoir. There is no modeled
seabed deposition. Once redeposition exists, cumulative erosion cannot simply
be called external export; it must be partitioned into transfers and remaining
inventories. That extension is not hidden in this kernel.

The kernel owns outputs and leaves caller arrays unchanged on failure. It checks
bounded reciprocal topology, connectedness, initial-water consistency, finite
rates/settings and overflow. Wet-terminal source rates must be zero. Positive
requests lost when adding to a large datum are observed and book zero accepted
displacement; multiplicative underflow of a positive request refuses. Net prism
reconstruction must agree within `1e-11` relative to gross exchange, without
repairing a stock. At most 128 steps, one million regions and 64 million
region-steps are accepted; these bounds do not upgrade production recipes.

## Visible controlled review

```bash
cargo run --locked --release --manifest-path native/Cargo.toml \
  --example uplift_landscape -- artifacts/new-uplift-cohort 6

cargo run --locked --release --manifest-path native/Cargo.toml \
  --example uplift_landscape -- artifacts/new-uplift-64 6 first-light --steps=64
```

With no seed argument, run the twenty ordinary seeds and the three existing
fragmented/broad/low-relief controls. An explicit seed runs one case. The optional
step control accepts 16/32/64, always with the same 800,000-year total time. New
directories only; `manifest.json` is written last, identifying partial failures.
`uplift-landscape-review-1` stores full initial recipe/fingerprint, initial bed,
source fields, accepted physical beds, two ledgers, original-stock water fitting,
physical metrics, depression geometry and nonphysical timing observations.
These are diagnostic records, not resumable World checkpoints or recipes for
an implemented new desktop family.

Five-panel atlases show the original and four actual prepared beds on a common
0–4500 m height / 0–5000 m depth palette. Blue lines are **static bed receivers**
with at least 50,000 km² contributing dry area; yellow rings mark genuine closed
dry terminals passing that display filter. Thickness is a logarithmic area
proxy, not discharge. The overlay cannot create outlets or modify the model;
flat seam/pole lines and polygons are projection approximations, not paths
measured along real relief. These are not active rivers or biome colors.

Retained: `artifacts/uplift-review-final-l6/manifest.json`,
`artifacts/uplift-review-final-first-light.png` and the twenty-pair contact sheet.
The 23 initial records and all 92 full preparation records match a prior complete
run after excluding only declared preparation timings. The low-relief control has
exactly zero source and identical physical metrics for all four H values.

Medians across twenty ordinary worlds:

| Diagnostic | Original | H=0 | H=500 | H=1000 | H=2000 |
| --- | ---: | ---: | ---: | ---: | ---: |
| Dry height p95 | 1575.59 m | 1225.27 m | 1280.07 m | 1333.07 m | 1456.55 m |
| Closed dry minima | 220 | 56.5 | 55.5 | 56.5 | 55 |
| Static closed share of dry land | 39.77% | 23.77% | 23.61% | 23.59% | 23.64% |
| Dry area with positive water-escape depth | 5.90% | 5.24% | 5.11% | 5.02% | 4.88% |
| Additional potential storage | 456792 km³ | 454367 km³ | 440741 km³ | 429067 km³ | 437614 km³ |

H=2000 improves retained p95 relative to no-source erosion in all twenty cases
and reduces static closed dry area relative to the original in all twenty.
Four candidates have more potential storage than no-source erosion. Maximum
prism-ledger residual across all 92 cases is `1.625e-16`; maximum represented
resolved-water difference from the original requested stock is `3.275e-14`.
These static potential volumes are not actual water inventories or rainfall
required to fill all basins. No seed is rejected for an undesirable result.

### Time and space are still limitations

For first-light H=2000 at 16/32/64 steps, p95 is 1477.37/1477.43/1477.95 m and
dry minima remain 55. Closed area is 26.2369/26.2369/26.3013%. However, 32→64
unweighted per-cell bed RMS difference is 2.15 m and maximum is **200.75 m**:
local rerouting/capture can change even when global summaries barely move.
Do not claim generated-world time convergence from a stable percentile alone.
The fixed-outlet analytic ODE test establishes only its narrower numeric scope.

The summary-only `uplift_scale` example accepts sampling subdivisions 6–8,
using a valid level-6 reference Recipe and a separately labeled actual probe
grid. It shares a tested initial-field composition with `landscape_scale`, never
instantiates a dense World or writes a dense snapshot. Optional `--continuous`
selects the old structural family. Parameters remain explicit:

```bash
cargo run --locked --release --manifest-path native/Cargo.toml \
  --example uplift_scale -- artifacts/new-uplift-scale-8.json first-light 8
```

One sequential process per resolution, not an isolated timing guarantee:

| Samples | Original closed dry area | H=0 closed area | H=2000 closed area | H=2000 p95 | Two-variant total | Process peak RSS |
| ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 40962 | 47.56% | 27.12% | 26.24% | 1477.43 m | 0.636 s | 36.03 MiB |
| 163842 | 57.54% | 40.78% | 39.56% | 1606.97 m | 2.682 s | 127.64 MiB |
| 655362 | 61.68% | 51.46% | 51.53% | 1719.79 m | 11.619 s | 501.35 MiB |

Dense 32-step preparation alone takes 3.125/3.221 s for H=0/2000. Cumulative RSS
includes both variants and allocator history; it is not isolated allocation
cost. Global/per-region source/removal prisms and water stock are independently
reconstructed. Supported level-6 fields agree with the separate World-based
review. Depression hierarchy/depth/storage is explicitly omitted above level 6;
there is no substituted inferred value. Dense static drainage still uses actual
beds. Retained reports: `artifacts/uplift-scale-l{6,7,8}.json`.

## Decision and next requirement

This source law improves relief retention and is a useful preparation component.
It does **not** complete structural mountain/valley generation: broad forms,
almost-everywhere coarse incision and resolution-sensitive terminal catchments
remain. Do not promote it as a finished default or assume more samples cure
the topology. H=2000 is a retained candidate, not Earth calibration.

One missing mechanism is sediment retention. All accepted incision currently
leaves the bed for an off-grid reservoir; genuine dry basins never receive
upstream deposition. [Davy–Lague](https://doi.org/10.1029/2008JF001146) and
[Landlab erosion/deposition](https://landlab.csdms.io/generated/api/landlab.components.erosion_deposition.erosion_deposition.html)
separate erosion, transport, trapping and deposition; their complete models are
not implemented here. Next: route actually accepted solid-equivalent prisms on
the same downhill DAG and audit finite dry-terminal supply versus first-spill
capacity, separately from wet-terminal export. No input material means no
depositional bed rise. Later deposition must debit real inventory, stay within
the actual minimum-leaf footprint and declared rate, retain excess stock, and
rebuild actual terrain/drainage—not apply blanket DEM filling or assume the
first sibling merge is an ocean outlet. That is a planned next step, not working
sediment/lake functionality.

## Tests and compatibility scope

Thirteen kernel tests cover known implicit/source solutions, reordering after
nonuniform source, ODE refinement, exact no-source positive-K equivalence, pure
displacement, observed subprecision versus rejected underflow, weighted near-zero
net balance, full wet/dry domains, disconnected/malformed inputs, workload bounds,
and four generated-sphere exact repeats with independent per-region/global prisms.
Atlas overlay controls test actual area filtering and closed-sink markers.
Sampling controls compare every initial field with supported World generation
for both pins and retain unsupported-recipe refusal. Input arrays are immutable.
This numerical scope is distinct from geological realism or full R3 acceptance.

Full `npm test` passes 722 native tests across 130 targets and 96 Node tests,
with one pre-existing ignored native test. Final helper extraction receives an
additional focused composition test and old scale pin check (`a072e8c2`). Strict
all-target Clippy, typechecking, formatting and whitespace checks pass. Six
preparation and five sampling CLI refusals leave old outputs unchanged and
create no new results. A first full run caught an incorrect new overlay fixture
(its supposed upstream cell was actually a closed sink); the fixture was
corrected and the entire suite was rerun successfully. All thirteen kernel and
five new example/overlay controls pass. Owner renderer hashes remain unchanged;
their existing edits are excluded. No new packaged-desktop capability is claimed.
