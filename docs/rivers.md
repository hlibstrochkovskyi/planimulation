# Climate-normal runoff and river discharge

`rivers-1` provides monthly runoff depth, river discharge, and annual terminal
balances from [climate normals](climate-normals.md), [C1 drainage](drainage.md),
and the [bed connectivity hierarchy](basin-inspection.md). It is a deterministic,
read-only analysis that the native session caches for each world. The result
includes the model versions, resolved settings, and month lengths. It uses no
random stream and lives outside recipes, fingerprints, and experiment
checkpoints. Terrain, initial water, climate normals, and seasonal-water
experiments keep their own inputs and states.

Water availability is one requirement for population readiness in
[DESIGN.md](../DESIGN.md). The analysis distinguishes locally generated runoff
from the water that reaches a region from upstream. Later ecology and food
models can use those fields. They do not currently consume them.

## Land water balance

The model uses the 12 months of `climate-normals-1` on its circular 365-day
calendar. Day zero is the northern spring equinox. Temperatures are monthly
means in °C and precipitation is a monthly depth of liquid water equivalent
in mm.

**Potential evapotranspiration (PET).** The monthly Oudin approximation uses
`PET = Ra × max(T + 5, 0) / (2.45 × 100)`. `Ra` is extraterrestrial radiation in
MJ/m² summed for the month by multiplying the mid-month day's mean flux by the
number of days. The existing insolation calculation uses latitude, the 23.44°
tilt, and 1,360 W/m² and handles polar day and night. The latent heat term is
2.45 MJ/kg; 1 kg/m² of water is 1 mm. A day at 20 °C and 30 MJ/m² gives
3.06 mm/day, and a day below −5 °C or without insolation gives zero. This
temperature/radiation method needs neither humidity nor a daily temperature
range. [Oudin et al. (2005)](https://doi.org/10.1016/j.jhydrol.2004.08.026)

**Snow.** At a monthly mean of 0 °C or below, precipitation enters a snow pack.
Above 0 °C, precipitation is liquid, and existing snow melts by up to
3 mm per °C per day. The annual snow map is a translation floored by the end
stock for a year started empty. Starting at that floor produces the periodic
snow cycle wherever melt can balance snowfall. If snowfall exceeds the full
year's melt capacity, the excess is reported as ice accumulation. This also
handles ice sheets. With every month at or below freezing, all precipitation
accumulates and the region generates zero runoff. Glacier flow, calving, and
sublimation are outside this monthly balance.

**Soil and actual evapotranspiration (AET).** Rain and melt join water carried
over from the previous month. AET withdraws up to PET; the remaining water fills
a uniform 100 mm root-zone bucket, with any excess becoming runoff. The capacity
represents an order-of-magnitude plant-available soil stock. This model takes
no soil or vegetation input. Monthly updates clamp a translated stock to
`[0, 100]`; their composition over a year is also a translated clamp. For
nonpositive annual translation, the end of one pass from empty is the fixed
point. For positive translation, the end of one pass from full is the fixed
point. The computed normal year starts at that exact periodic state, up to
rounding. For each land region over the year,

```text
P = AET + runoff + change in snow/ice and soil water
0 ≤ AET ≤ min(P, PET)
0 ≤ runoff
```

## Drainage and terminal balances

Each monthly runoff depth is converted to m³ with the native reference cell
area and added in topological order down the frozen C1 receivers. Flow is
instantaneous within each month. Monthly discharge is the month's volume
divided by its seconds. Mean discharge is the annual volume divided by
31,536,000 s; it is a duration-weighted mean of the twelve monthly rates.
Each wet cell reports its own receiving flow, and a water body's total
inflow is the sum across its wet cells. The largest initial water body is the
ocean outlet.

**Spill connections.** The bed hierarchy records when sublevel components first
join and provides adjacent sill contacts. The river analysis contracts the
static catchments by their canonical outlet and uses all cross-catchment
contacts at `max(bed[from], bed[to])`. This completes the contacts for tied
plateaus and for original bed branches split by water-body terminals. Each
catchment's exit follows the path to the ocean with the lowest maximum saddle
height. Ties prefer a lower local saddle, a stored hierarchy contact, then
deterministic region ID order among candidates. Each chosen edge points to an
already settled catchment, so the overflow graph is acyclic. On a world
without an ocean the lowest bed outlet anchors the graph. Surplus reaching
that root is recorded as terminal storage.

**Evaporation footprints.** An existing inland lake uses its initial wet area
and receives direct climate-normal precipitation on that area. A dry sink uses
the area of its original catchment below its local sill, including its sink
cell, with a 10% cap on the fraction of contributing land assigned to open
water. The below-sill footprint is only an evaporation proxy. It is applied
proportionally across that footprint, with no change to land runoff or
geography. Potential open-water evaporation is 1.15 × Oudin PET in months
above 0 °C. The 15% allowance represents a modest increase in evaporative
demand over open water. For the proxy at a dry sink only the positive
difference from AET already counted on that fraction is available to consume
runoff. Its precipitation has already entered the land budget.

**Annual equilibrium.** From upstream to downstream, each terminal uses incoming
volume plus its direct lake precipitation to satisfy its annual evaporation
capacity. A terminal in deficit stays closed. With a connected sill and excess
water, it sends the entire surplus across the sill into the receiving cell
and along that catchment's static receivers. The surplus is assigned to months
in the same proportions as combined monthly input, an approximation allowing
seasonal storage to supply annual evaporation. The balance records inflow,
direct precipitation, potential and actual evaporation, spill, and
accumulation. A mixed-water tracer partitions land runoff among ocean delivery,
closed terminals, and evaporation in lakes that also spill. The terminal label
follows the path of any surplus. Endorheic area is the dry surface whose final
destination is a closed terminal.

## Desktop and transport

**Runoff** shows locally generated land runoff on a linear 0–1,000 mm/year
scale. **River discharge** shows accumulated mean flow on a fixed
`log10(1 + Q)` scale through 100,000 m³/s. Both layers use the same fields in the
flat map and globe. The river overlay uses increasing line widths at
300, 1,000, 3,000, and 10,000 m³/s. Smaller streams remain inspectable. Rivers
with a maximum monthly discharge below 1 m³/s are classed as **None**. A minimum
of at least 1 m³/s is **Perennial**. The remaining rivers are **Seasonal /
ephemeral** and are drawn thinner and lighter. Area-weighted drainage lines
serve as the fallback while river normals are unavailable.

Inspect a region for PET, land AET and runoff, mean/minimum/maximum discharge
and months of the extrema (first on ties), flow class, and ocean or endorheic
destination. Inland terminals report their aggregate annual balances; a wet
terminal's balance applies to its entire body. A dry terminal's displayed
discharge is its incoming flow. Separate spill segments show the surplus from
the sink through the saddle. The reverse receiver path on the source side
indicates the spill connection through the basin footprint. World statistics
report the runoff fates and the land area draining to closed basins.

The additive `rivers` command and frame use protocol 9. The body encodes five
annual f64 fields, two region-major monthly f64 fields, region class and
terminal u32 fields, followed by sparse inland terminal balances and spill
segments. Metadata records settings and global totals. The dense block is
240 bytes per region, about 9.4 MiB for 40,962 regions. The decoder checks
calendar and model identities, bounds, monthly-to-annual aggregation, class
thresholds, destinations, and closure of the main budgets.

## Measurements

`cargo run --release --locked --manifest-path native/Cargo.toml --example rivers_report`
uses the default `multiscale-geography-4` geometry at subdivision 6 (40,962
regions). Depths are area-weighted over dry land. Ocean flow includes
precipitation contributed by spilling inland lakes. Timings are native analysis
only, starting with computed normals and ending before wire encoding.

| World | Land P (mm/year) | Land AET (mm/year) | Land runoff (mm/year) | Land ice gain (mm/year) | Ocean flow (km³/year) | Endorheic land | Native analysis |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| first-light | 635.3 | 524.2 | 92.2 | 18.9 | 13,501 | 5.9% | 32 ms |
| 5c151f46 | 655.5 | 531.0 | 114.3 | 10.2 | 17,113 | 2.4% | 34 ms |
| 4836187e | 764.5 | 682.6 | 70.5 | 11.4 | 10,821 | 13.2% | 32 ms |
| Earth reference | — | 480–550 | 250–320 | — | 37,000–45,000 | roughly 18–20% | — |

Earth reference ranges are comparison targets from the task. Published studies
estimate 44,200 ± 2,660 km³/year ocean inflow, a median terrestrial ET of
549 mm/year across water balance estimates, and endorheic area of approximately
one fifth of land. [Clark et al. (2015)](https://doi.org/10.1175/JHM-D-14-0183.1),
[Xiong et al. (2023)](https://doi.org/10.5194/essd-15-4571-2023),
[Wang et al. (2018)](https://doi.org/10.1038/s41561-018-0265-7)

| Seed | Land runoff reaching ocean | Land runoff reaching closed terminals | Land runoff evaporating in spilling lakes | Overflowing / closed inland terminals |
| --- | ---: | ---: | ---: | ---: |
| first-light | 98.03% | 1.01% | 0.96% | 14 / 5 |
| 5c151f46 | 99.61% | 0.08% | 0.30% | 29 / 9 |
| 4836187e | 98.65% | 0.30% | 1.05% | 21 / 25 |

Largest ocean mouth cells by mean discharge in m³/s, with region IDs in
parentheses. A receiving cell can aggregate several neighboring reaches.

| Rank | first-light | 5c151f46 | 4836187e |
| ---: | ---: | ---: | ---: |
| 1 | 17,542 (35050) | 41,221 (28145) | 24,548 (28512) |
| 2 | 11,580 (3759) | 18,959 (30004) | 9,554 (6269) |
| 3 | 10,730 (28550) | 17,449 (27320) | 9,538 (28529) |
| 4 | 9,873 (14512) | 9,745 (7699) | 6,823 (29478) |
| 5 | 9,336 (7266) | 8,570 (13184) | 4,971 (3482) |
| 6 | 8,654 (26084) | 8,443 (30480) | 4,614 (24837) |
| 7 | 7,407 (3756) | 7,472 (1840) | 4,100 (30416) |
| 8 | 7,291 (14598) | 6,120 (15049) | 4,053 (29469) |
| 9 | 6,893 (13764) | 5,912 (14939) | 3,630 (6138) |
| 10 | 6,531 (29248) | 5,771 (7226) | 3,370 (7545) |

The largest outlets span Mississippi- to Congo-scale flows (roughly 17,000 and
41,000 m³/s), and each seed has an upper tail above the next few mouths.
No outlet is Amazon-scale (roughly 209,000 m³/s). The overall flow distribution
is plausible for a drier set of worlds with this geography, with a compressed
upper tail relative to Earth. [Sperna Weiland et al. (2012), Table 2](https://doi.org/10.5194/hess-16-1047-2012)

Ocean flow is 2.2–4.2 times below the Earth reference range across these seeds.
The first two have AET in the reference range, but land precipitation is only
635–656 mm/year. Holding the measured ice gain and using even 480 mm/year of
AET leaves at most 136 and 165 mm/year of runoff, respectively. The third has
more precipitation, but AET consumes about 89% of it. Its low-runoff result
also depends on the spatial and seasonal overlap of rainfall and high PET,
and on the bucket giving monthly AET first access to available water. Lake
retention cannot explain the ocean-flow deficit. The ocean receives 98.0%,
99.6%, and 98.6% of land runoff, respectively. The precipitation climatology
was used as supplied.

## Acceptance and interpretation

- Native tests check the Oudin reference, cross-year snowmelt, persistent
  ice storage, exact periodic soil accounting, land and global conservation,
  monotonic monthly discharge along static routes, an independently summed
  coastal catchment, zero precipitation, a dry sink spilling into a wet lake,
  closure under arid forcing, and the fully wet and oceanless cases.
- The node test checks the protocol, both runoff-ledger closures, monthly
  discharge classes, adjacency of spill segments, regenerated fingerprints
  and stale model replacement, and malformed frames. Desktop coverage checks
  both layers, line mode, inspector values, and the unchanged fingerprint.
- Run `node --import tsx scripts/rivers-review.ts [OUTPUT_DIR] [seed]` after
  building the desktop to capture both atlas layers, a terminal inspector,
  and globe discharge. The default path is `artifacts/rivers-1-review/`.

The solution is a climatic normal-year budget with prescribed evaporation
areas. Lake levels, capacity to reach the sill, seasonal filling history,
groundwater baseflow, and channel hydraulics are outside the model. A small
annual surplus connects a basin even if reaching its sill would take many years
or require a larger evaporating area. The low endorheic area makes sensitivity
to the area cap and basin spill representation a review question. Bucket
capacity and within-month partitioning are further sensitivity questions,
especially where warm rain arrives in brief storms. The line widths are map
symbology and have no physical effect.
