# Finite seasonal surface/vapor exchange

`seasonal-moisture-1` couples the native [moisture-transport kernel](moisture-transport.md) to finite surface-water columns, temperature-dependent vapor capacity, and local precipitation. It runs headlessly with a versioned same-build checkpoint. It is an independent mobile-water calculation on fixed initial geography, not the general basin-water solver or desktop climate playback. Milestone D remains incomplete.

## Ownership and initial state

The initial atmosphere contains **zero** water. Each region starts with a mobile surface stock:

```text
S_i = area_i × min(initial_water_depth_i, active_depth) × 1000 kg/m³
V_i = 0
```

The default active depth is 1 m; supported settings are 0–10 m. This selects a finite partition of the generated water, rather than manufacturing atmospheric moisture or treating oceans as an unlimited source. The initial active depth is not a maximum on later local storage: deposition can raise a recipient's surface stock above it. Precipitation on initially dry land creates a local surface stock that can subsequently evaporate.

The remaining original deep water is outside this model and never replenishes the active layer. Its omission makes local depletion possible; it is not a claim about real ocean mixing or a physically chosen mixed-layer depth. The original `World.water` remains an immutable generation reference. This run must not be summed with that full initial volume or combined with the separate manual basin inventory: those would double-count water. Neither basin withdrawal nor precipitation delivery to that inventory is implemented here.

Surface stocks are condensed **water-equivalent** storage. The model does not distinguish rain, snow, ice, or soil water. Below 0°C evaporation is disabled. Deposited cold-region water is not a snowpack with its own melt delay; it becomes available to the empirical evaporation law when prescribed temperature becomes positive. Snow, melt energy, soil retention, and infiltration require subsequent model increments.

## Capacity and exchange law

