# Prescribed seasonal surface-wind normals

`seasonal-wind-1` is a read-only monthly surface-wind field on the generated sphere. It supplies an eastward and a northward component in m/s for every region and month. The desktop shows resultant speed on both projections and the components in the inspector. A native headless report uses the same calculation. This wind derivation itself moves no air or moisture. A separate [conservative moisture-transport kernel](moisture-transport.md) now uses its prescribed monthly field in bounded headless column-water runs, without surface exchange or precipitation.

## Rationale and definition

[UCAR's global-circulation teaching material](https://scied.ucar.edu/sites/default/files/documents/globe_weather_curriculum-all.pdf) describes tropical trade easterlies, midlatitude westerlies, and polar easterlies. [NASA's ITCZ explanation](https://science.nasa.gov/earth/earth-observatory/the-intertropical-convergence-zone-703/) describes convergence of the trades and the seasonal movement of that zone. Those qualitative patterns motivate the field; the speeds and interpolation below are **explicit uncalibrated model choices**, not values derived from those sources.

Day zero, 365-day circular orbit, axial tilt, and twelve month bins match [`seasonal-temperature-1`](seasonal-temperature.md). For latitude φ and solar declination δ, the modeled convergence latitude is `0.5 δ`. Let `b = |φ − 0.5 δ|`. Smooth cubic interpolation joins the following signed belt knots, where positive eastward means a westerly wind and positive northward means flow toward the north:

| Distance from convergence latitude | 0° | 15° | 30° | 45° | 60° | 75° | 90° |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| Eastward component before pole taper, m/s | 0 | −7 | 0 | +10 | 0 | −5 | 0 |
| Meridional component away from shifted equator, m/s | 0 | −2 | 0 | +1 | 0 | −0.5 | 0 |

The meridional knot values are multiplied by the sign of `φ − 0.5 δ`; both components are multiplied by `max(cos φ, 0)` so wind vanishes smoothly at a geographic pole. Daily vectors are averaged component-wise into each month; the displayed speed is the norm of that monthly mean vector, not the mean of daily speeds. A local east/north vector can be converted to a 3D tangent vector without using map longitude, avoiding a seam discontinuity. The field consumes no RNG stream and does not modify the generated world or its fingerprint.

The displacement of all three belts with the ITCZ is a simplification. There is no rotation-period parameter, Coriolis calculation, pressure field, vertical motion, atmospheric mass or momentum conservation, topographic deflection, land/sea breeze, monsoon, or weather. A belt may cross a coast or a mountain unchanged. The field must not be interpreted as an observed or dynamically predicted wind map. The separate transport kernel defines shared-edge column-water fluxes and a source-free mass ledger, but coupled evaporation/precipitation accounting remains open. This velocity field alone creates no rain shadow.

## Reproduce and validate

```sh
cargo run --release --manifest-path native/Cargo.toml --example seasonal_wind_report -- docs/scenarios/seasonal-temperature.json
cargo test --manifest-path native/Cargo.toml --test seasonal_wind
```

The report records its recipe, wind and temperature model versions, resolved settings, day counts, and area-weighted monthly speed and components. Native tests check belt signs, shifted convergence, tangent geometry across the atlas seam and at poles, reproducibility, zero-tilt constancy, and parameter rejection. Bridge tests reject damaged version, length, and non-finite fields. Desktop tests check the speed layer on flat and globe views without changing the world fingerprint. These are behavioral checks, not climatological validation or resolution-convergence evidence for a future moisture solver. A terrain recipe alone does not pin an old climate algorithm after a future upgrade; retain this report for exact model provenance. Cross-version climate replay remains a separate persistence task.
