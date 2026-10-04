//! Matched finite-donor ridge controls and generated-world response sensitivity.
use planimulation_core::{
    Recipe, Surface, World,
    moisture_transport::{self, Flow, Geometry, total_mass},
    orographic_response,
    seasonal_moisture::{self, Model, State},
    seasonal_temperature, seasonal_wind,
};
use serde_json::{Value, json};

fn ridge(
    level: u32,
    seconds: u32,
    direction: f64,
    height: f64,
    strength: f64,
) -> Result<Value, String> {
    let radius = 1_000_000.;
    let speed = 20.;
    let surface = Surface::build(level, radius);
    let geometry = Geometry::from_surface(&surface, radius)?;
    let velocity = |p: [f64; 3]| [-direction * speed * p[2], 0., direction * speed * p[0]];
    let flow = Flow::sample(&geometry, velocity)?;
    let heights: Vec<_> = surface
        .centers
        .iter()
        .map(|p| {
            let lon = p[2].atan2(p[0]);
            height * (-lon.powi(2) / 0.16 - p[1].powi(2) / 0.3).exp()
        })
        .collect();
    let gradients = orographic_response::terrain_gradients(&surface, radius, &heights)?;
    let response = orographic_response::Settings {
        strength,
        ..Default::default()
    };
    let rates: Vec<_> = surface
        .centers
        .iter()
        .zip(gradients)
        .map(|(&p, g)| {
            response.additional_rate_per_second(orographic_response::uplift(p, g, velocity(p))?)
        })
        .collect::<Result<_, _>>()?;
    let capacities: Vec<_> = heights
        .iter()
        .zip(&surface.areas)
        .map(|(&h, &area)| {
            Ok(
                seasonal_moisture::saturation_column_kilograms_per_square_meter(
                    20. - 0.0065 * h,
                    2000.,
                )? * area,
            )
        })
        .collect::<Result<_, String>>()?;
    let initial_lon = -direction * 0.8;
    let mut vapor: Vec<_> = surface
        .centers
        .iter()
        .zip(&surface.areas)
        .map(|(p, area)| {
            32. * (8. * (p[0] * initial_lon.cos() + p[2] * initial_lon.sin() - 1.)).exp() * area
        })
        .collect();
    let initial = total_mass(&vapor);
    let mut rain = vec![0.; vapor.len()];
    let duration = 100_000;
    if duration % seconds != 0 {
        return Err("Ridge interval must divide its duration.".into());
    }
    let mut maximum_residual: f64 = 0.;
    for _ in 0..duration / seconds {
        for phase in 0..2 {
            if phase == 1 {
                vapor = flow
                    .advance(
                        &vapor,
                        seconds as f64,
                        moisture_transport::Settings::default(),
                    )?
                    .stock_kilograms;
            }
            for i in 0..vapor.len() {
                let p = orographic_response::deposition(
                    vapor[i],
                    capacities[i],
                    1. / 21600.,
                    rates[i],
                    seconds as f64 * 0.5,
                )?;
                vapor[i] -= p;
                rain[i] += p;
            }
        }
        maximum_residual = maximum_residual
            .max((total_mass(&vapor) + total_mass(&rain) - initial).abs() / initial);
    }
    let mut windward = 0.;
    let mut lee = 0.;
    let mut downstream = 0.;
    for (i, p) in surface.centers.iter().enumerate() {
        let along = direction * p[2].atan2(p[0]);
        if p[1].abs() < 0.5 {
            if (-0.8..0.).contains(&along) {
                windward += rain[i];
            }
            if (0. ..0.8).contains(&along) {
                lee += rain[i];
            }
            if (0.4..1.4).contains(&along) {
                downstream += vapor[i];
            }
        }
    }
    Ok(
        json!({"level": level, "radiusMeters": radius, "windMetersPerSecond": speed,
        "direction": direction, "ridgeHeightMeters": height, "strength": strength,
        "durationSeconds": duration, "stepSeconds": seconds, "initialVaporKilograms": initial,
        "rainKilograms": total_mass(&rain), "windwardRainKilograms": windward, "leeRainKilograms": lee,
        "downstreamVaporKilograms": downstream, "maximumRelativeBudgetResidual": maximum_residual,
        "vapor": vapor, "rain": rain}),
    )
}

