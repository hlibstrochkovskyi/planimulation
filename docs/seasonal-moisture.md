# Finite seasonal water exchange

`seasonal-moisture-3` couples prescribed monthly temperature/wind, conservative vapor transport, and the [typed surface-water closure](surface-water.md) to [delayed runoff transport and evaporating terminal stores](runoff-transport.md). It runs headlessly and through an opt-in [desktop viewer/controller](seasonal-water-desktop.md) on fixed initial geography. This is a bounded mobile-water model, not the general basin inventory, changing lakes, or full planetary climate. Milestone D remains incomplete.

The [version-2 record](seasonal-moisture-2.md) and its measured results remain historical evidence. Versions 1/2 checkpoints are rejected rather than silently reinterpreted. Original source-free moisture reports and manual basin-water accounting remain unchanged.

An October 4 [experimental version-4 upslope response](orographic-response.md) accelerates removal of existing supersaturation using terrain/wind, without changing these version-3 equations or defaults. It has independent schema/protocol pins, controls, and retained refinement failures. Baseline evidence below is not a validation of that new mode.

A subsequent [headless version-5 soil-precision candidate](soil-precision.md) persists normalized high/low components of the same soil stock and preserves legacy models. It passes the two original soil witnesses but rejects annual snow-ledger qualification; it has no desktop protocol and is not a default-mode upgrade.

## Initial ownership

The atmosphere starts dry. Liquid starts as a finite active partition of generated reference water:

```text
L_i = area_i × min(initial_water_depth_i, active_depth) × 1000 kg/m³
snow_i = soil_i = transit_i = terminal_water_i = vapor_i = 0
```

Active depth defaults to 1 m, accepts 0–10 m, and does not limit later local storage. Inactive original deep water is excluded and never replenishes this layer. The initial stock remains liquid even in cold regions: it is a non-equilibrated initial condition, not a freeze-up calculation.

Every kilogram has one owner: local liquid, snow water equivalent, soil, runoff in transit, terminal water, or vapor. Do not add this partition to the full immutable `World.water` reference or deliver its rain to the separate manual basin inventory while retaining it here. No withdrawal/conversion bridge to that inventory is implemented. Recipe-based restore requires an unmodified `World::generate(recipe)` origin; custom initial fields or forcing edits are unsupported.

The original wet/dry mask continues to select land-only soil processes. It does not change when terminal pools form. The legacy `pendingRunoffKilograms` field now means moving transit storage when routing is enabled; with routing disabled it retains version 2's absorbing local stock. `surfaceKilograms` still means local liquid only. Terminal arrivals enter the new separate `terminalWaterKilograms` field.

## Atmospheric and surface exchange

