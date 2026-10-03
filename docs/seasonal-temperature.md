# Static seasonal temperature normals

`seasonal-temperature-1` is the first bounded climate increment. It derives a repeatable 365-day temperature cycle from a generated world's latitude, initial land/water mask, and elevation. The desktop displays twelve monthly *normals* on the same flat map and globe, with a fixed annual color scale and regional inspector. The native core can produce the same data without the renderer. This is not a weather simulation, timed climate state, or a complete planetary energy balance.

## Model contract

Day 0 is a northern spring equinox. The year is a circular 365-day orbit divided into twelve contiguous bins by `floor(day × 12 / 365)`; these are numbered months, not a terrestrial calendar. With axial tilt ε, orbital phase θ, latitude φ, and solar irradiance S₀:

```text
δ = asin(sin ε · sin θ)
cos H = clamp(-tan φ · tan δ, -1, 1)
Q = S₀/π · (H sin φ sin δ + cos φ cos δ sin H)
```

The code evaluates the equivalent expression using `sin φ` and handles polar day/night explicitly. `Q` is daily-mean *top-of-atmosphere* flux in W/m², not absorbed surface energy. The geometry follows [Purdue's daily-insolation derivation](https://web.ics.purdue.edu/~dchavas/teaching/seasons.html); 1360 W/m² and the approximate 340 W/m² spherical mean are consistent with [NASA's Earth energy-budget overview](https://science.nasa.gov/earth/earth-observatory/climate-and-earths-energy-budget/). NASA also explains why ocean heat storage can delay seasonal response, but the response times here are **model choices**, not fitted ocean measurements ([NASA Earth Observatory](https://science.nasa.gov/earth/earth-observatory/earths-big-heat-bucket/)).

```text
target(day, region) = Tref + k · (Q - S₀/4) - lapse · max(initial dry elevation, 0)
Tnext = exp(-1/τ) · Tcurrent + (1 - exp(-1/τ)) · target
```

Wet regions use the initial generated water mask and have no elevation penalty. `τ` is 20 days on dry land and 60 days on water. The annual periodic fixed point is solved algebraically, so there is no arbitrary spin-up count or dependence on a prior run. Monthly means are averages of the daily response; annual minimum and maximum are extrema of **daily**, not monthly, values. Settings and model version travel with the output. No random stream is consumed.

Default settings: tilt 23.44°, S₀ 1360 W/m², reference temperature 15 °C, sensitivity 0.09 °C per W/m², dry elevation lapse 0.0065 °C/m, and 20/60-day response times. Only S₀ is taken as an Earth-scale reference; the reference temperature, sensitivity, lapse, and response times are provisional parameters. The centered forcing and fixed reference do not make global mean temperature exactly 15 °C on a nonuniform land/water planet. They are not automatically adjusted to match an ocean-coverage or temperature target.

## Reproduce and inspect

```sh
cargo run --release --manifest-path native/Cargo.toml --example seasonal_temperature_report -- docs/scenarios/seasonal-temperature.json
cargo test --manifest-path native/Cargo.toml --test seasonal_temperature
```

The JSON report includes the exact recipe and settings, model version, monthly day counts, global and hemispheric area-weighted monthly means, and annual land/water area-weighted means. Land or water means are `null` if that surface type has zero area. The desktop requests the derived fields once after accepting a world; changing month or projection changes only presentation and does not change the initial-world fingerprint. Manual water inventory changes do **not** recompute these fixed-initial-geography normals.

Tests cover insolation geometry and polar night, area-weighted spherical flux convergence, reproducibility, periodic/monthly consistency, opposite hemispheric seasons, zero tilt, thermal lag, elevation response, settings rejection, protocol validation, and desktop display. These tests check model behavior, not calibration against observed climate.

## Open gates

There is no albedo, atmosphere, clouds, greenhouse balance, evaporation, wind, precipitation, snow, soil moisture, or water-cycle coupling. The land/water response is a local filter rather than heat transport. Tectonic features affect temperature only through final elevation; ocean proximity does not moderate neighboring land. Climate settings are fixed and versioned for this increment rather than exposed as planet-generation controls. Before using temperature as an ecological or hydrological input, evaluate ensembles across seeds and resolutions, calibrate stated parameters against appropriate observations or targets, and add the missing coupled mechanisms with separate conservation checks.