The equilibrium vapor-pressure formulas follow [Murphy and Koop (2005)](https://doi.org/10.1256/qj.04.94), with explicit equations also reproduced in [Baumgartner et al. (2022), Appendix A](https://acp.copernicus.org/articles/22/65/2022/acp-22-65-2022.html). Below 0°C we use their ice expression; at and above 0°C the liquid-water expression is used. Temperature `T` is Kelvin and pressure is Pa:

```text
ln(e_ice) = 9.550426 − 5723.265/T + 3.53068 ln(T) − 0.00728332 T

ln(e_water) = 54.842763 − 6763.22/T − 4.210 ln(T) + 0.000367 T
  + tanh(0.0415(T − 218.8))
    × (53.878 − 1331.22/T − 9.44523 ln(T) + 0.014025 T)

q_capacity = e_sat × H_effective / (461.5 × T)      [kg/m²]
C_i = area_i × q_capacity                         [kg]
```

This effective isothermal column is **our approximation**, not a vertical atmospheric profile or a capacity law fitted by either paper. `H_effective` defaults to 2,000 m and accepts 100–5,000 m. Monthly temperatures come from the existing uncalibrated `seasonal-temperature-1`; wind comes from `seasonal-wind-1`. The moisture model supports −100..50°C and rejects a world with a monthly normal outside that range instead of clamping it. There is no latent-heat feedback, energy-limited evaporation, pressure/air-mass field, cloud storage, or topographic lifting.

At fixed temperature and region area, with surface stock `S`, vapor stock `V`, and interval `Δt`:

```text
E = min(S, max(C − V, 0) × (1 − exp(−Δt/τ_evap)))   if temperature > 0°C
P = max(V − C, 0) × (1 − exp(−Δt/τ_precip))
S_new = S − E + P
V_new = V − P + E
```

The five-day evaporation and six-hour precipitation response times are provisional model choices. Supported ranges are one hour to thirty days and one hour to ten days respectively. Process switches can disable evaporation or precipitation without resetting stocks. Capping evaporation by its finite donor is a physical availability constraint; it does not create replacement water. Precipitation is relaxation of excess vapor, not a hard clamp to capacity, so temporary supersaturation remains possible.

## Time integration, accounting, and checkpoint

A caller step accepts 1–86,400 integer seconds and is divided into coupled intervals. Each interval applies half an exchange interval, a full conservative transport interval, then the other half exchange. The coupled limit defaults to 3,600 seconds and accepts 60–21,600 seconds; enabled process response times also limit it to at most one sixth of the shortest response time. Intervals end at the next clock-aligned coupled boundary or day boundary. Transport retains its own outgoing-rate substeps inside each interval. Numbered months follow the existing 365-day calendar. Temperature and wind are piecewise-constant monthly means. An hourly integration interval is not hourly weather: forcing remains prescribed and has no coherent weather fluctuations.

The first measured daily-coupling candidate preserved mass but differed from its six-hour repeat by up to 7.4% of the initial mobile stock in the nineteen-case sample. That result led to the explicit hourly default, rather than accepting conservation alone as evidence of integration accuracy. Capacity arrays and exchange-response fractions are prepared outside regional update loops; repeated substeps do not reevaluate vapor-pressure logarithms for every region.

The state records the clock, both stock arrays, and cumulative evaporation and precipitation **per region**. The initial partition and fixed forcing are regenerated from the pinned recipe and settings. Validation checks:

```text
Σ(surface + vapor) = initial_mobile_water
surface_i = initial_surface_i − cumulative_E_i + cumulative_P_i
Σ(vapor) = Σ(cumulative_E − cumulative_P)
```

Global mobile-water error is bounded by `1e−12 × max(initial_mobile_water, 1 kg)`. Each regional surface-ledger residual is bounded by `1e−12` of the largest of its initial/current stock or cumulative local exchanges, with a 1 kg floor. The vapor ledger has an independent `1e−12` bound based on vapor and cumulative exchanged water, so a large surface stock cannot alone mask a lost atmospheric exchange. One exchange operation also checks its actual stock-change residual against sixteen machine epsilons of the local stock scale and reports that residual. These are explicit floating-point tolerances; this model does not claim exact integer accounting or retention of arbitrarily sub-ULP transfers. No final correction, redistribution, or stock clamping repairs a failed balance.

Each full update is atomic: an invalid input, transport work-limit refusal, overflow, or failed final ledger leaves stocks, clock, and cumulative exchange unchanged. The runtime state is private and exposes copies through checkpoint export. Checkpoint schema 1 includes the moisture, transport, temperature, and wind versions, full recipe, all resolved settings, clock, both stock arrays, and both local cumulative ledgers. Restoring regenerates the fixed initial world and rejects unknown versions, malformed arrays, negative/nonfinite values, invalid settings, out-of-range clocks, or inconsistent local/global ledgers. Unknown JSON fields are rejected. A balanced edited checkpoint is not proof of historical reachability.

The current supported run is bounded to 3,650 days. Same-build JSON round-trip and exact continuation are tested across month/year boundaries and non-day-aligned calls. This is a complete checkpoint for **this bounded two-store model**, not a general world checkpoint, a migration facility, or cross-platform bitwise reproducibility.

## Reproduce and validate

```sh
cargo test --locked --manifest-path native/Cargo.toml --test seasonal_moisture --example seasonal_moisture_report
cargo run --release --locked --manifest-path native/Cargo.toml --example seasonal_moisture_report -- docs/scenarios/seasonal-temperature.json
cargo run --release --locked --manifest-path native/Cargo.toml --example seasonal_moisture_report -- docs/scenarios/seasonal-temperature.json 3650
cargo run --release --locked --manifest-path native/Cargo.toml --example seasonal_moisture_report -- --ensemble
```

The report contains its final complete checkpoint, stock/exchange budgets, water-equivalent precipitation and evaporation totals, deposition on initially dry land, observed vapor extrema, local numerical residuals, and a repeat with fifteen-minute caller steps. The baseline daily caller advances are internally divided into at most hourly coupled intervals. A kilogram per square meter corresponds to a millimeter at the model's 1,000 kg/m³ water-equivalent density. Numbered-month totals aggregate all years in the report and are not monthly climatological normals. Caller intervals, coupled substeps, and transport substeps are labeled separately.

Directed tests cover reference vapor pressures, monotonic capacity, the analytic warm-box solution, source exhaustion, empty reservoirs, supersaturation and cooling, disabling either process, finite initial import, deposition on initially dry regions, both stock/flow identities, decreasing errors under smaller coupled steps, damaged/mismatched checkpoints, complete replay, and rollback when transport rejects after a successful provisional evaporation phase. The ensemble runs three seeds at subdivisions 2–4 and two radii, plus one ten-year reference case, retaining rejected cases and comparing every accepted case with a fifteen-minute repeat.

Measured October 3, 2026 on the supported Linux environment: all nineteen baseline cases and their fifteen-minute repeats completed with no rejected case. All checkpoint round trips were exact; eighteen cases also tested an extra day of exact continuation, while the ten-year endpoint correctly remained at its declared clock bound. The largest observed **baseline** relative mobile-water budget residual was `1.2e−15`, and the largest final local surface-ledger residual was `4.2e−14`.

| Baseline family | Coupled time-refinement difference in combined stocks / initial mobile water | Global precipitation total |
| --- | ---: | ---: |
| Nine one-year cases, radius 1,000 km | 0.284–0.452% | 411.7–578.9 mm water equivalent |
| Nine one-year cases, radius 6,371 km | 0.045–0.094% | 159.9–247.6 mm water equivalent |
| One ten-year reference case, radius 6,371 km | 0.148% | 1,115.7 mm water equivalent over all ten years |

Observed baseline vapor-column maxima were 45.8–54.1 kg/m² across these cases. These are measured outcomes of the chosen effective-column model, not reference atmospheric values. Annual and ten-year precipitation totals are not directly comparable climate normals: the finite active layer redistributes water without runoff return or deep-water replenishment. Passing this sample does not establish calibrated rainfall, spatial convergence on generated terrain, or support for untested extremes.

## Next integration gates

The generated world's coastline, bed, drainage, and temperature's original wet/dry mask do not evolve with these surface stocks. Surface storage is local and can accumulate without runoff; it is neither a dynamically equilibrated lake level nor soil moisture. Do not send gross precipitation to the existing basin inventory while retaining it here: downstream integration must transfer ownership and reconcile evaporation, returning water, units, and changing geography.

This model establishes a bounded surface-to-vapor-to-surface chain. It remains uncalibrated. Before selecting a full seasonal-world default, compare declared climate targets and reference scenarios, evaluate parameter and active-depth sensitivity, introduce soil/snow and runoff ownership, add terrain-lifting/rain-shadow controls, and expose dynamic fields and their budgets in the desktop. Finite active-layer depletion and the absence of deep-water exchange must stay visible in longer runs.
