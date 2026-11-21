# River-carved land: `multiscale-geography-5`

November 21, 2026. Experimental generator family, selectable as **Multiscale
geography V · experimental**. `NEW_WORLD_RECIPE` stays `multiscale-geography-4`
until version 5 is reviewed. Old recipes and fingerprints are unchanged.

## Problem

The owner's review of version-4 worlds: the planet looks like a toy. Mountains
read as smooth bands with a shadowed wall; rivers run over terrain they never
cut, as straight parallel streams on uniform slopes. The cause is in the
generator: relief is noise and tectonic envelopes, and no process ever eroded it.
`stream-power-incision-1` existed as a standalone primitive but was never part of
a world.

## What changes relative to version 4

Version 5 reuses version 4 and adds two explicit elevation contributions, both
shown in the inspector:

- **Ocean floor** (`Terrain::seafloor`, `bathymetry-1`, see
  [bathymetry](bathymetry.md)), added before dry preparation and the initial
  water fit. It raises the fitted water level by roughly 250 m, so version 5
  keeps more coastal height in the lowland step: kept fractions 0.5 at the
  shore and 0.95 inland (version 4: 0.25 and 0.85).
- **Erosion and deposition** (`Terrain::erosion`), applied after the coastal
  lowlands and the first water fit, before shallow-depression filling and the
  water refit.

Version 5 is the only family that accepts subdivision 7 (163,842 regions, about
56 km spacing).

## Erosion model

Dry land is replaced by a steady-state stream-power landscape. Uplift balances
detachment-limited incision `E = K·A^m·S` with `n = 1` and `m = 0.5`, so along
the drainage network

```text
h_i = h_receiver + k_i · A_i^(−m) · L_i
```

where `A` is the contributing area, `L` the link length and `k = U/K` the
steepness. Water is the base level. This builds the drainage network and its
valleys together, as in uplift–erosion terrain generation (Cordonnier et al.
2016), ordered upstream with the Braun & Willett (2013) stack.

1. **Initial routing.** A priority flood from all water gives every dry region a
   path to water over its lowest sill, so closed dry depressions do not stall the
   network.
2. **Steepness.** `k_i = T̄_i / χ̄_i`, where `T` is the version-4 height above the
   water level and `χ = Σ A^(−m)·L` is the integrated drainage coordinate along
   the path to water, both smoothed by two neighbor-averaging passes. Then
   `h ≈ k·χ` region by region, and the regional height follows version 4. A
   fixed log-normal factor (σ = 0.3, stream `multiscale-geography-5/steepness`)
   breaks grid symmetry.
3. **Receivers.** Each iteration recomputes receivers by steepest descent on the
   new heights, with each link's slope scaled by a fixed factor in `[0.6, 1]` (a
   mesh-independent form of Rho8, Fairfield & Leymarie 1991). Without it, uniform
   slopes produced parallel valleys along the grid's preferred directions. The
   network settles within 40 iterations on the review seeds.
4. **Relief.** The final height is the version-4 envelope (two-pass smoothed)
   plus the fluvial relief around its own two-pass mean. The relief is damped on
   low ground by a weight `T̄ / 1500 m`, clamped to [0.35, 1], because on Earth
   local relief grows with elevation and plains stay flat. Every dry region stays
   at least 5 m above water, so shorelines do not move.

Deposition is limited to the existing filling of shallow depressions. Eroded
volume is not yet delivered to deltas or shelves.

## Measurements

The table below predates `bathymetry-1` and the version-5 lowland factors. With
both, land below 200 m is 21.9%, 21.1% and 27.9% and the median land height
408, 451 and 363 m on the three seeds at subdivision 6.

`cargo run --release --example eroded_landscape_report -- SEED SUBDIVISION MODEL`,
default recipe parameters. Heights are above the initial water level. Local
relief is the largest height difference to a dry neighbor.

