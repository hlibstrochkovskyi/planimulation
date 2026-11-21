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

**Soil and actual evapotranspiration (AET).** The uniform root-zone
capacity is 100 mm of plant-available water. Each month distributes rain plus
melt and PET evenly across four deterministic wetting and drying pulses. This
approximates wetting within the month from monthly inputs. The previous model
let monthly AET consume all available water before producing any runoff,
which suppressed runoff from wetting events even on moist ground.

The HBV-type recharge share is `(S / C)^2` for soil stock `S` and capacity
`C`. Integrating its complement, `dS/dL = 1 − (S / C)^2`, during a wetting
pulse lets recharge rise as that pulse wets the soil. With `s = S / C` and
`w = tanh(L / C)` for liquid input `L`, infiltration is
`C × (1 − s²) × w / (1 + s × w)`. The rest of the pulse generates runoff
immediately. This applies the HBV soil-moisture rule without a separate
groundwater response reservoir. AET then draws up to pulse PET, scaled by
`min(1, S / (0.7 × C))` at the post-wetting stock and bounded by the
available water. The capacity, stress threshold and pulse count are uniform
approximation parameters. [Lindström et al. (1997)](https://doi.org/10.1016/S0022-1694(97)00041-3),
[HBV-light manual](https://www.geo.uzh.ch/dam/jcr%3Ac8afa73c-ac90-478e-a8c7-929eed7b1b62/hbv_manual_2005.pdf)

Each pulse and thus each month conserves liquid input plus starting soil
stock as runoff, AET and ending soil stock. The annual map is monotone and
nonexpansive on `[0, C]`. Starting with an empty stock, up to four normal
years try to close the cycle. When the annual residual stays above
`10⁻¹⁰ mm`, bisection brackets the crossing of ending and starting stock
to that same precision. With no PET, a full bucket is periodic and all
liquid input generates runoff. Annual land storage records the snow/ice change.
For each dry land region,

```text
P = AET + runoff + snow/ice gain      (up to the soil-solver and rounding tolerance)
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

**Runoff** shows locally generated land runoff on a fixed `log(1 + depth)`
scale with color anchors at 0, 10, 50, 200, 600 and 2,000 mm/year. The palette
runs from warm dry tones through muted green to deep blue. **River discharge**
draws the network over neutral shaded relief with bathymetric water. Mean
discharge sets line width, color and opacity in five logarithmically spaced
classes starting at 300, 1,000, 3,000, 10,000 and 30,000 m³/s. Perennial reaches
are blue. Seasonal reaches are thinner and teal. The same lines appear on
**Land and water**, while **Runoff** shows the local depth field alone. These
displays use the same regional inputs and paths on the flat map and globe.

Rivers with a maximum monthly discharge below 1 m³/s are classed as **None**.
A minimum of at least 1 m³/s is **Perennial**. The remaining rivers are
**Seasonal / ephemeral**. Streams below the display cutoff remain inspectable.
Area-weighted lines serve as a fallback while river normals are unavailable.

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
uses the three review seeds at subdivision 6 (40,962 regions) with the default
`multiscale-geography-4` geometry. Depths are area-weighted over dry land.
Ocean flow includes precipitation contributed by spilling inland lakes.
Timings cover native analysis with normals already computed, before wire
encoding.

| World | Land P (mm/year) | Land AET (mm/year) | Land runoff (mm/year) | Land ice gain (mm/year) | Runoff/P | AET/P | Ocean flow (km³/year) | Endorheic land | Native analysis |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| first-light | 635.3 | 446.6 | 169.8 | 18.9 | 26.7% | 70.3% | 24,842 | 0.048% | 137.4 ms |
| 5c151f46 | 655.5 | 444.3 | 201.0 | 10.2 | 30.7% | 67.8% | 29,750 | 0.000% | 144.1 ms |
| 4836187e | 764.5 | 587.8 | 165.2 | 11.4 | 21.6% | 76.9% | 24,540 | 2.750% | 79.8 ms |

The original monthly bucket yielded 92.2, 114.3 and 70.5 mm/year of runoff
on these worlds. The new partition increases the runoff coefficients from
14.5%, 17.4% and 9.2% to the values above. The inherited land precipitation
is 635–765 mm/year under the frozen `climate-normals-1`.

For orientation, terrestrial ET estimates have a median near 549 mm/year.
Published global water budgets report runoff coefficients around 35–41%,
ocean flow estimates include 44,200 ± 2,660 km³/year, and roughly one fifth
of Earth's land is endorheic.
[Trenberth et al. (2007)](https://staff.cgd.ucar.edu/trenbert/trenberth.pdf/GEWEX-JHydroMet8.pdf),
[van der Ent et al. (2010)](https://doi.org/10.1029/2010WR009127),
[Clark et al. (2015)](https://doi.org/10.1175/JHM-D-14-0183.1),
[Xiong et al. (2023)](https://doi.org/10.5194/essd-15-4571-2023),
[Wang et al. (2018)](https://doi.org/10.1038/s41561-018-0265-7)

Using an Earth reference of 760 mm/year and a 0.35 runoff coefficient from
the Trenberth budget gives 266 mm/year of runoff. The table decomposes this
depth gap as `0.35 × (760 − P) + (0.35 × P − Q)`, where `Q` is modeled
runoff depth. The first term isolates precipitation at the reference
coefficient. The second includes the allocation to AET and permanent ice
within the model. This arithmetic comparison does not rerun the climate.

| World | Runoff gap (mm/year) | Precipitation term (mm/year) | Share of gap | Remaining term (mm/year) |
| --- | ---: | ---: | ---: | ---: |
| first-light | 96.2 | 43.6 | 45% | 52.6 |
| 5c151f46 | 65.0 | 36.6 | 56% | 28.4 |
| 4836187e | 100.8 | −1.6 | −2% | 102.4 |

For the third seed, annual precipitation slightly exceeds the reference. More
of its precipitation falls in climates with high PET relative to precipitation.
Snow/ice gain represents 1.5–3.0% of land precipitation across the three worlds.
The ocean receives 95.6–98.9% of generated land runoff. Ocean flow remains
26–39% below the 40,000 km³/year Trenberth reference. The largest ocean
receiving cells carry 33,730, 46,312 and 37,154 m³/s, respectively. The report
prints the ten highest-flow mouths with their region IDs for each world.

| World | Land runoff reaching ocean | Land runoff reaching closed terminals | Land runoff evaporating in spilling lakes | Overflowing / closed inland terminals |
| --- | ---: | ---: | ---: | ---: |
| first-light | 95.57% | 0.01% | 4.42% | 18 / 1 |
| 5c151f46 | 98.87% | 0.00% | 1.13% | 38 / 0 |
| 4836187e | 95.84% | 0.34% | 3.82% | 41 / 5 |

### Regional Budyko comparison

The annual diagnostic uses Fu's curve
`AET/P = 1 + φ − (1 + φ^ω)^(1/ω)`, where `φ = PET/P`. The reference is
`ω = 2.6` and the illustrative catchment span is `ω = 2–4`. The span is a
comparison, not a fitted confidence interval. For each modeled region without
permanent snow/ice gain and with positive precipitation, calculate its
aridity and its three Fu values. The table bins regions by aridity and
weights both modeled ratios and Fu values by precipitation volume. This
compares the curves at each region's aridity before taking the mean. The
excluded areas are 10.5%, 7.0% and 4.7% of dry land, respectively. Fu's curve
is a long-term catchment relationship, so this is a regional plausibility
check using gridded inputs and Oudin PET.
[Zhang et al. (2004)](https://doi.org/10.1029/2003WR002710)

| World | Aridity band | Land area | PET/P | AET/P | Fu (2.6) | Fu (2–4) |
| --- | --- | ---: | ---: | ---: | ---: | ---: |
| first-light | <0.5 | 13.6% | 0.352 | 0.333 | 0.324 | 0.289–0.347 |
| first-light | 0.5–0.8 | 11.1% | 0.639 | 0.497 | 0.526 | 0.450–0.596 |
| first-light | 0.8–1.2 | 17.8% | 1.027 | 0.741 | 0.701 | 0.592–0.817 |
| first-light | 1.2–2 | 32.5% | 1.521 | 0.845 | 0.816 | 0.697–0.927 |
| first-light | 2–3 | 12.8% | 2.372 | 0.908 | 0.904 | 0.796–0.980 |
| first-light | ≥3 | 1.8% | 3.184 | 0.941 | 0.940 | 0.846–0.992 |
| first-light | all | 89.5% | 1.252 | 0.729 | 0.707 | 0.606–0.802 |
| 5c151f46 | <0.5 | 13.9% | 0.395 | 0.364 | 0.359 | 0.318–0.388 |
| 5c151f46 | 0.5–0.8 | 20.4% | 0.645 | 0.488 | 0.529 | 0.453–0.601 |
| 5c151f46 | 0.8–1.2 | 23.5% | 1.012 | 0.733 | 0.696 | 0.587–0.811 |
| 5c151f46 | 1.2–2 | 29.9% | 1.497 | 0.837 | 0.812 | 0.693–0.925 |
| 5c151f46 | 2–3 | 5.2% | 2.263 | 0.888 | 0.897 | 0.787–0.977 |
| 5c151f46 | ≥3 | <0.1% | 3.025 | 0.941 | 0.936 | 0.839–0.991 |
| 5c151f46 | all | 93.0% | 1.092 | 0.690 | 0.676 | 0.577–0.771 |
| 4836187e | <0.5 | 7.1% | 0.351 | 0.329 | 0.323 | 0.288–0.346 |
| 4836187e | 0.5–0.8 | 6.9% | 0.638 | 0.486 | 0.525 | 0.449–0.595 |
| 4836187e | 0.8–1.2 | 19.1% | 1.043 | 0.757 | 0.706 | 0.596–0.823 |
| 4836187e | 1.2–2 | 39.8% | 1.511 | 0.858 | 0.814 | 0.695–0.926 |
| 4836187e | 2–3 | 17.9% | 2.372 | 0.926 | 0.904 | 0.796–0.980 |
| 4836187e | ≥3 | 4.5% | 3.287 | 0.962 | 0.943 | 0.851–0.993 |
| 4836187e | all | 95.3% | 1.386 | 0.784 | 0.748 | 0.642–0.848 |

Every band mean falls within the reference span. The largest departure from
the central curve is in the `0.8–1.2` band for `4836187e`. The model
consumes 0.757 of precipitation there versus 0.706 on the central curve.

### Dry-terminal evaporation sensitivity

The entries show the percentage of dry land ultimately assigned to a closed
terminal, the number of closed inland terminals, and ocean flow. Only the
dry-sink footprint cap varies. Initially wet lakes keep their own footprints.

| World | 5% cap (area / count / km³/year) | 10% cap (area / count / km³/year) | 20% cap (area / count / km³/year) |
| --- | ---: | ---: | ---: |
| first-light | 0.048% / 1 / 24,882 | 0.048% / 1 / 24,842 | 0.599% / 2 / 24,814 |
| 5c151f46 | 0.000% / 0 / 29,849 | 0.000% / 0 / 29,750 | 0.330% / 2 / 29,625 |
| 4836187e | 2.719% / 4 / 24,598 | 2.750% / 5 / 24,540 | 3.847% / 10 / 24,508 |

The modeled closed area is below the old-bucket range of 2.4–13.2% because
more runoff now exceeds terminal evaporation. Doubling the cap from 10% to
20% raises closed area by at most 1.10 percentage points, well short of the
terrestrial comparison. The default remains 10%. Wide departures call for
review of basin extents, spill equilibrium and the distribution of dry
climates before retuning the footprint cap.

## Acceptance and interpretation

- Native tests check the Oudin reference, cross-year snowmelt, persistent
  ice storage, monthly soil closure, increasing runoff with saturation, the
  periodic soil balance, a generated land Fu-band check, land and global
  conservation, monotonic discharge along static routes, an independently summed
  coastal catchment, zero precipitation, a dry sink spilling into a wet lake,
  closure under arid forcing, and the fully wet and oceanless cases.
- The node test checks the protocol, both runoff-ledger closures, monthly
  discharge classes, adjacency of spill segments, regenerated fingerprints
  and stale model replacement, and malformed frames. Desktop coverage checks
  both layers, line mode, inspector values, and fingerprint preservation.
- Run `node --import tsx scripts/rivers-review.ts [OUTPUT_DIR] [seed]` after
  building the desktop to capture the atlas and globe on Runoff, River
  discharge, and Land and water, plus a terminal inspector. The default output
  is `artifacts/rivers-2-review/`. The script also checks that switching off
  river lines changes the globe render.

The solution is a climatic normal-year budget with prescribed evaporation
areas. Lake levels, capacity to reach the sill, seasonal filling history,
groundwater baseflow, and channel hydraulics are outside the model. A small
annual surplus connects a basin even if reaching its sill would take many
years or require a larger evaporating area. The four soil pulses describe a
fixed within-month partition of monthly inputs. Actual storm timing, soil
variation, vegetation coupling and glacier runoff are absent. The line widths
are map symbology with no physical effect.
