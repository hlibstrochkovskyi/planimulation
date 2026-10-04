//! Matched accepted version-7/8 trajectories. Not a climate-readiness claim.
use planimulation_core::{
    Recipe, World,
    moisture_transport::total_mass,
    seasonal_moisture::{
        MAX_ELAPSED_SECONDS, Model, ReferenceWaterPool, Settings, SoilNumerics, SurfaceNumerics,
        TerminalNumerics, body_preparation, preparation,
    },
};
use serde_json::{Value, json};
use std::io::Write;

fn run(
    world: &World,
    depth: f64,
    limit: u32,
    years: u32,
    pooled: bool,
    diagnose: bool,
) -> Result<Value, String> {
    let settings = Settings {
        orography: Some(Default::default()),
        soil_numerics: Some(SoilNumerics::Compensated),
        surface_numerics: Some(SurfaceNumerics::Compensated),
        terminal_numerics: Some(TerminalNumerics::Compensated),
        reference_water_pool: pooled.then_some(ReferenceWaterPool::FastConnectedBody),
        initial_active_surface_depth_meters: depth,
        max_coupled_step_seconds: limit,
        ..Default::default()
    };
    let model = Model::from_world(world, settings, Default::default(), Default::default())?;
    let mut state = model.initial_state();
    let criteria = preparation::Criteria::default();
    let mut regional_monitor = if diagnose && !pooled {
        Some(preparation::Monitor::new(&model, &state, criteria)?)
    } else {
        None
    };
    let mut body_monitor = if diagnose && pooled {
        Some(body_preparation::Monitor::new(&model, &state, criteria)?)
    } else {
        None
    };
    let area = total_mass(&world.surface.areas);
    let mut annual = Vec::new();
    let mut last_p = 0.;
    let mut last_e = 0.;
    let mut failure = None;
    let mut atomic = None;
    let mut max_global: f64 = 0.;
    let mut max_local: f64 = 0.;
    'years: for year in 1..=years {
        for _ in 0..365 {
            let before = state.clone();
            match model.advance(&mut state, 86400) {
                Ok(step) => {
                    max_global = max_global.max(
                        step.budget.residual_kilograms.abs()
                            / step.budget.initial_mobile_water_kilograms.max(1.),
                    );
                    max_local =
                        max_local.max(step.budget.maximum_relative_local_surface_ledger_residual);
                }
                Err(error) => {
                    atomic = Some(before == state);
                    failure = Some(error);
                    break 'years;
                }
            }
        }
        let budget = model.budget(&state)?;
        let p = budget.cumulative_precipitation_kilograms;
        let e = budget.cumulative_evaporation_kilograms;
        let cp = state.checkpoint();
        let wet_liquid_and_terminal = if let Some(body) = budget.reference_body_water_kilograms {
            body
        } else {
            let mut parts = Vec::new();
            for (i, &id) in world.water.body_ids.iter().enumerate() {
                if id != 0 {
                    parts.extend([
                        cp.surface_kilograms[i],
                        cp.surface_low_kilograms.as_ref().unwrap()[i],
                        cp.terminal_water_kilograms[i],
                        cp.terminal_low_kilograms.as_ref().unwrap()[i],
                    ]);
                }
            }
            total_mass(&parts)
        };
        let mut entry = json!({"year":year, "precipitationMillimeters":(p-last_p)/area,
            "evaporationMillimeters":(e-last_e)/area, "connectedReferenceLiquidKilograms":wet_liquid_and_terminal,
            "budget":budget});
        let assessment = if let Some(monitor) = &mut regional_monitor {
            Some(
                monitor
                    .observe_year(&state)
                    .and_then(|a| serde_json::to_value(a).map_err(|e| e.to_string())),
            )
        } else {
            body_monitor.as_mut().map(|monitor| {
                monitor
                    .observe_year(&state)
                    .and_then(|a| serde_json::to_value(a).map_err(|e| e.to_string()))
            })
        };
        if let Some(assessment) = assessment {
            match assessment {
                Ok(value) => entry["preparationAssessment"] = value,
                Err(error) => {
                    failure = Some(format!(
                        "Annual diagnostic validation at year {year}: {error}"
                    ));
                    annual.push(entry);
                    break 'years;
                }
            }
        }
        annual.push(entry);
        last_p = p;
        last_e = e;
    }
    let cp = state.checkpoint();
    let text = serde_json::to_string(&cp).map_err(|e| e.to_string())?;
    let (restored_model, mut restored) =
        Model::restore(serde_json::from_str(&text).map_err(|e| e.to_string())?)?;
    let replay = state == restored
        && text == serde_json::to_string(&restored.checkpoint()).map_err(|e| e.to_string())?;
    let continuation = if state.elapsed_seconds() + 3600 <= MAX_ELAPSED_SECONDS {
        let mut a = state.clone();
        let x = model.advance(&mut a, 3600);
        let y = restored_model.advance(&mut restored, 3600);
        Some(
            x.as_ref().map(|_| ()).map_err(String::as_str)
                == y.as_ref().map(|_| ()).map_err(String::as_str)
                && a == restored,
        )
    } else {
        None
    };
    let mut report = json!({"recipe":world.recipe,"settings":settings,"temperatureSettings":model.temperature_settings(),"windSettings":model.wind_settings(),
        "schemaVersion":cp.schema_version,"modelVersion":cp.model_version,"surfaceModelVersion":cp.surface_model_version,
        "referenceBodyModelVersion":cp.reference_body_model_version,"terminalStockModelVersion":cp.terminal_stock_model_version,
        "transportModelVersion":cp.transport_model_version,"temperatureModelVersion":cp.temperature_model_version,
        "windModelVersion":cp.wind_model_version,"runoffModelVersion":cp.runoff_model_version,"orographicModelVersion":cp.orographic_model_version,
        "callerIntervalSeconds":86400,"requestedYears":years,"elapsedSeconds":state.elapsed_seconds(),
        "actualMaximumCoupledStepSeconds":model.maximum_coupled_step_seconds()?,"annual":annual,"budget":model.budget(&state)?,
        "maximumRelativeMassResidual":max_global,"maximumRelativeLocalLedgerResidual":max_local,
        "checkpointRoundTripExact":replay,"checkpointContinuationExact":continuation,
        "failure":failure,"failedIntervalAtomic":atomic,"passed":failure.is_none() && replay && continuation != Some(false)});
    if diagnose {
        report["preparationDiagnosticVersion"] = json!(if pooled {
            body_preparation::DIAGNOSTIC_VERSION
        } else {
            preparation::DIAGNOSTIC_VERSION
        });
        report["preparationCriteria"] = json!(criteria);
        report["lastDiagnosticObservationSeconds"] = json!(
            regional_monitor
                .as_ref()
                .map(preparation::Monitor::last_observed_seconds)
                .or_else(|| body_monitor
                    .as_ref()
                    .map(body_preparation::Monitor::last_observed_seconds))
        );
    }
    Ok(report)
}

