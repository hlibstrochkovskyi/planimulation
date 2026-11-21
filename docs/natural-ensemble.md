# Natural-world ensemble acceptance

November 21, 2026. Milestone E checks for climate normals, yearly vegetation, and
the wild-food resource. All runs use the default `multiscale-geography-4` recipe
at subdivision 6 (40,962 regions), `climate-normals-1`, `vegetation-2`, and
`wild-food-1`. It also measures annual climate-normal runoff with `rivers-1`.
The JSON includes every resolved recipe and model setting, seed, threshold, and
rerun hash. [Retained measurements](data/natural-ensemble-validation.json)

## Reproduce

From the repository root:

```sh
export CARGO_BUILD_JOBS=4
cargo run --release --locked --manifest-path native/Cargo.toml --example natural_ensemble -- --threads 4
python3 -I scripts/natural-ensemble-chart.py artifacts/natural-ensemble/ensemble.json artifacts/natural-ensemble/overview.html
```

The harness writes `ensemble.json` and `ensemble.md`. The chart script also writes
`overview.svg`. Pass `--output DIR` to put the raw reports elsewhere. Use
`--seed SEED` with a separate output directory to rerun one seed and its included
settings. Completed jobs are flushed to an interim JSON-lines file until both
final reports are written. The worker pool runs four seed/scenario jobs at a
time by default. Each job advances food
after every vegetation year and reduces both models' yearly summaries in region
order. One-year vegetation advances and the regional food diagnostic pass use
the seed worker's thread. Model setup and vegetation replay can use the native
core's own scoped workers.

The standard seeds, a fixed sequence `natural-ensemble-01` through
`natural-ensemble-24` plus `first-light`, `5c151f46`, and `4836187e`, run for
1,000 years with default settings. The three named seeds also run all five
one-factor perturbations: variability ×0, variability ×1.5, zero neighbor seed
establishment (background establishment remains 0.002), and spin-ups of 10 and
1,000 years. Spin-up and production use separate vegetation random streams.
Each of the 42 configurations is generated and simulated twice independently.

## Measurement definitions

- Land fraction and climate biome shares use total surface area. Land climate
  means and forest share are area-weighted over land. River metrics cover annual
  land runoff depth, land runoff as a share of land precipitation, and total
  discharge to the main ocean, including lake spill. They are evaluated once per
  generated world and have no feedback into vegetation or food. Forest extrema
  include year 0. Drift is the absolute difference between the area-weighted
  forest means of years 1–500 and 501–1,000, in percentage points.
- Flicker is the sum of annual vegetation label changes divided by the number
  of land regions and run years. Land NPP is annual dry matter in Pg. The
  global land precipitation-factor standard deviation uses the 1,000 annual
  area-weighted factors.
- The largest three-year woody loss compares cover in year `t − 3` with year
  `t` across all land regions and years, including year 0 as a starting value.
  The first year and lowest region index break ties. Recovery is the first
  year cover returns to within 0.05 of pre-loss cover, counted from the end
  of the loss. A loss still unrecovered at year 1,000 is right-censored and
  keeps its actual follow-up duration. Zero positive losses have no event.
  The recorded three-year rainfall factors help interpret the witness.
- Food means and extrema include year 0. Capacity density uses ice-free land
  (tundra included). Plant share is the ratio of summed plant to summed plant
  plus sustainable game energy. Minimum game ratio uses the lowest annual
  land stock divided by land stock capacity. The model allows the stock to
  exceed a newly decreased capacity during dieback.

## Acceptance checks

| Check | Measured value and rule |
| --- | --- |
| Fraction bounds | Min/max and count outside [0, 1]. Includes regional woody, herb, total and established cover, hunting access, each monthly food-season share, summary cover and forest fractions, land fraction, climate biome shares, runoff/precipitation and aggregate plant-energy share. |
| Forest drift | Half-run mean difference at most 2 percentage points, matching the earlier stability screen. |
| Class flicker | At most 0.015 changes per land region per year, the existing default-model stability threshold. The same threshold is applied to stress cases. |
| Annual NPP band | All 1,000 annual values within 50–250 Pg dry matter/year. This is a broad Earth-order plausibility screen around the roughly 120 Pg/year reference used in [vegetation measurements](vegetation.md). |
| Ice and tundra stability | Zero region-years where an ice or tundra climate class has a different vegetation class, including year 0. |
| Capacity at most stored capacity | Zero region-years with negative capacity, capacity above perfect-storage capacity or non-finite capacity. The report preserves the maximum excess and the maximum ratio where storage capacity is positive. |
| Deterministic rerun | FNV-1a 64-bit hashes must match for JSON-encoded climate metrics and both complete annual summary sequences, in their original order. |

