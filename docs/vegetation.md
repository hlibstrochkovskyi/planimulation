# Vegetation with memory: `vegetation-2`

November 16, 2026. Second Milestone E increment, after
[climate normals and biomes](climate-normals.md). Vegetation is a yearly state on
the static generated world. It is computed in the native core, is not part of the
world recipe or fingerprint, and never changes physical fields.

`vegetation-2` replaces `vegetation-1` (same day, never saved in any file format).
It adds seed dispersal between neighbors, a spin-up before year 0,
precipitation variability that grows with aridity, a ten-year woody-change field
and net primary productivity.

## Purpose

`DESIGN.md` treats a biome as a resulting state: a brief disturbance need not
change the classification, and recovery lags behind conditions. `climate-normals-1`
classifies the climate instantly. This model adds the state that responds to it:
cover that grows slowly, dies back quickly in dry years and needs years to recover,
with labels derived from that accumulated cover. Productivity is the first quantity
later systems (food supply) can consume.

## Model

**Wet and dry years.** Each year every land region receives a precipitation
factor `m = exp(σZ − σ²/2)` (mean 1) applied to its climate-normal annual
precipitation:

- `Z = Σ w_k A_k` sums amplitudes of 480 fixed anomaly centers on a Fibonacci
  sphere with Gaussian weights of length 1,000 km (cut off at three lengths),
  normalized so `Σ w_k² = 1`: unit variance, coherent over roughly a thousand
  kilometers, the order of observed decorrelation distances of annual
  precipitation;
- amplitudes are first-order autoregressive (year-to-year correlation 0.4), so
  dry spells persist for a few years;
- `σ` grows with aridity, as observed interannual variability does: 0.12 at
  2,000 mm of normal precipitation or more, 0.40 at 200 mm or less, interpolated
  on a logarithmic scale (about ±13% to ±50% in a typical year);
- normal deviates come from the `vegetation-2` random stream of the world seed;
  terrain, water and climate streams are untouched.

**Capacities.** Precipitation is compared with the Köppen aridity threshold of the
climate normals (`20 × annual mean + 140` mm), giving the aridity ratio `a`:

- woody capacity rises smoothly from 0 at `a = 1.5` to 1 at `a = 2.5` (one half
  at the Köppen forest boundary of 2), using root-zone moisture, an exponential
  memory of the factor with a three-year time constant;
- total cover capacity rises from 0 at `a = 0.6` to 1 at `a = 1.4` (one half at
  the desert boundary of 1), using the current year's factor;
- where the warmest month is below 0 °C both are zero; below 10 °C there are no
  trees and herb cover is scaled by summer warmth.

**Dynamics.** Woody cover `x` moves toward its capacity `K` each year: below it by
logistic growth `x + g (x + e)(K − x)/K` (capped at `K`), above it by dieback
`x − d (x − K)`. Woody plants grow at `g = 0.15` and die back at `d = 0.3`; herbs
grow at 1.5 and die back at 0.8 toward the cover capacity left by woody plants.

**Seed dispersal.** The woody seed term is `e = 0.002 + 0.05 × w̄`, where `w̄` is
the mean woody cover of the region's neighbors in the previous year. Bare land
next to forest re-establishes within decades; isolated bare land takes much
longer, so cleared areas fill from their wooded edges. Herbs disperse widely and
use a constant `e = 0.02`.

**Spin-up.** The state starts at the capacity of normal precipitation and runs
200 years on the separate `vegetation-2/spin-up` stream before year 0. Year 0 is
therefore a typical state of the variable climate, not the more forested
capacity of permanently normal years, and the run's own stream does not depend
on the spin-up length.

**Labels.** Labels reuse the Biomes classes and colors but describe accumulated
cover:

| Rule, in order | Class |
| --- | --- |
| warmest month < 0 °C | Ice |
| warmest month < 10 °C | Tundra |
| woody cover ≥ 0.5 (a forest keeps its label down to 0.4) | Forest; the climate rules choose boreal, temperate, tropical seasonal or rainforest |
| established cover ≥ 0.5 (forest or open land keeps the label down to 0.4) | Savanna (annual mean ≥ 18 °C) or grassland |
| otherwise | Hot or cold desert |

