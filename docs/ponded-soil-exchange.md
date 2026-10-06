# Ponded liquid and soil exchange

October 6, 2026: a retained model-15 discontinuity and an implemented **standalone native operator**, `ponded-soil-exchange-1`. No seasonal schema, desktop mode, default, or existing checkpoint uses this operator yet. This work addresses vertical ownership and the thin-film limit, not reference-body dynamics or calibrated soil physics.

## Retained defect in the current seasonal coupling

Model 15 freezes a half-stage exposure mask from positive pooled regional liquid. An exposed terrestrial column temporarily hides its soil and passes `is_land = false` to the local surface operator. Its soil is subsequently restored unchanged, and the remaining local liquid is captured into the regional pool. Consequently, arbitrarily little pooled water can suppress all infiltration, soil evaporation and drainage of a whole coarse column. Accounting can pass while this physical switch is wrong.

`positive_film_freezes_the_existing_soil_column_in_model_15` retains a generated-world regression with equal total water in every variant:

- Recipe: `docs/scenarios/spill-connections.json`, seed `first-light`, subdivision 2; initial coverage changed to 0.3.
- Region 1 has area `2475931774103.23 m²`. A constant prescribed 30 °C removes temperature gating; precipitation, evaporation, routing and horizontal mobility are disabled to isolate vertical behavior.
- Synthetic rain equivalent to 130 kg/m² is fully funded from a finite reference body with matching contact histories. Soil owns 120 kg/m²; the remainder is local liquid. This is a constructed history, not spontaneous climate.
- Variants move 1, `1e-6`, or `1e-12 kg` from that local liquid into its regional pool, retaining normalized low components and recording the capture. No extra water is added.
- In 900 s the zero-film variant infiltrates `0.4070564585868564 kg/m²` and drains `0.010521307320561097 kg/m²`. Every positive-film variant infiltrates and drains exactly zero, while native checkpoint validation and local ledgers pass.

The test deliberately preserves model 15's current result. Removing its assertion without introducing a separately versioned coupling would silently change reproducibility. No arbitrary wet-depth cutoff is proposed as a repair: that would move the discontinuity rather than remove it.

The same test independently runs the new operator on this extracted terrestrial column, recombining both liquid partitions and their signed tails into one owner. All four provenance variants then produce exactly the same component step, with nonzero infiltration and drainage. This is a component comparison on actual generated area, not application of a new seasonal model.

## Basis and declared approximation

