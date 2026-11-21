# Wild food for foragers: `wild-food-1`

November 21, 2026. Third Milestone E increment, after
[vegetation with memory](vegetation.md). Wild food turns the simulated vegetation
into the first renewable resource that later population groups will consume:
seasonal plant food, a game stock that recovers from losses with a lag, and the
number of foragers a region could feed year-round. It is computed in the native
core, advances with `vegetation-2` one year at a time, is not part of the world
recipe or fingerprint, and changes neither vegetation nor climate. It has no random
stream.

No people exist in the simulation yet. The capacity is a property of the
environment, and the foraging experiment is a single-region test of the stock, not
a population model.

## Purpose

`DESIGN.md` lists wild food as a renewable resource with stock, productivity,
recovery and seasonality, and says extraction reduces a shared local stock and
winter needs differ from summer needs. The implementation plan's population
readiness asks for water, wild food, seasonal hazards, traversal costs and change
history. This model supplies the wild-food part on top of the vegetation history
and records which months limit a region.

## Model

All energies are kcal per square meter per year unless stated; capacities are
people per square kilometer.

**Growing season.** Each month's share of the year's plant growth is a thermal
factor (smoothstep from 0 to 10 °C monthly mean) times a moisture factor: the
month's precipitation, scaled by this year's factor from `vegetation-2`, relative
to twice the monthly temperature in mm (the Bagnouls–Gaussen dry-month rule, with
at least 10 mm), smoothstep from 0.5 to 1.5. Shares are normalized to one.

**Plant food.** Harvestable plant food (seeds, tubers, fruit, nuts) is a share of
the year's net primary productivity: 0.0015% of the herbaceous and 0.003% of the
woody part, at 4 kcal/g. Productivity is split by the woody and herbaceous cover
fractions. A closed canopy (woody cover 0.5 → 0.9) hides up to 80% of tree food from
people on the ground. Standing plant food loses half of what remains each month to
spoilage and other consumers.

These shares are **calibrated**, not measured edible fractions: much more of the
vegetation is edible, but search, handling and competition limit what foragers
gather. They were set so the resulting densities fall in the observed range of
terrestrial hunter-gatherers (below).

**Game.** Game is a stock of herbivore live weight. Its capacity grows faster than
proportionally with forage, as herbivore biomass does across ecosystems
(McNaughton et al. 1989): 4.5 t/km² at 750 g/m²/year of forage, exponent 1.5.
Forage is herbaceous productivity plus 15% of woody productivity as browse. The
capacity remembers forage with a three-year time constant, because herds ride out
one poor year on body reserves and movement within the region. The stock grows
logistically toward the capacity at 0.3 per year (with recolonization of 2% of the
capacity per year), and loses half of any excess per year when the capacity falls.
Year 0 starts at the year-0 capacity.

**Sustainable offtake.** People can take the logistic production at the stock, at
most at half the capacity (the maximum sustainable yield `rK/4`), times 20% left to
them by other predators, times hunting success (up to 50% lower under a closed
canopy), at 0.75 kcal per gram of live weight. A depleted stock yields less until
it recovers.

**Need.** 2,200 kcal per person per day, up to 20% more in cold months (linear from
10 °C to −20 °C monthly mean).

**Capacity.** The year-round capacity is the largest population whose monthly need
is met every month of a repeating year by eating standing plant food first and
hunting the year's game offtake in whichever months plant food runs short, without
human storage. The core finds it by false position between the month-by-month
self-sufficient population and the perfect-storage capacity (all of the year's food
stored without loss), to 0.1%. Their ratio is the **lean-season limit**. The lean
month is the month with the least new plant food relative to need.

**Foraging experiment.** A group of `N` people forages in one region for 50 years
from year 1, over the recorded vegetation history, then leaves. Each year it eats
plant food up to the region's plant-only year-round capacity and hunts the rest,
taking at most 30% of the stock (times hunting success). Unmet need is reported.
After the group leaves the stock recovers along the normal dynamics.

## Desktop

Wild food is requested after every vegetation frame. Three layers in the **Wild
food** group:

- **Forager capacity**: people per 100 km², logarithmic from 0.1 to 100;
- **Lean-season limit**: year-round capacity as a share of perfect-storage capacity;
- **Game stock**: stock relative to its capacity; gray where there is no game.

The Vegetation tab summary adds the land's year-round forager capacity and the
game stock, and its chart adds capacity over its own range. The inspector's **Why
this food supply?** section names what limits the region (total food or the lean
season), then rows for plant food, game, offtake and need, the region's history
(peak growth month, worst year and recovery) with a chart of yearly capacity, its
ten-year mean and the game stock, and buttons to forage with 1×, 3× or 10× the
group the region feeds on average (its mean year-round capacity over the history). The experiment chart shows the stock with and without the group
and the unmet need.