fn main() -> Result<(), String> {
    let mut args: Vec<_> = std::env::args().skip(1).collect();
    let output_path = if let Some(index) = args.iter().position(|a| a == "--output") {
        if index + 1 >= args.len() || args[index + 1].starts_with("--") {
            return Err("--output requires a new output-file path.".into());
        }
        let path = args.remove(index + 1);
        args.remove(index);
        Some(path)
    } else {
        None
    };
    if args
        .iter()
        .any(|a| a != "--refined" && a != "--decade" && a != "--preparation")
        || args.iter().filter(|a| *a == "--refined").count() > 1
        || args.iter().filter(|a| *a == "--decade").count() > 1
        || args.iter().filter(|a| *a == "--preparation").count() > 1
    {
        return Err(
            "Usage: reference_pool_report [--refined] [--decade] [--preparation] [--output NEW_FILE]".into(),
        );
    }
    let output = output_path
        .map(|path| {
            std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(path)
                .map_err(|e| e.to_string())
        })
        .transpose()?;
    let years = if args.iter().any(|a| a == "--decade") {
        10
    } else {
        2
    };
    let diagnose = args.iter().any(|a| a == "--preparation");
    let limit = if args.iter().any(|a| a == "--refined") {
        450
    } else {
        900
    };
    let base: Recipe = serde_json::from_str(include_str!(
        "../../docs/scenarios/seasonal-temperature.json"
    ))
    .map_err(|e| e.to_string())?;
    let mut cases = Vec::new();
    for radius in [1_000_000., 6_371_000.] {
        for depth in [1., 10.] {
            let mut recipe = base.clone();
            recipe.subdivision = 2;
            recipe.radius_meters = radius;
            let world = World::generate(recipe.clone())?;
            for pooled in [false, true] {
                eprintln!(
                    "Reference pool: radius {radius}, depth {depth}, pooled {pooled}, years {years}, ceiling {limit} s"
                );
                cases.push(match run(&world, depth, limit, years, pooled, diagnose) {
                    Ok(value) => value,
                    Err(error) => json!({"recipe":recipe,"depthMeters":depth,"pooled":pooled,"requestedYears":years,"coupledCeilingSeconds":limit,"passed":false,"stage":"construction or persistence validation","failure":error}),
                });
            }
        }
    }
    let failures = cases.iter().filter(|c| c["passed"] != true).count();
    let mut report = json!({"reportVersion":1,"failureCount":failures,"numericRunQualificationPassed":failures==0,"cases":cases,
        "scope":"Matched version-7/8 finite active inventories with fixed reference geography and local atmosphere response; common connected-body availability is an explicit uncalibrated approximation. Regional arrival provenance remains recorded. Annual flows difference leading cumulative totals, not independent integrals. No default or desktop promotion, stationarity classification, clock reset, new lake geometry, current solver, or climate-readiness claim."});
    if diagnose {
        report["reportVersion"] = json!(2);
        report["bodyPreparationDiagnosticVersion"] = json!(body_preparation::DIAGNOSTIC_VERSION);
        report["regionalStockColumns"] =
            json!(["liquid", "snow", "soil", "transit", "terminal", "vapor"]);
        report["regionalFlowColumns"] = json!([
            "precipitation",
            "evaporation",
            "generatedRunoff",
            "snowfall",
            "terminalDelivery"
        ]);
        report["bodyFlowColumns"] =
            json!(["rain", "melt", "terminalDelivery", "liquidEvaporation"]);
        report["scope"] = json!(
            "Adds read-only whole-year stationarity assessments with distinct version-7 regional and version-8 body ownership. All physical report fields retain the unobserved path. A candidate under recorded uncalibrated criteria is not climate/eco readiness; body areas are fixed reference footprints, not virtual regional stock or changing lake surfaces. Body flow totals are explanatory leading-field differences; eligibility uses all owner stocks and regional annual flow comparisons. No physical law, conservation tolerance, clock, checkpoint, default, or desktop change."
        );
    }
    if let Some(mut file) = output {
        serde_json::to_writer(&mut file, &report).map_err(|e| e.to_string())?;
        writeln!(file).map_err(|e| e.to_string())?;
        eprintln!(
            "Saved full report: {} cases, {failures} failures.",
            report["cases"].as_array().unwrap().len()
        );
    } else {
        println!("{report}");
    }
    if failures != 0 {
        return Err(format!(
            "{failures} reference-pool cases failed; report retains failures."
        ));
    }
    Ok(())
}
