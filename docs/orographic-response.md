# Experimental upslope precipitation response

Implemented October 4, 2026, as an **opt-in, uncalibrated approximation**. `seasonal-moisture-4` adds `orographic-response-1` to the six-stock model. Default initialization remains version 3; its schema-3 shape and physical path are preserved. Milestone D remains incomplete.

## Motivation versus project choices

Terrain-following ascent has the kinematic proxy `w = U · grad(h)`. [Smith and Barstad (2004)](https://doi.org/10.1175/1520-0469(2004)061%3C1377:ALTOOP%3E2.0.CO;2) relate terrain forcing to precipitation with additional atmospheric/cloud processes. [Hergarten and Robl (2022)](https://gmd.copernicus.org/articles/15/2063/2022/) describe moisture transport with separate vapor/cloud components and finite conversion/fallout processes. These motivate directional forcing and finite moisture transport, **not our deposition equation or parameter values**. Neither published model is implemented here.

Existing temperature normals already decrease land temperature with elevation; saturation capacity already depends on height. Avoid counting that cooling twice. Instead accelerate removal of **existing supersaturation** on positive upslope motion, without changing temperature/capacity. A subsaturated column cannot rain under this rule.

## Geometry and equations

Air-facing height uses prepared native bed on initially dry regions and fixed initial reference water level on wet regions. Ocean-floor relief is not a mountain. Seasonal pools do not update this geometry. Globe exaggeration, visual interpolation, projection, and tessellation never enter physics.

Fit a dimensionless tangent gradient using spherical neighbor log-maps:

```text
d_ij = R × angular_distance(p_i, p_j)
t_ij = unit tangent direction from p_i toward p_j
minimize over tangent g_i: sum_j [(g_i · t_ij) − (h_j − h_i)/d_ij]^2
w_i = U_i · g_i                                  [m/s]
k_extra_i = strength × max(w_i, 0) / H_response   [1/s]
P_i = max(vapor_i − capacity_i, 0)
      × [1 − exp(−(1/tau_precip + k_extra_i) × dt)] [kg]
```

The fit is equivalent to inverse-distance-squared weighting of raw height residuals. No longitude/pole finite difference is used. Require finite unit centers, sorted reciprocal neighbors, nondegenerate distances, and a nonsingular fit. Invalid/overflowing inputs reject. Sample existing monthly tangent winds at region centers.

Default `H_response = 1000 m`, `strength = 1`; headless ranges 100–5000 m and 0–2. These are empirical response parameters, not calibrated cloud depth. Descending/still air has no extra rate. Strength zero retains the exact old deposition expression; disabling precipitation disables both terms. No subsaturation removal, second cooling, or rain-shadow mask is added.

Actual deposited kilograms debit vapor once and enter the existing rain/snow recipient. All six stocks, surface/routing laws, graph ownership, and budget tolerances remain unchanged. No seventh cloud stock or new source exists.

## Coupling, versions, and desktop

Coupling additionally respects one sixth of the fastest combined precipitation response across all twelve months. A positive extra rate rounds the bound down to an integer hour divisor, preserving default daily/hourly batching. Transport retains its stability bound. Subsecond requirements reject; more than 4096 coupled steps per version-4 caller interval rejects atomically and suggests a shorter interval. These are work safeguards, not performance guarantees. `Model::maximum_coupled_step_seconds()` exposes the actual bound; sensitivity must actually reduce physical steps.

Schema 4 pins `orographicModelVersion: "orographic-response-1"` and resolved `settings.orography`. Schema/model/settings/module-pin combinations must agree. Missing optional fields mean legacy; present `null` rejects. Restore regenerates gradients/monthly rates from the recipe and checks all stock/graph ledgers. No legacy checkpoint is silently upgraded.

Choose **Initialize upslope response** before initialization; **Initialize baseline water** retains version 3. No mid-run mode switch: regenerate or open another checkpoint. Footer/budget note identify active version and experimental scope. Desktop accepts pinned default settings for either mode; other valid headless settings reject explicitly.

`orographicMoisture` and `orographicMoistureCheckpoint` use protocol 12; old kinds retain protocol 11. Display body remains eighteen field-major Float64 arrays, not a checkpoint. Existing layers, inspectors, pause/run, file limits, candidate lifecycle, and original-JSON numeric transport support both modes. Schema 4 preserves full state and signed accumulator corrections. Derived uplift is available natively, not yet a desktop layer.

## Evidence and limits

The [measured record](data/orographic-response-validation.json) retains twelve directed comparisons and six generated cases, **including two refinement failures**:

```sh
cargo run --release --locked --manifest-path native/Cargo.toml --example orographic_response_report
```

The directed fixture transports a finite vapor pulse across a smooth 3000 m ridge on a closed 1000 km-radius sphere. Wind rotation speed is 20 m/s, duration 100000 s, initial vapor at most 32 kg/m². Capacity uses the existing column law with `20°C − 0.0065 × height`; no evaporation replenishes water. Compare flat terrain, altitude-dependent capacity alone, and the additional upslope response, at levels 3/4 and both wind directions, with 500/250 s steps.

Compared with the **same ridge/capacity without extra response**, coarse windward rain rises about 70–74%, lee rain falls 36–51%, and downstream remaining vapor falls 3–5%. Reversing wind mirrors the response. Flat terrain gives zero rain; maximum relative budget residual is below 1.6e−16. Combined vapor/rain L1 change after halving the step is 1.1–1.8% of initial donor. Spatial refinement changes results appreciably: first-order transport remains dispersive. This verifies directionality/conservation, not a converged precipitation forecast or generic realistic deserts.

Generated tests use three seeds × radii 1000/6371 km, level 2 (162 regions), forty days. All six default runs pass budgets, zero-strength stock equality with legacy physics, and exact checkpoint continuation. Actual default coupling is 1800 s; maximum regional residual is below 4.7e−14. A separate version-3 control uses the **same cadence**, isolating the response from time-step changes. Total precipitation increases only about 0.0013–0.245%: the ridge fixture does not imply strong effects everywhere.

Actual 60 s refinement completes four cases with combined six-stock L1 differences 5.9e−5–4.0e−4 of initial mobile water. Two reject soil-stock ledger index 2 slightly above the unchanged 1e−12 local tolerance:

| Seed / radius | Rejected second | Region | Relative local residual |
| --- | ---: | ---: | ---: |
| seasonal-reference / 1000 km | 2654880 | 99 | 1.0000439762e−12 |
| moisture-coast / 1000 km | 2979660 | 103 | 1.0000302584e−12 |

Their failure stages/settings remain recorded; no refined endpoint/error norm is invented. No tolerance widening or stock correction hides this open numerical-ledger gate. Forty-day default success does not establish unrestricted stability or spatial convergence.

Native tests cover analytic gradient convergence, rotation/datum/radius controls, flattened reference water, invalid inputs, exact excess relaxation, finite donors, legacy compatibility, process switches, batching, year-boundary replay, strict pins, and atomic excessive-work rejection. The directed example is an all-target test. Adapter tests cover protocol/mode isolation, corruption, transactional loading, and complete JSON continuation. A level-6 one-hour checkpoint is 14,685,965 bytes for the test recipe; a second hour exactly matches uninterrupted state. This is short persistence evidence, not long-run fine-grid qualification. Real Electron tests select/display/save/restore/continue schema 4 exactly; old version-3 tests also pass.

## Next gates

Investigate the retained 60 s soil-ledger witnesses without changing legacy physics or tolerances. Broaden temporal/spatial/parameter evidence before making this mode default. Cloud storage/advection, condensation/fallout delays, re-evaporation, atmospheric pressure/mass/energy, mountain waves, blocking, and thermodynamic foehn effects are absent. Wind remains prescribed, not terrain-deflected. Weather, lake levels/spill ownership, groundwater, ecology, and calibration remain future work.