`node --import tsx scripts/wild-food-review.ts OUTPUT_DIR [seed] [years]` captures
the layers and a foraging experiment for visual review.

## Measurements

Default new-world recipe (`multiscale-geography-4`, 40,962 regions), 1,000 years,
`cargo run --release --example wild_food_report -- 1000 SEED`:

| Seed | Year-round capacity at year 1,000 | Density over ice-free land | Range over the run | Plant share of energy |
| --- | --- | --- | --- | --- |
| first-light | 14.8 M (15.5 M with perfect storage) | 0.108/km² | 11.7–16.7 M | 52% |
| 5c151f46 | 11.7 M (12.8 M) | 0.083/km² | 10.9–14.9 M | 56% |
| 4836187e | 20.1 M (20.5 M) | 0.139/km² | 16.4–23.7 M | 51% |

Median year-round capacity by vegetation class, people per 100 km² (lean-season
limit in parentheses):

| Class | first-light | 5c151f46 | 4836187e | Observed foragers |
| --- | --- | --- | --- | --- |
| Savanna | 23.6 (1.00) | 21.0 (1.00) | 24.4 (1.00) | about 10–30 |
| Tropical seasonal forest | 22.5 (1.00) | 19.7 (1.00) | 20.8 (1.00) | about 10–20 |
| Grassland | 14.5 (1.00) | 11.4 (1.00) | 17.1 (1.00) | about 5–15 |
| Rainforest | 7.6 (1.00) | 7.2 (1.00) | 8.0 (1.00) | about 5–20 |
| Temperate forest | 5.6 (0.82) | 3.9 (0.75) | 5.3 (0.82) | about 5–15 |
| Deserts | 2–11 (1.00) | 2–4 (1.00) | 3–5 (1.00) | about 1–20 |
| Tundra | 1.9 (0.64) | 1.8 (0.64) | 2.0 (0.64) | about 0.5–2 |
| Boreal forest | 0.5 (0.25) | 0.5 (0.24) | 0.6 (0.24) | about 0.5–3, mostly with fish |

The observed ranges are orders of magnitude for terrestrial hunter-gatherers
compiled in ethnographic surveys (Binford 2001; Kelly 2013); they are a sanity
band, not a fit. Earth's pre-agricultural population is usually estimated at a few
million to about ten million, including peoples who lived largely on fish;
terrestrial-only capacity here is at the upper end of that band.

- Forager capacity responds to droughts: the land total falls up to about a quarter
  below year 0 in the driest years and recovers within years. In a single marginal
  savanna region a drought year can cut capacity by more than 90%, because herb
  cover collapses in `vegetation-2`.
- Seasons limit cold regions: in boreal forest the year-round capacity is a quarter
  of the perfect-storage value, so storage (not modeled) decides how many people
  such regions could feed.
- The food step costs about 6 ms per year at 40,962 regions on the shared test
  machine (8 threads), so 1,000 years add about 6 s to a vegetation run.

## Acceptance checks

Native tests in `native/src/wild_food.rs`:

- monthly growth shares sum to one and are zero in months below 0 °C;
- capacity lies between the self-sufficient and stored bounds, is exactly the
  feasibility limit, grows with game and approaches the stored value for evenly
  spread food;
- a closed canopy lowers plant food, browse supports less game than herbs, lower
  productivity lowers game capacity, and the offtake peaks at half the capacity;
- three dry years cut the game stock below 65% of capacity after a first year above
  80%, and recovery to 95% takes at least five years;
- a group at 80% of the sustainable size never goes short and keeps the stock above
  40% of the untouched stock; four times that size depletes it below 10% with more
  than 30% unmet need, and the stock recovers above 90% within 30 years after the
  group leaves;
- the run advances with vegetation, rejects out-of-order years, and region replays
  equal the run.

`tests/wild-food.test.ts` checks the protocol: the stock starts at capacity, water
regions have no capacity, dry years leave game below capacity, region series match
the frame, heavy foraging depletes game and it recovers after the group leaves,
invalid experiments are rejected, and regeneration repeats the run exactly.

## Limitations

- No aquatic food. Coasts, lakes and rivers were the richest foraging grounds on
  Earth; their absence makes boreal and coastal regions far too poor.
- No human storage, processing, fire management or trade.
- Game is one aggregate herbivore stock per region without migration between
  regions, predators as a stock, or disease.
- Plant food shares and the game share left to people are calibrated to observed
  forager densities; they are not measured edible fractions.
- Tundra capacity is probably high: Miami productivity is generous at low mean
  temperatures and there is no winter forage limit beyond the capacity memory.
- Inherits `vegetation-2` limitations, including the strong herb dieback in dry
  years and precipitation from `climate-normals-1` rather than a water budget.
- The run is not saved in checkpoints; the recipe and year count reproduce it.