The existing equilibrium vapor-pressure expressions follow [Murphy and Koop (2005)](https://doi.org/10.1256/qj.04.94); their equations appear in the [version-2 record](seasonal-moisture-2.md) and [Baumgartner et al. (2022), Appendix A](https://acp.copernicus.org/articles/22/65/2022/acp-22-65-2022.html). Below 0°C use ice, otherwise liquid water. The effective isothermal column remains our uncalibrated approximation:

```text
q_capacity = e_sat × H_effective / (461.5 × temperature_kelvin)
C_i = area_i × q_capacity
E_potential = max(C − vapor, 0) × (1 − exp(−Δt/τ_evap))
P = max(vapor − C, 0) × (1 − exp(−Δt/τ_precip))
```

H defaults to 2,000 m (100–5,000 m); evaporation response defaults to five days (one hour–thirty days), precipitation response to six hours (one hour–ten days). Temperatures outside −100..50°C reject. Temperature/wind are fixed monthly `seasonal-temperature-1`/`seasonal-wind-1` fields, not dynamic circulation or weather. The standalone exported `exchange` function preserves its original analytic two-stock box semantics.

At positive temperature, terminal water satisfies potential evaporation first, capped by that donor. Remaining demand goes to local liquid and moisture-limited soil evaporation through the surface closure. Both donor debits credit the same vapor stock; the demand is not counted twice. At or below 0°C all evaporation stops; no pool freezing or snow sublimation is modeled. Precipitation enters snow at T ≤ 0°C and liquid otherwise; degree-day melt, infiltration, finite soil retention, and generated runoff retain their existing laws. Disabling evaporation/precipitation does not reset any stock.

Terminal evaporation uses the region's area as an effective receiving footprint. In a previously dry sink this is a deliberately coarse pooled-water approximation, not a computed wet fraction. Even tiny terminal pools are eligible, with actual evaporation capped by their water. No lake level/area curve, sea-ice physics, latent-heat feedback, terrain lifting, pressure/air-mass field, or groundwater is implemented.

## Coupling and delays

Caller advances accept 1–86,400 integer seconds, up to 3,650 days. Each coupled interval executes:

1. Half a local surface/atmosphere exchange, then half an interval of runoff routing.
2. A full conservative vapor-transport interval.
3. Another half exchange, then half an interval of runoff routing.

Arrivals after routing become available for evaporation in the next exchange, not retroactively in the preceding one. A routing interval reads old departure stocks: newly arrived water cannot depart again during that same routing pass. Transit receives generated runoff already removed from local liquid/soil. Closed sinks and existing wet self-receivers deliver to finite terminal stores at actual physical contact cells. Common water-body labels do not teleport incoming water to the minimum-ID cell.

The default coupled maximum is 3,600 seconds (supported setting 60–21,600), bounded by one sixth of each enabled atmospheric/surface response time and one third of the shortest nonterminal routing response. Because routing runs for half an interval, each routing pass is at most one sixth of its shortest response time. Coupled clocks align to this bound and day boundaries; vapor transport has its own outgoing-rate substeps. Identical physical ticks can be batched into daily calls without changing state. Monthly forcing follows the 365-day calendar.

Local operators are sequential approximations, not an exact or globally second-order coupled network solution. Time refinement compares all six stocks. Shorter spatial edges change delay and numerical dispersion; generated-world seed/resolution ensembles alone do not prove spatial convergence.

## Complete budgets and checkpoint

With transfers below cumulative per region:

```text
Σ(liquid + snow + soil + transit + terminal_water + vapor) = initial_mobile_water
liquid_i = initial_liquid_i + rain_i + melt_i − liquid_evaporation_i − infiltration_i − liquid_runoff_i
snow_i = snowfall_i − melt_i
soil_i = infiltration_i − soil_evaporation_i − soil_drainage_i
transit_i = liquid_runoff_i + soil_drainage_i + received_transit_i − sent_i
terminal_water_i = terminal_delivery_i − terminal_evaporation_i
E_i = liquid_evaporation_i + soil_evaporation_i + terminal_evaporation_i
P_i = rain_i + snowfall_i
Σ(vapor) = Σ(E − P)
```

In addition, incoming transit and terminal delivery at each region must match the cumulative departures of its actual upstream receivers. This graph-ownership check rejects a forged checkpoint that conserves global mass and individual transit identities but invents an arrival on another region.

Global mass tolerance remains 1e−12 × max(initial mobile water, 1 kg). Regional identities use 1e−12 times their largest initial/current condensed stock, cumulative atmospheric exchange, surface transfer, routing transfer, or terminal stock, with a 1 kg floor. Vapor has a separate 1e−12 tolerance based on vapor and cumulative atmospheric transfers. One routing pass and one surface operation each use 32 machine epsilons of their actual stock/transfer scale; one terminal evaporation debit uses 16 machine epsilons of its donor scale. Residuals are reported. No final mass correction, tolerance widening, global remainder, or redistribution repairs a failed budget. Binary64 stock arithmetic is not exact sub-ULP accounting.

Long-lived surface and runoff transfer ledgers use Kahan summation; signed correction arrays are saved, validated, and never counted as physical mass. E/P totals derive from their components. Correction values must be finite and no larger than four machine epsilons times the component total (1 kg floor).

Schema 3 records six stocks, clock, cumulative surface/routing transfers and their roundoff, E/P, full recipe, all resolved settings, and moisture/surface/runoff/transport/temperature/wind versions. Restore regenerates fixed geometry and forcing, then checks stocks, shapes, versions, settings, clock, soil capacity, terminal ownership, graph flow identities, and numerical budgets. Unknown JSON fields reject. All new pools/flows/corrections must be zero at initialization; disabling routing requires its pools and ledgers to remain zero. Terminal transit is zero after enabled routing.

The entire update is atomic, including evaporation already provisionally performed before a transport failure. Checkpoint round trips and continuation are exact within the same supported build; no general world checkpoint, migration support, historical reachability proof for edited files, or cross-platform bitwise guarantee is claimed.

## Reports and validation

```sh
cargo test --locked --manifest-path native/Cargo.toml --test runoff_transport --test surface_water --test seasonal_moisture --example seasonal_moisture_report
cargo run --release --locked --manifest-path native/Cargo.toml --example seasonal_moisture_report -- docs/scenarios/seasonal-temperature.json
cargo run --release --locked --manifest-path native/Cargo.toml --example seasonal_moisture_report -- docs/scenarios/seasonal-temperature.json 3650
cargo run --release --locked --manifest-path native/Cargo.toml --example seasonal_moisture_report -- --ensemble
```

Report version 3 contains baseline daily caller advances (internally hourly or smaller), a fifteen-minute repeat, and a daily **routing-disabled control** with otherwise identical resolved settings and initial conditions. The control keeps generated runoff locally and suppresses all terminal return. It isolates the routing/return intervention in this model; it is not Earth observation or a reference hydraulic solver. All accepted baselines round-trip the complete checkpoint and continue another day except at the ten-year limit.

The ensemble retains all cases and failures: three seeds × subdivisions 2–4 × radii 1,000/6,371 km for one year, plus one ten-year reference. Rejections still produce JSON and a failing exit status. Gross edge departures count each traversal, not water production. Total terminal delivery and subsequent evaporation are independently reported in stock budgets. Monthly rain totals aggregate all reported years, not climatological monthly normals. No-routing controls use daily steps, not an additional refined repeat.

Tests cover independent one/two-reach analytic references, decreasing time-refinement errors, branched accumulation, relabeling, first-contact water-body arrivals, finite terminal retention, physical distance/speed, work bounds, invalid adjacency/stocks/versions, checkpoint replay with signed roundoff, and rejection of balanced invented teleports. Generated-year tests require actual terminal return to vapor and compare a matched disabled control. The older soil-ledger failure witness remains routing-disabled and uses the identical 900-second physical intervals batched into daily advances.

## Measured version-3 evidence

Measured October 3, 2026 on the supported Linux environment: all nineteen baselines, nineteen fifteen-minute repeats, and nineteen daily disabled controls completed without rejection. All baseline checkpoints round-tripped exactly; eighteen also checked an extra day of exact continuation. Every enabled baseline recorded positive terminal delivery and evaporation. [Complete inputs and compact budgets for all cases](data/seasonal-moisture-3-validation.json) retain the controls and results.

Maximum baseline relative total-water residual was 4.1e−15. The largest final regional stock/graph-ledger residual across baselines and refined repeats was 6.5e−13, within the unchanged 1e−12 bound; this is finite numerical headroom, not exact accounting or evidence for arbitrary longer runs.

| Family | Six-stock time-refinement difference / initial water | Enabled precipitation total | Difference from disabled control |
| --- | ---: | ---: | ---: |
| Nine one-year cases, radius 1,000 km | 0.276–0.428% | 404.9–576.5 mm | +0.382–2.58% |
| Nine one-year cases, radius 6,371 km | 0.051–0.091% | 157.8–245.5 mm | +0.244–1.24% |
| One ten-year reference case, radius 6,371 km | 0.167% | 1,112.2 mm over ten years | +7.05% |

These differences are matched interventions in this empirical model; they do not imply universal rainfall amplification or validate a terrestrial climate. Initial active water stays finite and unreplenished; annual totals and a cumulative ten-year total are not directly comparable normals.

In the ten-year baseline, transit held 0.074% and terminal stores 11.96% of initial mobile water at the endpoint; terminal evaporation cumulatively returned 14.15% of initial water to vapor. The latter is a flow integral, not another stock. Some water can cycle repeatedly, so cumulative returned water must not be added to the final inventory. Baseline vapor maxima across all cases were 46.0–54.0 kg/m², not reference atmospheric observations.

The daily disabled controls exactly matched historical version 2 across all nineteen cases for the original five stocks, initial total, cumulative evaporation/precipitation, and eight surface-transfer totals. No checkpoint migration or cross-platform reproducibility is implied by this same-environment numerical parity.

Full project tests (strict TypeScript, all native targets, and Node adapter checks), native formatting, Clippy with warnings denied, and diff checks passed. Additional targeted tests cover a spherical seam-crossing receiver and malformed physical adjacency. The original version-3 numerical increment was headless; subsequent [desktop validation and timing evidence](seasonal-water-desktop.md#validation-and-measured-limits) concern its unchanged equations through a new adapter/viewer.

## Remaining integration gates

This increment closes a bounded runoff-return path; it does not select a full planetary hydrology default. Coasts, bed, drainage, initial wet/dry temperature mask, and terrain remain fixed. Terminal pools have no spill thresholds, shared-body leveling, area curves, seepage, or shoreline updates. Transit has no channel evaporation/infiltration, width/depth, slope-dependent speed, sediment, flood waves, or momentum balance. Deep water and groundwater are excluded. Geography/thermal feedback, terrain-dependent rainfall, climate calibration, and coherent weather remain separate work. The subsequent [desktop checkpoint integration](seasonal-water-checkpoints.md) now preserves this complete bounded state without changing its equations.
