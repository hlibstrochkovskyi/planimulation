//! Floating expansions audit legacy soil, compensated soil, and retained snow drift.
use planimulation_core::{
    Recipe, World,
    seasonal_moisture::{Model, Settings, SoilNumerics},
    seasonal_temperature, seasonal_wind,
};
use serde_json::json;

// Error-free TwoSum for finite inputs without overflow. Grow an expansion so
// this audit does not merely repeat the production ledger's Kahan arithmetic.
fn add(parts: &mut Vec<f64>, value: f64) {
    let mut q = value;
    let mut next = Vec::with_capacity(parts.len() + 1);
    for &p in parts.iter() {
        let sum = q + p;
        let b = sum - q;
        let error = (q - (sum - b)) + (p - b);
        if error != 0. {
            next.push(error);
        }
        q = sum;
    }
    if q != 0. {
        next.push(q);
    }
    *parts = next;
}
fn sum(values: impl IntoIterator<Item = f64>) -> f64 {
    let mut parts = Vec::new();
    for v in values {
        add(&mut parts, v);
    }
    parts.into_iter().sum()
}
fn main() -> Result<(), String> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    let (compensated, snow) = match args.as_slice() {
        [] => (false, false),
        [arg] if arg == "--compensated" => (true, false),
        [arg, stock] if arg == "--compensated" && stock == "--snow" => (true, true),
        _ => return Err("Usage: soil_ledger_probe [--compensated [--snow]]".into()),
    };
    let mut cases = Vec::new();
    let fixtures = if snow {
        [("seasonal-reference", 151), ("moisture-coast", 95)]
    } else {
        [("seasonal-reference", 99), ("moisture-coast", 103)]
    };
    for (seed, region) in fixtures {
        let mut recipe: Recipe = serde_json::from_str(include_str!(
            "../../docs/scenarios/seasonal-temperature.json"
        ))
        .map_err(|e| e.to_string())?;
        recipe.seed = seed.into();
        recipe.radius_meters = 1_000_000.;
        recipe.subdivision = 2;
        let world = World::generate(recipe)?;
        let model = Model::from_world(
            &world,
            Settings {
                orography: Some(Default::default()),
                max_coupled_step_seconds: 60,
                soil_numerics: compensated.then_some(SoilNumerics::Compensated),
                ..Default::default()
            },
            seasonal_temperature::Settings::default(),
            seasonal_wind::Settings::default(),
        )?;
        let mut state = model.initial_state();
        let mut drift = Vec::new();
        let mut flux = Vec::new();
        let mut operations = 0;
        let mut maximum_local_relative_error: f64 = 0.;
        let mut failure = None;
        let days = if snow { 365 } else { 40 };
        let caller = if snow { 3600 } else { 60 };
        while state.elapsed_seconds() < days * 86400 {
            let before = state.clone();
            let result = model.advance_observed(&mut state, caller, |obs| {
                if obs.region != region {
                    return;
                }
                let f = obs.after.transfers;
                let local = if snow {
                    [
                        obs.after.stocks.snow,
                        0.,
                        -obs.before.snow,
                        0.,
                        -f.snowfall,
                        f.melt,
                        0.,
                    ]
                } else {
                    [
                        obs.after.stocks.soil,
                        obs.after_soil_low_kilograms,
                        -obs.before.soil,
                        -obs.before_soil_low_kilograms,
                        -f.infiltration,
                        f.soil_evaporation,
                        f.soil_drainage,
                    ]
                };
                for v in local {
                    add(&mut drift, v);
                }
                let transfers = if snow {
                    [f.snowfall, -f.melt, 0.]
                } else {
                    [f.infiltration, -f.soil_evaporation, -f.soil_drainage]
                };
                for v in transfers {
                    add(&mut flux, v);
                }
                maximum_local_relative_error = maximum_local_relative_error.max(
                    sum(local).abs()
                        / obs
                            .before
                            .total()
                            .max(f.infiltration)
                            .max(f.snowfall)
                            .max(1.),
                );
                operations += 1;
            });
            if let Err(error) = result {
                assert_eq!(state, before);
                failure = Some(error);
                break;
            }
        }
        // Drift/flux include the failed provisional interval. Reconstruct its
        // final stock from the telescoping expansion, not from an edited state.
        let drift_value = drift.iter().copied().sum::<f64>();
        let net = flux.iter().copied().sum::<f64>();
        cases.push(json!({"recipe":world.recipe,"settings":model.settings(),"modelVersion":model.model_version(),"region":region,"lastCommittedSeconds":state.elapsed_seconds(),"callerIntervalSeconds":caller,"requestedDays":days,
            "failure":failure,"failedIntervalAtomic":failure.as_ref().map(|_|true),"provisionalLocalOperations":operations,
            "exactExpansionArithmeticDriftKilograms":drift_value,"exactExpansionNetTransfersKilograms":net,
            "maximumLocalRelativeRoundingResidual":maximum_local_relative_error,
            "driftExpansion":drift,"transferExpansion":flux,
            "scope":"Audit includes any rejected interval's provisional operations. Observations do not modify stocks or tolerances."}));
    }
    println!(
        "{}",
        serde_json::to_string_pretty(
            &json!({"probeVersion":3,"compensated":compensated,"stock":if snow {"snow"} else {"soil"},"cases":cases})
        )
        .map_err(|e| e.to_string())?
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn expansion_audit_retains_small_signed_terms_across_large_cancellation() {
        let mut parts = Vec::new();
        for value in [2_f64.powi(70), 1., 0.25, -0.125, -2_f64.powi(70)] {
            add(&mut parts, value);
        }
        assert_eq!(parts.iter().sum::<f64>(), 1.125);
        let mut exact = 0_i128;
        let mut random = 21_u64;
        add(&mut parts, -1.125);
        add(&mut parts, 2_f64.powi(70));
        for _ in 0..10000 {
            random = random.wrapping_mul(6364136223846793005).wrapping_add(1);
            let value = ((random >> 32) % 2048) as i128 - 1024;
            add(&mut parts, value as f64);
            exact += value;
        }
        add(&mut parts, -2_f64.powi(70));
        assert_eq!(parts.iter().sum::<f64>(), exact as f64);
    }
}