A run is flagged for any failed check. A separate diagnostic also flags worst
woody losses with recovery longer than 100 years, or at least 100 years of
follow-up without recovery. Forest area is allowed to differ across seeds.

## Standard seed distributions

27 of 27 standard seeds passed all seven checks. Quartiles below
use linear interpolation across seed summaries.

| Metric | Min | P25 | Median | P75 | Max |
| --- | ---: | ---: | ---: | ---: | ---: |
| Land (% of surface) | 29.00 | 29.00 | 29.00 | 29.00 | 29.00 |
| Land temperature (°C) | 5.9 | 8.5 | 10.6 | 12.9 | 15.5 |
| Land precipitation (mm/year) | 620 | 665 | 688 | 755 | 851 |
| Land runoff (mm/year) | 152.4 | 170.6 | 183.5 | 195.5 | 208.5 |
| Runoff / precipitation (%) | 20.8 | 23.8 | 26.3 | 27.7 | 31.4 |
| Ocean inflow (km³/year) | 22,418 | 24,708 | 26,986 | 28,926 | 32,190 |
| Forest (mean % of land) | 21.8 | 24.7 | 27.2 | 30.4 | 33.0 |
| Forest half-run drift (pp) | 0.00 | 0.12 | 0.22 | 0.31 | 0.74 |
| Class changes (% of land regions/year) | 0.68 | 0.77 | 0.85 | 0.97 | 1.13 |
| Mean land NPP (Pg/year) | 110.4 | 121.6 | 128.1 | 143.9 | 163.5 |
| Land precipitation-factor SD | 0.051 | 0.056 | 0.060 | 0.063 | 0.070 |
| Largest 3-year woody loss (pp) | 53.8 | 55.6 | 56.8 | 58.4 | 60.5 |
| Mean forager capacity (million people) | 12.47 | 14.49 | 16.30 | 18.00 | 20.66 |
| Mean forager capacity (people/100 km²) | 8.85 | 10.39 | 11.25 | 12.42 | 14.19 |
| Plant food (% of energy) | 50.4 | 51.9 | 52.7 | 53.2 | 55.0 |
| Minimum game stock/capacity | 0.757 | 0.818 | 0.829 | 0.840 | 0.877 |
| Observed woody recovery (25 seeds, years) | 16 | 25 | 33 | 51 | 332 |

### Climate biome areas

Percent of total surface area.

| Biome | Min | P25 | Median | P75 | Max |
| --- | ---: | ---: | ---: | ---: | ---: |
| Water | 71.00 | 71.00 | 71.00 | 71.00 | 71.00 |
| Ice | 0.11 | 0.63 | 1.08 | 1.56 | 2.19 |
| Tundra | 1.32 | 2.47 | 4.25 | 4.91 | 6.61 |
| Boreal forest | 1.77 | 2.92 | 3.60 | 4.61 | 5.65 |
| Temperate forest | 1.38 | 1.79 | 2.05 | 2.28 | 2.76 |
| Grassland | 3.62 | 4.96 | 5.72 | 6.38 | 7.90 |
| Cold desert | 0.33 | 0.88 | 1.81 | 2.55 | 3.95 |
| Hot desert | 0.00 | 0.14 | 0.40 | 1.09 | 3.85 |
| Savanna | 4.60 | 6.03 | 6.65 | 8.07 | 9.95 |
| Tropical seasonal forest | 0.58 | 0.89 | 0.99 | 1.18 | 1.76 |
| Tropical rainforest | 0.85 | 1.57 | 1.86 | 2.45 | 3.59 |

### Named seed histories

| Seed | Forest year 0 / min / max / year 1,000 (%) | NPP min / mean / max (Pg/year) | Forager capacity min / mean / max (M) |
| --- | --- | --- | --- |
| `first-light` | 22.5 / 20.6 / 25.5 / 23.2 | 93.2 / 114.3 / 131.7 | 11.68 / 14.18 / 16.72 |
| `5c151f46` | 34.2 / 30.2 / 35.8 / 34.0 | 101.5 / 120.4 / 134.2 | 10.91 / 13.12 / 14.91 |
| `4836187e` | 23.6 / 19.7 / 25.8 / 21.9 | 119.8 / 143.9 / 170.5 | 16.41 / 19.71 / 23.74 |