| Seed, subdivision | Model | Median land | p99 / max | Below 200 m | Local relief, plains < 200 m | Local relief, 500–1,000 m | Local relief, > 2,000 m | Dry sinks |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| first-light, 6 | v4 | 466 m | 2,032 / 3,055 m | 21.5% | 118 m | 120 m | 611 m | 8 |
| first-light, 6 | v5 | 464 m | 1,989 / 3,054 m | 21.5% | 73 m | 194 m | 768 m | 0 |
| first-light, 7 | v5 | 417 m | 1,973 / 3,367 m | 25.4% | 52 m | 141 m | 684 m | 1 |
| 5c151f46, 6 | v4 | 492 m | 1,897 / 2,988 m | 21.7% | 109 m | 118 m | 558 m | 29 |
| 5c151f46, 6 | v5 | 496 m | 1,886 / 3,441 m | 21.7% | 71 m | 201 m | 756 m | 0 |
| 5c151f46, 7 | v5 | 435 m | 1,854 / 3,683 m | 25.6% | 49 m | 138 m | 632 m | 9 |
| 4836187e, 6 | v4 | 376 m | 1,935 / 2,990 m | 28.4% | 86 m | 119 m | 633 m | 13 |
| 4836187e, 6 | v5 | 375 m | 1,901 / 3,136 m | 28.0% | 63 m | 185 m | 750 m | 4 |
| 4836187e, 7 | v5 | 319 m | 1,895 / 3,501 m | 34.2% | 43 m | 129 m | 596 m | 9 |

- Hypsometry stays at version 4's calibration: median and share below 200 m
  change by at most a few percent at the same subdivision.
- Local relief now grows with height. Plains are 30–40% smoother than in
  version 4, uplands and mountains 20–70% rougher, and valleys and ridges follow
  the drainage network.
- At subdivision 7 the share of land draining straight into the sea falls from
  about 16% to 9%, so the coastal "comb" of one-region streams is halved.
- Generation takes 0.4 s at subdivision 6 and 1.9 s at subdivision 7.

The whole subdivision-7 chain runs on the 12-thread test machine: generation
2.3 s, rivers 7.0 s, vegetation spin-up 0.5 s, and 100 years of vegetation with
wild food 3.0 s. At subdivision 6 these take 0.7 s, 2.7 s, 0.1 s and 0.9 s.

## Natural ensemble on version 5

`NATURAL_MODEL=multiscale-geography-5 cargo run --release --example natural_ensemble -- --threads 4`
(subdivision 6, with `bathymetry-1` and the version-5 lowland factors) passed all
seven laws in all 27 standard and 15 extreme runs (294 of 294 checks), in 7 min
16 s. Downstream calibrations hold:

| Seed | Forest year 0, v4 → v5 | Mean NPP, v4 → v5 (Pg/yr) | Mean forager capacity, v4 → v5 (M) |
| --- | --- | --- | --- |
| first-light | 22.5% → 22.9% | 114.3 → 116.6 | 14.18 → 14.47 |
| 5c151f46 | 34.2% → 35.6% | 120.4 → 122.9 | 13.12 → 13.42 |
| 4836187e | 23.6% → 25.7% | 143.9 → 149.9 | 19.71 → 20.14 |

## Acceptance checks

Native tests in `native/src/eroded_landscape.rs`:

- contributions sum to the elevation, and version 4 has zero ocean floor and
  erosion;
- erosion leaves water unchanged and keeps land above it, is deterministic and
  seed-dependent, and keeps the mean land height within 10% of version 4;
- the fitted network drains every dry region to water through strictly lower
  receivers and settles before the iteration limit;
- local relief grows from low ground to uplands to mountains;
- no water means no erosion, and invalid inputs are rejected;
- only version 5 accepts subdivision 7.

`tests/eroded-landscape.test.ts` checks recipe parsing, subdivision limits, the
protocol fields and the resolved export. `tests/resolved-export.test.ts` checks
the export bound at subdivision 7.

## Limitations

- This is a steady-state initial condition, not a simulated geological history.
  There is no time, lithology or climate-dependent erodibility.
- Mountain belts keep their version-4 envelope. Their steep single-region crest
  is still visible under strong display exaggeration.
- Eroded volume is not deposited. There are no deltas, alluvial fans or
  sediment-built shelves.
- Blending the fluvial relief onto the envelope can leave a few closed dry
  depressions deeper than the 200 m fill limit (0–9 per world).
- Rivers still step between region centers in six directions; sub-region
  meanders are a display matter.