Established cover is an exponential memory of total cover with an eight-year time
constant, so one bad year does not turn grassland into desert. With variability
set to zero the state equals the climate-normals classes forever (tested).

**Productivity.** Net primary productivity follows the Miami model (Lieth 1975),
in grams of dry matter per square meter per year: the smaller of
`3000 / (1 + exp(1.315 − 0.119 T))` for annual mean temperature `T` and
`3000 (1 − exp(−0.000664 P))` for this year's precipitation `P`. It is multiplied
by the realized share of normal cover, `min(1, cover / normal cover capacity)`
(the reference is at least 0.05), so drought losses and slow regrowth lower it
until vegetation recovers. It is an output only; nothing consumes it yet.

**Woody change.** Each region keeps its woody cover of the last ten years; the
change field is cover now minus ten years ago.

## Desktop

Vegetation starts automatically when climate normals finish loading. The
**Vegetation** experiment tab advances 1, 10 or 100 years (up to 1,000 per request
and 2,000 in total) and restarts at year 0. It reports land precipitation, forest
share, class changes and land productivity, and charts forest share (over its own
range) and land precipitation by year. Five layers:

- **Vegetation state**: labels in the Biomes colors;
- **Woody cover**: fraction under woody plants;
- **Woody change**: change over ten years, from −0.3 (drought loss) to +0.3
  (regrowth);
- **Productivity**: 0 to 2,500 g/m²/year;
- **Year's precipitation**: the last year's factor on a logarithmic scale from
  half to twice the normal.

The inspector's **Why this vegetation?** section states the deciding rule, then
short rows for woody and herb capacities, seed supply from neighbors, the
region's precipitation variability, productivity limits, woody change and the
history. The chart shows the region's woody and herb cover and precipitation since
year 0, with its driest year, the loss that followed and how long recovery took.
Region histories replay the woody cover that can reach the selected region from
year 0. Herbs, labels and productivity are calculated for the selected region.
The history is identical to the run.

`node --import tsx scripts/vegetation-review.ts OUTPUT_DIR [seed] [years]` captures
the layers and an inspected region's history for visual review.

The run is not saved in seasonal checkpoints. The recipe and year count reproduce
it exactly; the core has a versioned checkpoint structure with tested
continuation, but the desktop does not expose it yet.

## Measurements

Default new-world recipe (`multiscale-geography-4`, 40,962 regions), 1,000 years,
`cargo run --release --example vegetation_report -- 1000 SEED`:

| Seed | Forest share: year 0 → minimum → year 1,000 | Mean, first / second 500 years | Class changes per land region per year | Land productivity, Pg dry matter/year (range) |
| --- | --- | --- | --- | --- |
| first-light | 22.5% → 20.6% → 23.2% | 23.0% / 23.0% | 0.81% | 113 (93–132) |
| 5c151f46 | 34.2% → 30.2% → 34.0% | 32.8% / 33.1% | 0.68% | 114 (102–134) |
| 4836187e | 23.6% → 19.7% → 21.9% | 22.3% / 22.5% | 1.02% | 149 (120–171) |

- No drift: the two halves differ by at most 0.3 percentage points, and year 0
  lies within the long-run range. Under `vegetation-1` forest fell 1.5–5 points
  below year 0 because year 0 was the capacity of normal years.
- Land productivity matches Earth's order of magnitude: about 56–60 Pg of carbon,
  or roughly 115–125 Pg of dry matter per year.
- Most class changes are reversible pairs at class edges (grassland ↔ cold
  desert, savanna ↔ tropical seasonal forest, temperate forest ↔ grassland).
  Ice and tundra never change.
- The standard deviation of the global land precipitation factor is 0.056–0.064,
  probably larger than Earth's, where global land precipitation varies by a few
  percent. Shorter anomaly lengths reduce it but make the field lumpy with this
  number of centers. With 1,500 km it was 0.073–0.090.