[USACE HEC-HMS soil moisture accounting](https://www.hec.usace.army.mil/confluence/hmsdocs/hmstrm/canopy-surface-infiltration-and-runoff-volume/infiltration/soil-moisture-accounting-loss-model) includes existing surface water in infiltration supply and limits potential infiltration by soil saturation and available water. It also separates soil storage and deeper receivers. These concepts motivate the ownership and saturation response here; the implementation is not HEC-HMS, Richards' equation or a measured soil model.

For this candidate, soil capacity is `C = area * capacity_per_area`. Under an unlimited surface supply, the selected empirical infiltration law is `dB/dt = (C - B)/tau_i`. Its exact isolated interval potential is `(C - B) * (1 - exp(-dt/tau_i))`. Actual infiltration is capped by the finite liquid donor and represented soil room. This is **different** from the legacy law proportional to available liquid: the default potential at empty soil corresponds to 25 kg/m²/hour, but neither that rate nor its response to saturation is calibrated.

Uniform defaults and supported ranges reuse the old closure: capacity 150 kg/m² (1–2000), retained fraction 0.6 (0–1), infiltration response 6 hours (1 hour–30 days), drainage response 30 days (1–365 days). They are resolved project choices, not geology-derived constants.

## Implemented ownership and interval order

The value-owned input has four normalized high/low kilogram stocks: surface liquid, soil, vapor, and drainage awaiting an explicit downstream policy. Surface liquid is one source-agnostic reservoir: rain, melt and lateral arrivals must not acquire different vertical behavior merely because their provenance differs. Soil remains owned and active on initially terrestrial columns regardless of positive surface depth.

For a positive interval at air temperature above 0 °C:

1. Evaporate from liquid into the finite vapor recipient, bounded by one caller-supplied interval demand. Satisfy only the remaining demand from soil, scaled by its leading-component saturation fraction `B_high/C` and bounded by the finite soil donor. There is no second independent evaporation allowance for a tiny film.
2. Apply the capacity-response infiltration potential, debiting liquid and crediting soil with exactly the same scalar grant.
3. Drain `(B - retained_fraction*C)_+ * (1 - exp(-dt/tau_d))` into the separately owned drainage recipient. This recipient is neither groundwater nor an implicit sink, and the operator does not route it.

Every stock identity is checked independently. Each transfer also checks the actual represented debit and receipt within `32 * epsilon * grant`, **not** a large planetary stock scale. Unrepresentable transfers reject the entire candidate; they are not repaired, silently discarded or placed in an unowned escrow. Input is passed by value, so rejection cannot mutate caller state. Small updates that do fit the low components remain with their actual owner.

Intervals are bounded to 0–86400 s, forcing must be finite/nonnegative, and stock pairs/capacities must validate. A zero interval is identity even if nonzero demand is supplied. The inherited `T <= 0` gate disables all four processes; this remains a crude air-temperature approximation, not frozen-soil thermodynamics. Evaporation demand is prescribed mass for this interval, not a flux or an energy budget.

There is no binary surface-wet switch. In the limit of vanishing liquid, infiltration vanishes with its finite supply, while existing soil drainage and soil evaporation remain active. Genuine saturation and donor exhaustion still have continuous piecewise bounds. Sequential infiltration then drainage is a first-order split: drainage-created room is available on the next step, not retrospectively within the current infiltration solve.

## Directed validation

Ten operator tests cover isolated analytic infiltration, finite surface supply and the thin-film limit, shared evaporation demand, saturation/retention, cold/zero controls, area scaling, signed low components and almost-full soil, exact same-build serialized continuation, finite recipient capacity, malformed settings/state, atomic numerical refusal, and coupled refinement. An independent integer oracle checks **100,000** one-kilogram evaporation transfers between `2^70 kg` liquid/vapor stocks; both owner totals and every local residual are exact, including signed low tails. This is an arithmetic stress control, not a planetary climate scenario.

The coupled reference independently solves `dB/dt = (C-B)/tau_i - (B-R)/tau_d` where supply stays sufficient and soil remains above retention. Starting at 120 kg/m² with 1000 kg/m² surface liquid, an interval of 7200 s gives:

| Step [s] | Soil error [kg/m²] | Drainage error [kg/m²] |
| ---: | ---: | ---: |
| 900 | 0.0029692324880556953 | 0.0014639375176671438 |
| 450 | 0.0014795408411316657 | 0.0007346006202482158 |
| 225 | 0.0007385013529699336 | 0.0003679582189955316 |

Both errors approximately halve. This checks the selected sequential closure on one controlled column, not universal convergence or agreement with real soil. The retained generated witness separately verifies the legacy discontinuity; it does not claim that the new operator is already coupled to the seasonal world.

Reproduce the directed evidence:

```sh
cargo test --locked --manifest-path native/Cargo.toml --test ponded_soil -- --nocapture
cargo test --locked --manifest-path native/Cargo.toml --test regional_surface_flow positive_film -- --nocapture
```

`State` JSON is the component's owned input, **not** a complete seasonal checkpoint. Its caller must pin `MODEL_VERSION` and resolved settings; it omits snow, forcing, clock, graph and cumulative history. Existing seasonal replay is unchanged.

Validation on October 6: full `npm test` passes native all-target regressions, TypeScript checking and all 77 Node tests, including existing regional desktop/checkpoint replay. The ten operator tests and all five regional integration tests also pass in release. Warnings-denied all-target Clippy, formatting and diff checks pass. The pre-existing ignored long annual-snow qualification remains unchanged; no new tests are ignored. No GUI interaction test was rerun for this native-only increment, and no new desktop functionality is claimed.

## Next integration gate

The next implementation is a new explicit seasonal coupling, not another basin-frontier experiment:

1. Define one available terrestrial liquid owner for precipitation, melt and face arrivals. Avoid charging both the old liquid-runoff closure and regional face flow for the same mobile water.
2. Add independent cumulative regional infiltration and soil exchange identities, including the opposite atmospheric and drainage receipts. Resolve how component precision maps to currently scalar vapor/transit stocks; never drop a low component at the adapter boundary or credit vapor twice.
3. Specify the drainage receiver policy explicitly, preserve legacy schema-15 behavior, and require new version pins and complete replay before exposing the new coupling in the desktop.
4. Test generated seasonal runs, wetting/drying and cadence sensitivity with this operator actually applied. Column-level success alone does not pass this integration gate.

Initially wet reference regions remain a separate problem; their prescribed heads/coasts do not become dynamic through this change. Fine wet-area fractions, soil type/conductivity, groundwater backpressure, thermal ice, hydraulic calibration and paired spatial-resolution qualification remain open. Milestones C/D and ecology readiness are not complete.
