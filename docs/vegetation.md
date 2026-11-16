# Vegetation with memory: `vegetation-1`

November 16, 2026. Second Milestone E increment, after
[climate normals and biomes](climate-normals.md). Vegetation is a yearly state on
the static generated world. It is computed in the native core, is not part of the
world recipe or fingerprint, and never changes physical fields.

## Purpose

`DESIGN.md` treats a biome as a resulting state: a brief disturbance need not
change the classification, and recovery lags behind conditions. `climate-normals-1`
classifies the climate instantly. `vegetation-1` adds the state that responds to it:
cover that grows slowly, dies back quickly in dry years and needs years to
recover, with labels derived from that accumulated cover.

## Model

**Wet and dry years.** Each year every land region receives a precipitation
factor `m = exp(σZ − σ²/2)` (σ = 0.22, so the mean factor is 1) applied to its
climate-normal annual precipitation:

- `Z = Σ w_k A_k` sums amplitudes of 240 fixed anomaly centers on a Fibonacci
  sphere with Gaussian weights of length 1,500 km (cut off at three lengths),
  normalized so `Σ w_k² = 1`, giving unit variance and spatially coherent
  droughts across a few thousand kilometers;
- amplitudes are first-order autoregressive (year-to-year correlation 0.4), so
  dry spells persist for a few years;
- normal deviates come from the independent `vegetation-1` random stream of the
  world seed; terrain, water and climate streams are untouched.

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
Establishment `e = 0.02` lets cover return from zero without seed sources.

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
constant, so one bad year does not turn grassland into desert.

**Year 0.** Cover equals the capacity of normal precipitation, and the labels
equal the climate-normals classes. With variability set to zero the state stays
exactly there forever (tested).

## Desktop

Vegetation starts automatically when climate normals finish loading. The
**Vegetation** experiment tab advances 1, 10 or 100 years (up to 1,000 per request
and 10,000 in total) and restarts at year 0. It shows global forest share and land
precipitation by year. Three layers are added:

- **Vegetation state**: labels in the Biomes colors;
- **Woody cover**: fraction under woody plants;
- **Year's precipitation**: the last year's factor on a logarithmic scale from
  half to twice the normal.

The inspector's **Why this vegetation?** section names the rule that decided the
label, the capacities and the current memories. A chart shows the selected
region's woody and herb cover and precipitation since year 0, together with its
driest year, the loss that followed and how long recovery took. The native core
replays the selected region's history from the recorded anomaly amplitudes; the
result is identical to the full run.

`node --import tsx scripts/vegetation-review.ts OUTPUT_DIR [seed] [years]` captures
the three layers and an inspected region's history for visual review.

The run is not saved in seasonal checkpoints. The recipe and year count reproduce
it exactly; the core has a versioned checkpoint structure with tested
continuation, but the desktop does not expose it yet.

## Measurements

Default new-world recipe (`multiscale-geography-4`, 40,962 regions), 200 years,
`cargo run --release --example vegetation_report -- 200 SEED`:

| Seed | Forest share of land: year 0 → minimum → year 200 | Label changes per land region per year | Never changed | Driest / wettest land year |
| --- | --- | --- | --- | --- |
| first-light | 25.6% → 20.8% → 23.9% | 0.73% | 74% | 83% / 121% of normal |
| 5c151f46 | 36.4% → 30.2% → 33.3% | 0.58% | 77% | 85% / 125% |
| 4836187e | 25.4% → 16.4% → 20.6% | 1.00% | 65% | 84% / 120% |

Regional factors in one year span roughly 65–70% to 115–155% of normal
(5th–95th percentiles). Most label changes are reversible pairs at class edges:
grassland ↔ cold desert, savanna ↔ tropical seasonal forest, temperate forest ↔
grassland and, on the driest seed, savanna ↔ hot desert. Each pair accounts for
roughly 10–17% of all changes. Ice and tundra never change because they depend
only on temperature.

Forest settles 1.5–5 percentage points below year 0. Year 0 sits at the capacity
of normal precipitation; with variable years, fast dieback and slow regrowth keep
the average below that capacity. This is a consequence of the model, not drift:
after 1,000 years forest share is 23.3% on first-light (minimum 19.0%) and 20.5%
on 4836187e (minimum 16.0%), and the label-change rate is unchanged. 1,000 years
take about 1.4 s.

Setup takes about 0.15 s and 200 years about 0.3 s.

Calibration notes: without the root-zone and established-cover memories about
4.3% of land changed class every year (mostly grassland and desert flipping), and
forest lost 7–10 percentage points. The memories reduce label changes to under 1%
per year.

## Acceptance checks

Native tests in `native/src/vegetation.rs`:

- relaxation stays within `[0, capacity]` and dieback is faster than growth;
- without variability the state equals the climate classification for 100 years;
- with variability all fractions stay in `[0, 1]`, neighboring regions have far
  more similar factors than distant ones, and after year 50 fewer than 1.5% of
  land regions change class per year;
- a checkpoint continued to year 60 matches an uninterrupted run, and the
  per-region replay matches the stored state;
- in a forest region, three years at 60% of normal precipitation cut woody cover
  by more than 0.2, and recovery to within 0.05 takes at least five years.

`tests/vegetation.test.ts` checks the protocol: year-0 labels equal the climate
biomes, runs and replays agree, regeneration discards the run and repeats it
exactly, and over-limit requests are rejected.

## Limitations

- Precipitation comes from `climate-normals-1`, not the seasonal-water budget;
  vegetation does not use or change soil water, runoff or lakes.
- No temperature anomalies, fire, grazing, insects, soils, nutrients or seed
  spread between regions; establishment is the same everywhere.
- One woody and one herbaceous fraction per region; no age structure, biomass or
  productivity yet.
- Parameters are plausible rather than fitted to observations; timing of
  recovery (decades for forest) is the intended order of magnitude, not a
  calibrated value.
- Labels use the climate-normals thresholds; deserts and savanna inherit that
  model's biases.
