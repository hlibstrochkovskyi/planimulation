# Finite seasonal water exchange

`seasonal-moisture-2` couples the native [moisture-transport kernel](moisture-transport.md) to finite liquid, snow, soil, and pending-runoff stocks, temperature-dependent vapor capacity, and local precipitation. The [surface-water closure](surface-water.md) declares the melt and soil laws and their limits. This runs headlessly with a versioned same-build checkpoint on fixed initial geography, not the general basin-water solver or desktop climate playback. Milestone D remains incomplete.

## Ownership and initial state

The atmosphere initially contains zero water. Each region starts with liquid mass:

```text
L_i = area_i × min(initial_water_depth_i, active_depth) × 1000 kg/m³
snow_i = soil_i = pending_runoff_i = vapor_i = 0
```

Active depth defaults to 1 m and accepts 0–10 m. It selects a finite partition of generated water, not an unlimited ocean source. The remaining original deep water is excluded and never replenishes this layer. Active depth is not a maximum on later liquid storage. This run must not be summed with the full immutable `World.water` reference or combined with the separate manual basin inventory: those would double-count water. Basin withdrawals and delivery to that inventory are not implemented.

The initial stock is liquid even in initially cold reference-water regions. This is a declared non-equilibrated starting condition, not a freeze-up calculation. The original wet/dry mask selects land-only infiltration, soil evaporation, and runoff generation throughout the run. Cold precipitation accumulates as snow until positive prescribed temperatures permit melting. Rain and melt can infiltrate a finite soil bucket; liquid runoff and excess soil drainage enter a separately owned pending-runoff stock. That stock is retained but **not routed or returned**. It cannot evaporate locally or be counted again as liquid/soil.

The recipe-based checkpoint supports an unmodified `World::generate(recipe)` origin. Custom initial fields or forcing edits are not encoded and are outside this restore contract.

## Vapor capacity and atmospheric exchange