- The [natural-world ensemble](natural-ensemble.md) extends these measurements
  to 27 default seeds and 15 stress cases.

### Core timings

Release build with Rust 1.100.0 on a shared Ryzen 5 PRO 4650U. Medians of five
repeats with `first-light` at subdivision 6. Setup includes the vegetation
spin-up. The benchmark generates geography and climate outside the timed
interval and replays region 4, in the largest connected component with a
warmest month of at least 10 °C (3,105 land regions).

| Core | Threads (setup / advance / replay) | Setup | 1,000-year advance | 1,000-year replay |
| --- | --- | ---: | ---: | ---: |
| Serial at `a655880` | 1 / 1 / 1 | 0.685 s | 2.457 s | 2.376 s |
| Optimized, one CPU via affinity | 1 / 1 / 1 | 0.231 s | 1.186 s | 0.143 s |
| Optimized, default | 6 / 6 / 5 | 0.084 s | 0.464 s | 0.098 s |

The default core measured 5.3× faster for bulk advance and 24.3× faster for
that history replay on this host.

Run `cargo run --release --locked --manifest-path native/Cargo.toml --example vegetation_timing`.
Bulk advances compute fixed land chunks in parallel and reduce per-region terms
in the original order. Single-year calls run inline so coupled models can
parallelize across seeds. Replay time depends on the connected component and the
history length. These timings cover vegetation only. The coupled desktop path
advances vegetation and wild food one year at a time.

Calibration notes: without the root-zone and established-cover memories about
4.3% of land changed class every year, mostly grassland and desert flipping.

## Acceptance checks

Native tests in `native/src/vegetation.rs`:

- relaxation stays within `[0, capacity]` and dieback is faster than growth;
- the Miami limits reproduce the published values at 20 °C and 1,000 mm;
- without variability the state equals the climate classification for 50 years
  and woody change stays zero;
- variability is 0.12 in humid and 0.40 in arid regions;
- with variability all fractions stay in `[0, 1]`, neighboring regions have far
  more similar factors than distant ones, and fewer than 1.5% of land regions
  change class per year;
- forest share in the first 50 years matches years 250–300 within two points,
  and the spin-up leaves fewer forest regions than the capacity state;
- a cleared region inside a forest regrows more than twice as much in 20 years
  as the same region without neighbor seeds;
- a checkpoint continued to year 30 matches an uninterrupted run, and region
  replays match the stored state;
- one- and four-thread runs at subdivision 6 have identical state and summary
  bits after 301 years, and their region replays agree;
- the x86-64 GNU/Linux reference uses `first-light` at subdivision 4 and default
  settings. An example containing the test's hash routines was compiled in
  a clean archive of `a655880`. FNV-1a 64-bit over little-endian state fields
  (each float's raw bits, including every summary) gave `625bbd214b17de76`
  at year 0 and `bd25e437844301ca` at year 300. The fixture also retains
  histories for one region in each of the 11 climate classes;
- in a forest region, three years at 60% of normal precipitation cut woody cover
  by more than 0.2 and lower productivity, and recovery to within 0.05 takes at
  least five years.

`tests/vegetation.test.ts` checks the protocol: year-0 labels agree with the
climate classes in most regions and exactly for ice and tundra, productivity and
change fields arrive, runs and replays agree, regeneration discards the run and
repeats it exactly, and over-limit requests are rejected.

## Limitations

- Precipitation comes from `climate-normals-1`, not the seasonal-water budget;
  vegetation does not use or change soil water, runoff or lakes. That feedback
  waits for an accepted water model.
- No temperature anomalies, fire, grazing, insects, soils or nutrients.
- One woody and one herbaceous fraction per region; no age structure or biomass.
- Productivity is an empirical climate relation scaled by cover, not a carbon or
  water balance; nothing consumes it yet.
- Parameters are plausible orders of magnitude, not fitted to observations.
- Labels use the climate-normals thresholds; deserts and savanna inherit that
  model's biases.
