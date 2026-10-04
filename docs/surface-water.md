# Typed local surface-water closure

`surface-water-1` is the unchanged pure native transfer operator used by seasonal moisture versions 2/3. It separates liquid, snow water equivalent, soil water, and pending runoff. It does not generate additional water, route rivers, solve groundwater, or change geometry. Version 3's caller separately [routes pending runoff and evaporates terminal stores](runoff-transport.md). All mass is kilograms; area is m², temperature is °C, and time is seconds. Atmosphere and downstream ownership remain the caller's responsibility.

The [headless soil-precision candidate](soil-precision.md) separately pins `surface-water-compensated-soil-1` in seasonal version 5. It retains the same process laws but persists a low soil component and bounds transfers using represented mass/capacity. Other surface stocks remain ordinary `f64`; annual snow-ledger qualification fails. The legacy operator and defaults described below are not silently replaced.

## Scientific basis versus implementation choices

Temperature-index snowmelt is an established empirical alternative to a full energy budget; [USACE's degree-day description, equation 3-5](https://www.hec.usace.army.mil/publications/IHDVolumes/IHD-4.pdf) makes the coefficient and threshold explicit. Our constant coefficient and monthly forcing are deliberately much simpler than [HEC-HMS's temperature-index model](https://www.hec.usace.army.mil/confluence/hmsdocs/hmstrm/snow-accumulation-and-melt/temperature-index-method). Neither source validates the selected coefficient for generated planets.

[HEC-HMS soil moisture accounting](https://www.hec.usace.army.mil/confluence/cwmsdocs/hmsum/4.11/selecting-a-loss-method-118101217.html) distinguishes surface storage, finite soil storage, infiltration, water extraction, and deeper groundwater. That motivates explicit ownership, but our one-bucket response laws below are project choices, **not an implementation of HEC-HMS, HBV, Richards' equation, or measured soil physics**. No calibration has been performed.

## Operator and order

Let L be liquid, N snow, B soil, Q pending runoff, A area, and C = A × soil capacity. Each local interval executes:

1. Deposit precipitation P: if T ≤ 0, add to N; otherwise add to L.
2. If T > 0, melt M = min(N, A × degree_day_factor × T × Δt/86400); debit N and credit L.
3. If T > 0, evaporate E_L = min(L, potential demand); then on land E_B = min(B, (remaining demand) × B/C). Debit the actual donors. The caller credits only E_L + E_B to vapor.
4. On warm land, infiltrate I = min(L × (1 − exp(−Δt/τ_infiltration)), C − B), then drain D = max(B − retained_fraction × C, 0) × (1 − exp(−Δt/τ_drainage)). Debit L for I, credit B, then debit B for D.
5. On land, form liquid runoff R = L × (1 − exp(−Δt/τ_runoff)). Debit L and credit Q with R + D.

Each expression uses the updated stock from preceding operations. This is a declared sequential split and needs time-refinement checks, not an exact continuous coupled solution. Rain/melt arriving in an interval can participate in that interval's subsequent processes. Isolated exponential response controls have analytic references.

No infiltration or soil drainage occurs at T ≤ 0. This air-temperature gate is only a crude frozen-ground approximation: existing liquid and soil are not frozen into a thermodynamic ice reservoir. Existing cold liquid can still form runoff. No snow sublimation, cold content, refreezing, albedo, latent heat, snow density, vegetation/canopy interception, or snow-covered evaporation reduction is modeled. Snow deposition over reference-water regions also uses the water-equivalent store; it is not a sea-ice or floating-snow solver.

## Defaults and supported bounds

All values are serialized resolved settings; these defaults are uncalibrated.

| Setting | Default | Supported range |
| --- | ---: | ---: |
| Degree-day melt factor | 3 kg/m²/°C/day | 0–20 |
| Soil capacity | 150 kg/m² (150 mm) | 1–2,000 |
| Retained fraction | 0.6 | 0–1 |
| Infiltration response | 6 hours | 1 hour–30 days |
| Liquid runoff response | 1 day | 1 hour–30 days |
| Soil drainage response | 30 days | 1–365 days |

Soil capacity and retained fraction are uniform placeholders, not geology-derived properties. Infiltration has a finite receiving capacity but no explicit hydraulic-conductivity limit. Drainage goes directly into pending runoff, not groundwater; groundwater timing is deferred. There is no travel time or slope dependence in runoff generation.

## Ownership and verification

Pending runoff is a real separately owned mass stock, removed from its donors and unavailable to local evaporation. Version 2 retains it locally; version 3's caller debits/credits it through receiver-edge routing and separately owns terminal arrivals. It must not be another copy of rain, soil drainage, or liquid. Reference-water regions do not acquire soil or transit runoff; they are terminal receiving locations in version 3. The immutable initial wet/dry mask still determines local surface processes.

The operator returns all eight transfers (rain, snowfall, melt, liquid/soil evaporation, infiltration, liquid runoff, soil drainage) and checks:

```text
after_total − before_total − precipitation + actual_evaporation = 0
```

Tolerance is 32 machine epsilons times max(before total, precipitation, actual evaporation, 1 kg). Invalid forcing, malformed ownership, nonfinite arithmetic, soil above capacity, or a failed residual rejects. There are no final balance repairs. Caller-owned input is passed by value, so rejection cannot mutate it. The coupled checkpoint separately checks every stock's cumulative identity and retains these transfers through save/restore. Long-lived totals use Kahan summation with checkpointed signed roundoff; this roundoff is accounting state, not an additional physical stock. Cumulative evaporation/precipitation are derived from component totals. Interval response fractions are prepared once outside the coupled regional loop.

Tests include threshold deposition, finite melt, disabled melt, analytic infiltration/drainage, full soil, dry-soil evaporation, no land stores over reference water, an area-scaling control, a synthetic year with snow release, overflow, invalid inputs, and a 2^53-plus-unit-increments summation control with nonzero-roundoff JSON continuation. These establish the declared numerical behavior, not physical realism.