Equilibrium vapor pressure follows [Murphy and Koop (2005)](https://doi.org/10.1256/qj.04.94), with equations reproduced in [Baumgartner et al. (2022), Appendix A](https://acp.copernicus.org/articles/22/65/2022/acp-22-65-2022.html). Temperature T is Kelvin; pressure is Pa. Below 0°C use ice; otherwise use liquid water:

```text
ln(e_ice) = 9.550426 − 5723.265/T + 3.53068 ln(T) − 0.00728332 T
ln(e_water) = 54.842763 − 6763.22/T − 4.210 ln(T) + 0.000367 T
  + tanh(0.0415(T − 218.8))
    × (53.878 − 1331.22/T − 9.44523 ln(T) + 0.014025 T)
q_capacity = e_sat × H_effective / (461.5 × T)      [kg/m²]
C_i = area_i × q_capacity                         [kg]
```

The effective isothermal column is our approximation, not a vertically resolved atmosphere or a law calibrated by these papers. H defaults to 2,000 m and accepts 100–5,000 m. Temperatures and winds are prescribed monthly `seasonal-temperature-1` and `seasonal-wind-1` fields. Monthly temperatures outside −100..50°C reject rather than clamp. No latent-heat feedback, energy-limited evaporation, pressure/air-mass field, cloud storage, or topographic lifting is implemented.

At fixed capacity, with vapor V and local interval Δt:

```text
E_potential = max(C − V, 0) × (1 − exp(−Δt/τ_evap))
P = max(V − C, 0) × (1 − exp(−Δt/τ_precip))
E_actual = liquid_evaporation + soil_evaporation
V_new = V − P + E_actual
```

The surface closure receives potential demand and precipitation; only actual evaporation credits vapor. At or below 0°C evaporation stops and precipitation enters snow. The five-day evaporation and six-hour precipitation response times are provisional choices, with supported ranges one hour–thirty days and one hour–ten days respectively. Switches disable either atmospheric exchange without resetting stocks; surface melt/infiltration/drainage can still occur. Supersaturation relaxes rather than being forcibly clamped. The exported scalar `exchange` function retains its original two-stock semantics for analytic controls; the coupled model now uses typed surface exchange instead.

## Integration, ledgers, and checkpoint

Caller advances accept 1–86,400 integer seconds. A coupled interval applies half a local surface/atmosphere exchange, full conservative vapor transport, then half an exchange. The default coupled maximum is 3,600 seconds (supported 60–21,600), additionally bounded by one sixth of enabled atmospheric response times and all surface response times. Intervals end at clock-aligned coupled boundaries or day boundaries. Transport retains outgoing-rate substeps. Months follow the existing 365-day calendar. Hourly integration is not hourly weather: forcing is piecewise-constant monthly normals.

The local surface operator is sequential, not an exact or globally second-order coupled solution. Refinement compares all five owned stocks. Capacity arrays and atmospheric response fractions are prepared outside regional update loops.

The state stores five stock arrays, cumulative E/P, and eight surface transfer ledgers per region. Transfer totals use Kahan summation with signed roundoff saved per component; cumulative E/P are derived from those component totals. Roundoff is accounting state, not physical mass or a final balance correction. With all transfers below cumulative:

```text
Σ(liquid + snow + soil + pending_runoff + vapor) = initial_mobile_water
liquid_i = initial_liquid_i + rain_i + melt_i − liquid_evaporation_i − infiltration_i − liquid_runoff_i
snow_i = snowfall_i − melt_i
soil_i = infiltration_i − soil_evaporation_i − soil_drainage_i
pending_runoff_i = liquid_runoff_i + soil_drainage_i
E_i = liquid_evaporation_i + soil_evaporation_i
P_i = rain_i + snowfall_i
Σ(vapor) = Σ(E − P)
```

Global residual tolerance is 1e−12 × max(initial mobile water, 1 kg). Each regional typed-ledger residual uses 1e−12 of the largest initial/current condensed stock, cumulative local exchange, or individual surface transfer, with a 1 kg floor. The vapor ledger has its own 1e−12 tolerance based on exchanged water and vapor. A surface operation checks its stock-change residual against 32 machine epsilons of its stock/actual-transfer scale. These are floating-point tolerances, not exact accounting or retention of arbitrarily sub-ULP transfers. No final correction or redistribution repairs a failed budget.

Each full update is atomic, including overflow and transport work-limit refusal. Checkpoint schema 2 pins moisture, surface, transport, temperature, and wind versions, recipe, resolved settings, clock, five stocks, and all ledgers including summation roundoff. Restore regenerates initial geography and rejects malformed arrays, negative/nonfinite values, soil above capacity, land-only stocks/transfers on reference-water regions, incompatible versions/settings, clocks beyond 3,650 days, or inconsistent ledgers. Roundoff must be finite and bounded by four machine epsilons of its component total (1 kg floor). Initial ledgers and roundoff must be zero; unknown JSON fields reject. Schema-1/version-1 checkpoints are not migrated or silently reinterpreted. A balanced edited checkpoint is not proof of historical reachability.

Same-build JSON round-trip and exact continuation are tested across month/year boundaries and non-day-aligned calls. This is a complete checkpoint for this bounded five-store model, not a general world checkpoint or cross-platform bitwise reproducibility guarantee. The legacy `surfaceKilograms` field now means liquid only; snow, soil, and pending runoff are separate required fields.

## Reproduce and validate

```sh
cargo test --locked --manifest-path native/Cargo.toml --test surface_water --test seasonal_moisture --example seasonal_moisture_report
cargo run --release --locked --manifest-path native/Cargo.toml --example seasonal_moisture_report -- docs/scenarios/seasonal-temperature.json
cargo run --release --locked --manifest-path native/Cargo.toml --example seasonal_moisture_report -- docs/scenarios/seasonal-temperature.json 3650
cargo run --release --locked --manifest-path native/Cargo.toml --example seasonal_moisture_report -- --ensemble
```

An ensemble with rejected cases still writes its complete JSON report, then exits unsuccessfully. Report version 2 includes the complete checkpoint, five-stock budgets and cumulative surface transfers, precipitation on initially dry regions, vapor extrema, and a fifteen-minute repeat. All five stocks enter its combined L1 difference, normalized by initial mobile water. A kilogram per square meter is a millimeter of water equivalent at 1,000 kg/m³. Monthly precipitation totals aggregate all reported years, not monthly climatological normals.

Directed controls cover cold accumulation, finite degree-day melting, saturated/unsaturated infiltration, soil retention and drainage, moisture-limited evaporation, area scaling, and a synthetic cold-to-warm year. Generated-year controls require snow, melt, soil evaporation, infiltration, and pending runoff to occur while all stock identities hold. Tests also cover thermodynamic references, disabled exchange, smaller-step convergence, a retained refined-year soil-ledger failure witness, compensated-sum controls, malformed/phase-swapped checkpoints, replay, and atomic rollback. The ensemble retains all failures: three seeds × subdivisions 2–4 × radii 1,000/6,371 km for one year, plus one ten-year reference case, each repeated at fifteen-minute caller steps.

## Measured version-2 evidence

Measured October 3, 2026 on the supported Linux environment: all nineteen baselines and their fifteen-minute repeats completed without rejection. All checkpoints round-tripped exactly; eighteen cases checked an extra day of exact continuation, while the ten-year case ended at the declared bound. All cases produced snowfall, melt, infiltration, soil evaporation, liquid runoff, and soil drainage. [Retained inputs, budgets, and earlier failure witnesses](data/seasonal-moisture-2-validation.json) record every case, not just successful selected seeds.

The initial ordinary-summation candidate rejected five cases on the regional soil identity (ledger 2). The first retained witness was the 1,000 km reference world at subdivision 3, second 25,287,300, region 10. Kahan accumulation of the transfer ledgers, with saved roundoff and derived E/P totals, eliminated these sampled refusals without changing stocks, physical rates, or the 1e−12 ledger limit. The witness is a refined-year regression test. A separate 2^53-plus-unit-increments control checks the summation independently and resumes with nonzero roundoff.

| Version-2 baseline family | Five-stock refinement difference / initial mobile water | Precipitation total | Pending runoff / initial mobile water |
| --- | ---: | ---: | ---: |
| Nine one-year cases, radius 1,000 km | 0.296–0.460% | 400.9–572.6 mm | 10.6–22.4% |
| Nine one-year cases, radius 6,371 km | 0.057–0.102% | 157.4–244.4 mm | 2.02–5.03% |
| One ten-year reference case, radius 6,371 km | 0.170% | 1,038.9 mm over ten years | 22.6% |

Maximum baseline relative total-stock residual was 3.5e−15. The largest final regional typed-ledger residual across baselines **and repeats** was 8.7e−13, close to its declared 1e−12 bound: this is limited numerical headroom, not an exact-accounting guarantee or justification for longer runs. Stock arithmetic still uses binary64. Vapor maxima across baselines were 45.7–53.9 kg/m², outcomes of the uncalibrated effective-column law rather than reference atmospheric observations.

The growing pending-runoff fraction demonstrates the remaining missing return path. Annual precipitation and ten-year totals are not directly comparable climate normals in this finite non-replenished system. Passing these cases does not establish general planetary water ownership, calibrated rainfall, spatial convergence, or untested extreme-parameter behavior.

Preparing interval response fractions outside the regional loop matched the retained 1,000 km subdivision-3 reference case's baseline summary, refined summary, and stock-difference metric exactly. Full project tests, strict TypeScript checks, Rust formatting, and all-target Clippy with warnings denied passed. No desktop playback check is claimed: this increment does not change the desktop.

## Historical version-1 evidence

The previous two-store model's daily-coupling candidate conserved mass but differed from its six-hour repeat by up to 7.4%, motivating the hourly default. Its final nineteen baseline/repeat cases passed on October 3, 2026; maximum baseline relative total residual was 1.2e−15, maximum local surface-ledger residual 4.2e−14. These values belong to version 1 and do not validate version 2.

| Version-1 family | Refined combined-stock difference / initial mobile water | Precipitation total |
| --- | ---: | ---: |
| Nine one-year cases, radius 1,000 km | 0.284–0.452% | 411.7–578.9 mm |
| Nine one-year cases, radius 6,371 km | 0.045–0.094% | 159.9–247.6 mm |
| One ten-year case, radius 6,371 km | 0.148% | 1,115.7 mm over ten years |

## Next integration gates

Coastline, bed, drainage, and temperature's initial wet/dry mask do not evolve. Local liquid is not an equilibrated lake level. Pending runoff is an accumulating endpoint, not river flow, discharge, or groundwater. Delivering gross precipitation downstream while keeping it locally remains forbidden; routing must debit its actual owner and credit an explicit receiving stock, reconcile units, and define returns to the mobile water cycle.

Next connect formed runoff to drainage with explicit recipient ownership, then evaluate groundwater, terrain lifting/rain shadows, evolving shorelines, and desktop dynamic fields/budgets. Finite active-layer depletion and absent runoff return/deep-water exchange must remain visible. No climate calibration, generated-geography spatial convergence, or support for untested extremes is claimed.