fn model(world: &World, settings: seasonal_moisture::Settings) -> Result<Model, String> {
    Model::from_world(
        world,
        settings,
        seasonal_temperature::Settings::default(),
        seasonal_wind::Settings::default(),
    )
}
fn run(model: &Model, days: u32, seconds: u32) -> Result<(State, Value), String> {
    let mut state = model.initial_state();
    let mut residual: f64 = 0.;
    let mut ledger: f64 = 0.;
    let mut steps = 0;
    for _ in 0..days * 86400 / seconds {
        let step = model.advance(&mut state, seconds)?;
        residual = residual.max(
            step.budget.residual_kilograms.abs()
                / step.budget.initial_mobile_water_kilograms.max(1.),
        );
        ledger = ledger.max(step.budget.maximum_relative_local_surface_ledger_residual);
        steps += step.coupled_substeps;
    }
    let summary = json!({"modelVersion": model.model_version(), "settings": model.settings(),
        "actualMaximumCoupledStepSeconds": model.maximum_coupled_step_seconds()?, "callerIntervalSeconds": seconds,
        "days": days, "coupledSubsteps": steps, "maximumRelativeMassResidual": residual,
        "maximumRelativeRegionalLedgerResidual": ledger, "budget": model.budget(&state)?});
    Ok((state, summary))
}
fn generated(recipe: Recipe) -> Result<Value, String> {
    let world = World::generate(recipe)?;
    let settings = seasonal_moisture::Settings {
        orography: Some(Default::default()),
        ..Default::default()
    };
    let baseline = model(&world, Default::default())?;
    let control = model(
        &world,
        seasonal_moisture::Settings {
            orography: Some(orographic_response::Settings {
                strength: 0.,
                ..Default::default()
            }),
            ..Default::default()
        },
    )?;
    let response = model(&world, settings)?;
    let finer = model(
        &world,
        seasonal_moisture::Settings {
            max_coupled_step_seconds: 60,
            ..settings
        },
    )?;
    let (a, legacy) = run(&baseline, 40, 86400)?;
    let (b, zero_strength) = run(&control, 40, 86400)?;
    let (c, enabled) = run(&response, 40, 3600)?;
    let matched = model(
        &world,
        seasonal_moisture::Settings {
            max_coupled_step_seconds: response.maximum_coupled_step_seconds()? as u32,
            ..Default::default()
        },
    )?;
    let (_, matched_cadence) = run(&matched, 40, 3600)?;
    let original = c.clone();
    let encoded = serde_json::to_vec(&c.checkpoint()).map_err(|e| e.to_string())?;
    let (restored_model, mut restored) =
        Model::restore(serde_json::from_slice(&encoded).map_err(|e| e.to_string())?)?;
    let roundtrip = restored == original;
    let mut continued = original.clone();
    response.advance(&mut continued, 3600)?;
    restored_model.advance(&mut restored, 3600)?;
    let mut report = json!({"recipe": world.recipe, "regionCount": world.surface.centers.len(), "baseline": legacy,
        "zeroStrength": zero_strength, "zeroStrengthStocksExact": a.owned_stocks().eq(b.owned_stocks()),
        "enabled": enabled, "matchedCadenceBaseline": matched_cadence,
        "refinementActuallyReducedCoupledLimit": finer.maximum_coupled_step_seconds()? < response.maximum_coupled_step_seconds()?,
        "checkpointRoundTripExact": roundtrip, "checkpointContinuationExact": continued == restored});
    match run(&finer, 40, 60) {
        Ok((d, refined)) => {
            let scale = response.budget(&c)?.initial_mobile_water_kilograms.max(1.);
            report["relativeSixStockL1Difference"] = json!(
                c.owned_stocks()
                    .zip(d.owned_stocks())
                    .map(|(a, b)| (a - b).abs())
                    .sum::<f64>()
                    / scale
            );
            report["refined"] = refined;
        }
        Err(error) => {
            report["refined"] = json!({"stage": "60-second refinement", "settings": finer.settings(),
                "actualMaximumCoupledStepSeconds": finer.maximum_coupled_step_seconds()?, "failure": error});
        }
    }
    Ok(report)
}
fn compact(mut value: Value) -> Value {
    value.as_object_mut().unwrap().remove("vapor");
    value.as_object_mut().unwrap().remove("rain");
    value
}
fn directed() -> Result<Vec<Value>, String> {
    let mut cases = Vec::new();
    for level in [3, 4] {
        for direction in [1., -1.] {
            for (height, strength) in [(0., 1.), (3000., 0.), (3000., 1.)] {
                let coarse = ridge(level, 500, direction, height, strength)?;
                let fine = ridge(level, 250, direction, height, strength)?;
                let error: f64 = ["vapor", "rain"]
                    .into_iter()
                    .map(|key| {
                        coarse[key]
                            .as_array()
                            .unwrap()
                            .iter()
                            .zip(fine[key].as_array().unwrap())
                            .map(|(a, b)| (a.as_f64().unwrap() - b.as_f64().unwrap()).abs())
                            .sum::<f64>()
                    })
                    .sum::<f64>()
                    / coarse["initialVaporKilograms"].as_f64().unwrap();
                cases.push(json!({"coarse": compact(coarse), "refined": compact(fine), "relativeVaporAndRainL1Difference": error}));
            }
        }
    }
    Ok(cases)
}
fn main() -> Result<(), String> {
    let mut cases = Vec::new();
    let base: Recipe = serde_json::from_str(include_str!(
        "../../docs/scenarios/seasonal-temperature.json"
    ))
    .map_err(|e| e.to_string())?;
    for seed in ["seasonal-reference", "moisture-coast", "moisture-interior"] {
        for radius in [1_000_000., 6_371_000.] {
            let mut recipe = base.clone();
            recipe.seed = seed.into();
            recipe.subdivision = 2;
            recipe.radius_meters = radius;
            cases.push(match generated(recipe.clone()) {
                Ok(v) => v,
                Err(error) => json!({"recipe": recipe, "failure": error}),
            });
        }
    }
    let report = json!({"reportVersion": 1, "orographicModelVersion": orographic_response::MODEL_VERSION,
        "directedRidgeControls": directed()?, "generatedCases": cases,
        "scope": "Finite closed donor. Ridge controls prescribe wind, terrain and altitude-dependent capacity; they do not simulate weather or clouds. Generated cases retain all failures. Empirical upslope acceleration is not calibrated."});
    println!(
        "{}",
        serde_json::to_string_pretty(&report).map_err(|e| e.to_string())?
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn a_finite_moist_pulse_crosses_a_ridge_with_directional_depletion() {
        for direction in [1., -1.] {
            let flat = ridge(3, 500, direction, 0., 1.).unwrap();
            let baseline = ridge(3, 500, direction, 3000., 0.).unwrap();
            let response = ridge(3, 500, direction, 3000., 1.).unwrap();
            for case in [&flat, &baseline, &response] {
                assert!(case["maximumRelativeBudgetResidual"].as_f64().unwrap() < 1e-12);
            }
            assert_eq!(flat["rainKilograms"].as_f64().unwrap(), 0.);
            assert!(baseline["rainKilograms"].as_f64().unwrap() > 0.);
            assert!(
                response["windwardRainKilograms"].as_f64().unwrap()
                    > baseline["windwardRainKilograms"].as_f64().unwrap()
            );
            assert!(
                response["leeRainKilograms"].as_f64().unwrap()
                    < baseline["leeRainKilograms"].as_f64().unwrap()
            );
            assert!(
                response["downstreamVaporKilograms"].as_f64().unwrap()
                    < baseline["downstreamVaporKilograms"].as_f64().unwrap()
            );
        }
    }
}
