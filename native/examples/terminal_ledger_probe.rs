//! Independent floating-expansion audit of the retained terminal precision gate.
use planimulation_core::{
    Recipe, World,
    seasonal_moisture::{Model, Settings, SoilNumerics, SurfaceNumerics, TerminalNumerics},
};
use serde_json::json;

fn add(parts: &mut Vec<f64>, value: f64) {
    let mut q = value;
    let mut next = Vec::new();
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
fn main() -> Result<(), String> {
    let mut recipe: Recipe = serde_json::from_str(include_str!(
        "../../docs/scenarios/seasonal-temperature.json"
    ))
    .map_err(|e| e.to_string())?;
    recipe.subdivision = 2;
    recipe.radius_meters = 1_000_000.;
    let world = World::generate(recipe.clone())?;
    let mut cases = Vec::new();
    for precise in [false, true] {
        eprintln!("Terminal audit: compensated={precise}, region 8, up to ten years");
        let settings = Settings {
            orography: Some(Default::default()),
            soil_numerics: Some(SoilNumerics::Compensated),
            surface_numerics: Some(SurfaceNumerics::Compensated),
            terminal_numerics: precise.then_some(TerminalNumerics::Compensated),
            max_coupled_step_seconds: 60,
            ..Default::default()
        };
        let model = Model::from_world(&world, settings, Default::default(), Default::default())?;
        let mut state = model.initial_state();
        let mut drift = Vec::new();
        let mut failure = None;
        let mut operations = 0;
        let mut atomic = None;
        for _ in 0..3650 * 24 {
            let before = state.clone();
            let result = model.advance_terminal_observed(&mut state, 3600, |obs| {
                if obs.region != 8 {
                    return;
                }
                for term in [
                    obs.after_kilograms,
                    obs.after_low_kilograms,
                    -obs.before_kilograms,
                    -obs.before_low_kilograms,
                    -obs.received_kilograms,
                    obs.evaporated_kilograms,
                ] {
                    add(&mut drift, term);
                }
                operations += 1;
            });
            if let Err(error) = result {
                atomic = Some(before == state);
                failure = Some(error);
                break;
            }
        }
        cases.push(json!({"recipe":recipe,"settings":settings,"modelVersion":model.model_version(),"terminalStockModelVersion":state.checkpoint().terminal_stock_model_version,
            "region":8,"requestedDays":3650,"callerIntervalSeconds":3600,"lastCommittedSeconds":state.elapsed_seconds(),
            "failure":failure,"failedIntervalAtomic":atomic,"provisionalLocalOperations":operations,
            "arithmeticDriftKilograms":drift.iter().copied().sum::<f64>(),"driftExpansion":drift,
            "scope":"Copied provisional debit/credit observations, including any rejected caller interval; not committed transfer totals. No production stock/ledger/tolerance is changed."}));
    }
    println!(
        "{}",
        serde_json::to_string_pretty(&json!({"probeVersion":1,"cases":cases}))
            .map_err(|e| e.to_string())?
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn signed_dyadic_cancellation_matches_an_independent_integer_oracle() {
        let mut parts = Vec::new();
        add(&mut parts, 2_f64.powi(70));
        let mut exact = 0_i128;
        for n in 0..10000 {
            let term = (n % 2048) as i128 - 1024;
            add(&mut parts, term as f64);
            exact += term;
        }
        add(&mut parts, -2_f64.powi(70));
        assert_eq!(parts.iter().sum::<f64>(), exact as f64);
    }
}