## Extreme settings

15 of 15 runs passed all seven checks. Forest and capacity shifts
compare each extreme run with the default settings on the same seed.

| Setting | All checks | Mean forest shift (pp) | Max drift (pp) | Flicker (%/year) | Precipitation-factor SD | Mean capacity shift (M) |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| `variability-0` | 3/3 | 2.64 to 3.42 | 0.00 | 0.00 to 0.00 | 0.000 to 0.000 | -0.02 to 1.11 |
| `variability-1.5` | 3/3 | -3.50 to -2.71 | 0.49 | 1.11 to 1.46 | 0.085 to 0.099 | -0.71 to -0.15 |
| `seed-establishment-0` | 3/3 | -0.17 to -0.15 | 0.31 | 0.66 to 1.00 | 0.056 to 0.064 | 0.04 to 0.06 |
| `spin-up-10` | 3/3 | -0.03 to 0.02 | 0.28 | 0.68 to 1.02 | 0.056 to 0.064 | -0.01 to 0.00 |
| `spin-up-1000` | 3/3 | -0.01 to -0.00 | 0.35 | 0.68 to 1.02 | 0.056 to 0.064 | -0.01 to 0.00 |

## Failed checks and flagged runs


All 294 law evaluations passed, including 42 independent rerun hash matches.
Across the standard runs, annual NPP reached 89.5–185.8 Pg/year. Zero
variability produced zero flicker. Variability ×1.5 brought the rate up to
1.465% per year on `4836187e`, close to the 1.5% limit. The short and long
spin-ups shifted year-zero forest share by up to 1.6 percentage points,
while changing the 1,000-year mean by about 0.03 points or less.

Five standard seeds took more than a century to recover the largest observed
local loss. Their world-level drift and flicker stayed within the thresholds.

| Standard seed | Region | Loss ended in year | Woody cover before → after | Recovery (years) |
| --- | ---: | ---: | ---: | ---: |
| `5c151f46` | 1907 | 39 | 0.920 → 0.352 | 123 |
| `natural-ensemble-05` | 18280 | 640 | 0.927 → 0.369 | 137 |
| `natural-ensemble-07` | 6194 | 5 | 0.957 → 0.377 | 332 |
| `natural-ensemble-08` | 992 | 78 | 0.957 → 0.368 | 128 |
| `natural-ensemble-16` | 6262 | 79 | 0.977 → 0.400 | 202 |

Region 6194 on `natural-ensemble-07` received 36%, 33% and 52% of its normal
annual precipitation during the loss. The reported recovery required the cover
to reach about 0.907 again. In `5c151f46`, the same region and production-year
rainfall sequence recur with neighbor seeds disabled and with both alternative
spin-ups. Its recovery took 122–123 years in all four runs, so those variations
had little effect on this episode. Two other standard runs reached year 1,000
before recovery, with only 16 years (`natural-ensemble-04`) and 21 years
(`natural-ensemble-24`) of follow-up. The durations are right-censored.

## Limitations

The seed set samples the default Earth-sized geography with 71% target water
coverage. The stress runs vary one vegetation setting at a time on three
seeds. The thresholds screen internal stability and broad productivity
plausibility. Calibrating regional drought frequency, recovery times and
food capacities would require observations. The reported population capacity
comes from the terrestrial wild-food model under its seasonality and storage
assumptions. The runoff figures describe a repeating climate-normal year.
Drought diagnostics retain one extreme event per run, so their recovery
distribution describes selected regional maxima.

## Suggested implementation-plan paragraph

> The natural-world ensemble runs 27 default worlds and 15 vegetation stress cases for 1,000 years with climate normals, vegetation and wild food, and records climate-normal runoff. All seven checks passed on these runs, including independent deterministic replays. Standard worlds sustained mean forest shares from 21.8% to 33.0% of land. Five standard seeds flagged local woody recovery longer than a century. Retain these witnesses and the complete distributions for future calibration. Bulk vegetation advances and region histories use the original vegetation-2 arithmetic and are checked against the earlier serial core.
