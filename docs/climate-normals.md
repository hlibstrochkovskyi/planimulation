# Climate normals and biomes: `climate-normals-1`

November 16, 2026. First Milestone E increment. A read-only product computed on
demand from a generated world (about 1.5 s at 40,962 regions); it is not part of
the world recipe or fingerprint and never changes physical fields.

## Why a prescribed climatology

Biomes need annual temperature and precipitation normals. Measured on the
default world (`native/examples/climate_year.rs`, first-light), the existing
models are not usable for that:

- the baseline seasonal-water model produces 283 mm/year of global
  precipitation (Earth: about 1,000), 419 mm of ocean evaporation (Earth: about
  1,200) and 22 mm of land evaporation; 78% of land receives under 250 mm by the
  second year, and the land keeps drying;
- one simulated year takes about 2.5 minutes (the unified soil model about
  36 minutes);
- `seasonal-temperature-1` compresses the latitudinal range (equatorial land
  about 17 °C, polar land about −5 °C), and its linear insolation response makes
  polar summers hot.

The owner chose a fast, explainable prescribed climatology of the same kind as
the existing temperature and wind normals instead of calibrating the water
budget. Calibrating the seasonal-water model remains separate, open work; the
two products are labeled differently in the interface.

## Model

**Temperature.** Monthly means per region:

- a prescribed annual sea-level profile by latitude (26.5 °C at the equator,
  19 °C at 30°, 10 °C at 45°, 0 °C at 60°, −25 °C at the poles) stands in for
  atmospheric and oceanic heat transport;
- a seasonal anomaly of 0.075 °C per W/m² of daily top-of-atmosphere insolation
  departure from the annual mean, lagged by 20 days on land and 60 days on water;
- water damps the anomaly to 35%; land damps it toward 50% where onshore air
  arrives straight from water (maritime versus continental climates, using the
  same upwind path as moisture supply);
- −6.5 °C per kilometer of height above the initial water level.

**Precipitation.** For each month, `zonal potential × moisture supply × windward lift`:

- the zonal potential (mm/year by absolute latitude: 2,400 at the equator, 550 at
  30°, 1,150 at 50°, 150 at the poles) follows the sun with the prescribed wind
  model's half-declination shift, giving monsoon-like seasonality;
- moisture supply is `0.15 + 0.85 · exp(−D / 1,700 km)`, where `D` is the cheapest
  path from any water region over neighbor edges. An edge costs its length in calm
  air or with the wind, up to three times its length against strong wind
  (`seasonal-wind-1` monthly normals), plus 500 m per meter of climbing (rain
  shadow);
- windward lift is `1 + 0.0015 × rise` above the most upwind neighbor, capped at 2.5.

**Biomes.** Köppen-style rules on annual mean, warmest and coldest month,
annual precipitation and driest month:

| Rule, in order | Class |
| --- | --- |
| warmest month < 0 °C | Ice |
| warmest month < 10 °C | Tundra |
| precipitation < 20 × mean + 140 mm | Hot desert (mean ≥ 18 °C) or cold desert |
| precipitation < twice that threshold | Savanna (mean ≥ 18 °C) or temperate grassland |
| coldest month ≥ 18 °C | Tropical rainforest (driest month ≥ 60 mm) or tropical seasonal forest |
| coldest month > −3 °C or warmest ≥ 22 °C | Temperate forest |
| otherwise | Boreal forest |

The inspector shows each factor and the rule that decided the class.

## Display

The **Biomes** layer draws land in class colors with the same shoreline,
bathymetry, relief shading and drainage lines as the surface layer. Region colors
are softened with dry neighbors for display only; classes are unchanged. The
**Annual precipitation** layer uses a fixed logarithmic scale up to 4,000 mm.

## Measurements

Default new-world recipe (`multiscale-geography-4`, 40,962 regions),
`cargo run --release --example climate_normals_report -- SEED`:

| Seed | Global / land mean precipitation | Ice + tundra | Boreal | Temperate forest | Grassland | Deserts | Savanna | Tropical forest |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| first-light | 996 / 635 mm | 27% | 12% | 5% | 18% | 9% | 21% | 8% |
| 5c151f46 | 991 / 656 mm | 28% | 19% | 8% | 17% | 2% | 17% | 10% |
| 4836187e | 972 / 765 mm | 11% | 6% | 8% | 17% | 14% | 33% | 12% |

Earth for orientation: global precipitation about 1,000 mm, land about 750 mm;
deserts roughly a quarter to a third of land, tropical forests roughly a tenth.

## Limitations

- Precipitation is not conserved: it is not linked to evaporation, the
  seasonal-water stocks or river discharge.
- No ocean currents (no cold-current coastal deserts or warm western coasts
  beyond the maritime damping), no sea ice, snow albedo or monsoon dynamics.
- Hot deserts are under-represented (0.4–4% of land on the review seeds) and
  savanna over-represented; the subtropical dry belt is weaker than Earth's.
- High-latitude maritime coasts tend toward tundra.
- Classes are an instant climate classification; vegetation growth, delays,
  disturbance and soils (Milestone E proper) are not modeled.
