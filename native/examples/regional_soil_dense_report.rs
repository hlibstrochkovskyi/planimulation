//! Dense generated-world qualification, not a spatial-convergence claim.
use planimulation_core::{
    Recipe, World,
    moisture_transport::{Geometry, total_mass},
    seasonal_moisture::regional_soil::{
        Checkpoint, Model, NumericalPolicy, Settings, SurfaceOperator,
    },
    seasonal_temperature,
    surface_water::ponded_soil::Mass,
};
use serde_json::{Value, json};
use std::io::{BufWriter, Write};

const START_SECONDS: u64 = 900;
const INPUT_KILOGRAMS_PER_SQUARE_METER: f64 = 50_000.;

fn add(m: &mut Mass, amount: f64) {
    let sum = m.high + amount;
    let v = sum - m.high;
    let tail = m.low + ((m.high - (sum - v)) + (amount - v));
    let high = sum + tail;
    let v = high - sum;
    m.high = high;
    m.low = (sum - (high - v)) + (tail - v);
}
fn sum(masses: &[Mass]) -> f64 {
    total_mass(
        &masses
            .iter()
            .flat_map(|m| [m.high, m.low])
            .collect::<Vec<_>>(),
    )
}
fn recipe(seed: &str, subdivision: u32) -> Result<Recipe, String> {
    let mut recipe: Recipe =
        serde_json::from_str(include_str!("../../docs/scenarios/spill-connections.json"))
            .map_err(|e| e.to_string())?;
    recipe.seed = seed.into();
    recipe.subdivision = subdivision;
    recipe.water = planimulation_core::water::WaterSettings::Coverage { fraction: 0.3 };
    Ok(recipe)
}
// Finite-funded adjacent history as in the earlier annual report, but from the
// first canonical main-ocean/land contact: small dense-grid lakes cannot fund
// this stress depth. The contact is mesh-dependent; cross-mesh inputs are NOT equal.
fn fund(world: &World, cp: &mut Checkpoint) -> Result<Value, String> {
    let geometry = Geometry::from_surface(&world.surface, world.recipe.radius_meters)?;
    let (face, source, target) = geometry
        .boundaries()
        .iter()
        .enumerate()
        .find_map(|(f, b)| {
            let [a, c] = b.regions;
            if world.water.main_ocean_id > 0
                && world.water.body_ids[a] == world.water.main_ocean_id
                && world.water.body_ids[c] == 0
            {
                Some((f, a, c))
            } else if world.water.main_ocean_id > 0
                && world.water.body_ids[c] == world.water.main_ocean_id
                && world.water.body_ids[a] == 0
            {
                Some((f, c, a))
            } else {
                None
            }
        })
        .ok_or("No actual main-ocean/land contact for finite funding.")?;
    let mut ids = world.water.body_ids.clone();
    ids.sort_unstable();
    ids.dedup();
    ids.retain(|&id| id > 0);
    let body = ids
        .binary_search(&world.water.body_ids[source])
        .map_err(|_| "Missing body owner.")?;
    let amount = world.surface.areas[target] * INPUT_KILOGRAMS_PER_SQUARE_METER;
    if amount >= cp.reference_bodies[body].high {
        return Err("Finite body cannot fund input.".into());
    }
    add(&mut cp.reference_bodies[body], -amount);
    add(&mut cp.local_transfers[source].liquid_evaporation, amount);
    add(
        &mut cp.atmospheric_transfers[2 * face + usize::from(source > target)],
        amount,
    );
    add(&mut cp.local_transfers[target].rain, amount);
    add(&mut cp.liquid[target], amount);
    cp.elapsed_seconds = START_SECONDS;
    Ok(
        json!({"sourceRegion":source,"targetRegion":target,"face":face,
        "bodyId":world.water.body_ids[source],"bodySlot":body,
        "sourceDirection":world.surface.centers[source],"targetDirection":world.surface.centers[target],
        "targetAreaSquareMeters":world.surface.areas[target],"kilograms":amount,
        "kilogramsPerSquareMeter":INPUT_KILOGRAMS_PER_SQUARE_METER,"naturallyReachedRainfall":false,
        "selection":"first canonical main-ocean/land contact"}),
    )
}
struct Run {
    report: Value,
    initial_checkpoint: Checkpoint,
    final_checkpoint: Checkpoint,
    complete: bool,
}
fn run(
    world: &World,
    origin: &Checkpoint,
    days: u32,
    ceiling: u32,
    cap: f64,
) -> Result<Run, String> {
    let mut start = origin.clone();
    start.settings.max_coupled_step_seconds = ceiling;
    start
        .settings
        .surface_flow
        .maximum_diffusivity_square_meters_per_second = cap;
    let initial_checkpoint = start.clone();
    let (model, mut state) = Model::restore(start)?;
    let initial_budget = model.budget(&state)?;
    let (mut global, mut local) = (
        initial_budget.relative_global_residual,
        initial_budget.maximum_relative_local_residual,
    );
    let mut accepted_days = 0;
    let mut failure = Value::Null;
    let mut counts = [0_u64; 3];
    for day in 1..=days {
        let before = state.clone();
        match model.advance(&mut state, 86400) {
            Ok(step) => {
                accepted_days += 1;
                global = global.max(step.budget.relative_global_residual);
                local = local.max(step.budget.maximum_relative_local_residual);
                counts[0] += step.coupled_substeps as u64;
                counts[1] += step.atmospheric_substeps as u64;
                counts[2] += step.surface_substeps as u64;
            }
            Err(message) => {
                failure = json!({"firstRefusedDay":day,"message":message,"atomicRollback":state == before});
                break;
            }
        }
    }
    let cp = state.checkpoint();
    let text = serde_json::to_string(&cp).map_err(|e| e.to_string())?;
    let (resumed, mut saved) =
        Model::restore(serde_json::from_str(&text).map_err(|e| e.to_string())?)?;
    let round_trip =
        text == serde_json::to_string(&saved.checkpoint()).map_err(|e| e.to_string())?;
    let mut direct = state.clone();
    let continuation = match (
        model.advance(&mut direct, 3600),
        resumed.advance(&mut saved, 3600),
    ) {
        (Ok(_), Ok(_)) => json!({"accepted":true,"completeJsonExact":
            serde_json::to_string(&direct.checkpoint()).map_err(|e| e.to_string())?
            == serde_json::to_string(&saved.checkpoint()).map_err(|e| e.to_string())?}),
        (Err(a), Err(b)) => {
            json!({"accepted":false,"completeJsonExact":a == b && direct == saved && direct == state,"message":a})
        }
        _ => {
            json!({"accepted":false,"completeJsonExact":false,"message":"Replay acceptance mismatch."})
        }
    };
    let land_area = total_mass(
        &world
            .surface
            .areas
            .iter()
            .enumerate()
            .filter_map(|(r, &a)| (world.water.body_ids[r] == 0).then_some(a))
            .collect::<Vec<_>>(),
    );
    let soil_area = total_mass(
        &world
            .surface
            .areas
            .iter()
            .enumerate()
            .filter_map(|(r, &a)| (cp.soil[r].high > 0.).then_some(a))
            .collect::<Vec<_>>(),
    );
    let active_directions = cp.surface_transfers.iter().filter(|m| m.high > 0.).count();
    let max_depth = cp
        .liquid
        .iter()
        .zip(&world.surface.areas)
        .map(|(m, &a)| m.high / (1000. * a))
        .fold(0., f64::max);
    let complete = accepted_days == days && failure.is_null();
    let report = json!({"coupledCeilingSeconds":ceiling,"diffusivityCapSquareMetersPerSecond":cap,
        "requestedDays":days,"acceptedDays":accepted_days,"failure":failure,
        "initialBudget":initial_budget,"maximumRelativeGlobalResidual":global,"maximumRelativeLocalResidual":local,
        "coupledSubsteps":counts[0],"atmosphericSubsteps":counts[1],"surfaceSubsteps":counts[2],
        "roundTripCompleteJsonExact":round_trip,"continuation":continuation,
        "activeSurfaceDirections":active_directions,"surfaceCrossingsKilograms":sum(&cp.surface_transfers),
        "stockKilograms":{"liquid":sum(&cp.liquid),"soil":sum(&cp.soil),"snow":sum(&cp.snow),
            "vapor":sum(&cp.vapor),"drainage":sum(&cp.drainage),"referenceBodies":sum(&cp.reference_bodies)},
        "positiveSoilLandAreaFraction":soil_area / land_area,"maximumLiquidDepthMeters":max_depth,
        "checks":{"completed":complete,"replayExact":round_trip && continuation["completeJsonExact"] == true,
            "surfaceActivityMatchesCap":if cap == 0. { active_directions == 0 } else { active_directions > 0 },
            "soilExchangeActive":cp.local_transfers.iter().any(|f| f.infiltration.high > 0.)},
        "finalCheckpoint":cp});
    Ok(Run {
        report,
        initial_checkpoint,
        final_checkpoint: cp,
        complete,
    })
}
fn field_difference(a: &[Mass], b: &[Mass], areas: &[f64]) -> Result<Value, String> {
    if a.len() != b.len() || a.len() != areas.len() {
        return Err("Comparison field shape mismatch.".into());
    }
    let differences: Vec<_> = a
        .iter()
        .zip(b)
        .map(|(a, b)| total_mass(&[a.high, -b.high, a.low, -b.low]).abs())
        .collect();
    let l1 = total_mass(&differences);
    let denominator = sum(b);
    Ok(
        json!({"l1Kilograms":l1,"relativeToSecondStock":(denominator > 0.).then_some(l1 / denominator),
        "maximumColumnDifferenceMillimeters":differences.iter().zip(areas).map(|(&d,&a)| d / a).fold(0.,f64::max)}),
    )
}
// Body inventories have no regional footprint. Never label their differences
// as column depth or assign a fictitious unit area to obtain a depth metric.
fn body_difference(a: &[Mass], b: &[Mass]) -> Result<Value, String> {
    if a.len() != b.len() {
        return Err("Comparison body-owner shape mismatch.".into());
    }
    let differences: Vec<_> = a
        .iter()
        .zip(b)
        .map(|(a, b)| total_mass(&[a.high, -b.high, a.low, -b.low]).abs())
        .collect();
    let l1 = total_mass(&differences);
    let denominator = sum(b);
    Ok(
        json!({"l1Kilograms":l1,"relativeToSecondStock":(denominator > 0.).then_some(l1 / denominator)}),
    )
}
// Never compare different geography, forcing, initial time, or failed endpoints.
fn compare(a: &Run, b: &Run, areas: &[f64], control: &str) -> Result<Value, String> {
    let (a_cp, b_cp) = (&a.final_checkpoint, &b.final_checkpoint);
    if !a.complete || !b.complete || a_cp.elapsed_seconds != b_cp.elapsed_seconds {
        return Ok(
            json!({"control":control,"comparable":false,"reason":"Incomplete or unequal-time endpoints."}),
        );
    }
    let mut a_settings = a_cp.settings;
    match control {
        "cadence" => a_settings.max_coupled_step_seconds = b_cp.settings.max_coupled_step_seconds,
        "mobilityCap" => {
            a_settings
                .surface_flow
                .maximum_diffusivity_square_meters_per_second = b_cp
                .settings
                .surface_flow
                .maximum_diffusivity_square_meters_per_second
        }
        _ => return Err("Unknown comparison control.".into()),
    }
    if a_cp.recipe != b_cp.recipe
        || a_cp.temperature_settings != b_cp.temperature_settings
        || a_cp.wind_settings != b_cp.wind_settings
        || a_settings != b_cp.settings
    {
        return Err("Comparison changed geography, forcing or another setting.".into());
    }
    let mut normalized_initial = a.initial_checkpoint.clone();
    normalized_initial.settings = a_settings;
    if normalized_initial != b.initial_checkpoint {
        return Err(
            "Comparison changed the complete initial stock, history, pins or clock.".into(),
        );
    }
    let stock_fields = ["liquid", "soil", "snow", "vapor", "drainage"];
    let mut fields = serde_json::Map::new();
    for (name, (a, b)) in stock_fields.into_iter().zip([
        (&a_cp.liquid, &b_cp.liquid),
        (&a_cp.soil, &b_cp.soil),
        (&a_cp.snow, &b_cp.snow),
        (&a_cp.vapor, &b_cp.vapor),
        (&a_cp.drainage, &b_cp.drainage),
    ]) {
        fields.insert(name.into(), field_difference(a, b, areas)?);
    }
    let a_flow = sum(&a_cp.surface_transfers);
    let b_flow = sum(&b_cp.surface_transfers);
    if a_cp.settings.surface_operator == SurfaceOperator::CotangentWeakForm {
        fields.insert(
            "referenceBodies".into(),
            body_difference(&a_cp.reference_bodies, &b_cp.reference_bodies)?,
        );
    }
    Ok(
        json!({"control":control,"comparable":true,"elapsedSeconds":a_cp.elapsed_seconds,
        "firstCeilingSeconds":a_cp.settings.max_coupled_step_seconds,"secondCeilingSeconds":b_cp.settings.max_coupled_step_seconds,
        "firstCap":a_cp.settings.surface_flow.maximum_diffusivity_square_meters_per_second,
        "secondCap":b_cp.settings.surface_flow.maximum_diffusivity_square_meters_per_second,
        "stockDifferences":fields,"absoluteGrossSurfaceCrossingDifferenceKilograms":(a_flow-b_flow).abs(),
        "relativeGrossSurfaceCrossingDifference":(b_flow > 0.).then_some((a_flow-b_flow).abs()/b_flow)}),
    )
}
fn geography_difference(coarse: &World, fine: &World) -> Result<Value, String> {
    let n = coarse.surface.areas.len();
    if fine.surface.centers.get(..n) != Some(coarse.surface.centers.as_slice()) {
        return Err("Meshes did not preserve common directions.".into());
    }
    let area = total_mass(&coarse.surface.areas);
    let weighted_bed: Vec<_> = (0..n)
        .map(|r| {
            coarse.surface.areas[r]
                * (coarse.terrain.elevation[r] - fine.terrain.elevation[r]).abs()
        })
        .collect();
    let changed_mask: Vec<_> = (0..n)
        .filter_map(|r| {
            ((coarse.water.body_ids[r] > 0) != (fine.water.body_ids[r] > 0))
                .then_some(coarse.surface.areas[r])
        })
        .collect();
    let a = seasonal_temperature::Normals::from_world(coarse, Default::default())?;
    let b = seasonal_temperature::Normals::from_world(fine, Default::default())?;
    let thermal: Vec<_> = (0..n)
        .map(|r| {
            coarse.surface.areas[r] * (a.annual_mean_celsius[r] - b.annual_mean_celsius[r]).abs()
        })
        .collect();
    Ok(json!({"commonDirections":n,"exactCommonDirections":true,
        "areaWeightedMeanAbsoluteBedDifferenceMeters":total_mass(&weighted_bed)/area,
        "areaWeightedInitialWetClassificationDisagreement":total_mass(&changed_mask)/area,
        "areaWeightedMeanAbsoluteAnnualTemperatureDifferenceCelsius":total_mass(&thermal)/area,
        "spatialConvergenceIsolated":false}))
}
fn report(days: u32) -> Result<Value, String> {
    report_with_operator(days, SurfaceOperator::BarycentricTwoPoint)
}
fn report_with_operator(days: u32, operator: SurfaceOperator) -> Result<Value, String> {
    if !(1..=365).contains(&days) {
        return Err("Report supports 1–365 days.".into());
    }
    let mut groups = Vec::new();
    let mut geography = Vec::new();
    for seed in ["first-light", "receiver-2"] {
        let mut previous = None;
        for subdivision in [3, 4] {
            let world = World::generate(recipe(seed, subdivision)?)?;
            if let Some(coarse) = &previous {
                geography
                    .push(json!({"seed":seed,"comparison":geography_difference(coarse,&world)?}));
            }
            let model = Model::from_world(
                &world,
                Settings {
                    numerical_policy: NumericalPolicy::RetainDonor,
                    surface_operator: operator,
                    ..Default::default()
                },
                Default::default(),
                Default::default(),
            )?;
            let mut origin = model.initial_state().checkpoint();
            let funding = fund(&world, &mut origin)?;
            // Only these two settings vary. Every branch shares the complete
            // funded initial stock/history, geography and absolute clock.
            let controls = [(900, 1e6), (450, 1e6), (225, 1e6), (900, 1e5), (900, 0.)];
            let mut runs = Vec::new();
            for (ceiling, cap) in controls {
                eprintln!("Dense soil: {seed}, {subdivision}, {ceiling} s, cap {cap}, {days} days");
                runs.push(run(&world, &origin, days, ceiling, cap)?);
            }
            let comparisons = [
                compare(&runs[0], &runs[1], &world.surface.areas, "cadence")?,
                compare(&runs[1], &runs[2], &world.surface.areas, "cadence")?,
                compare(&runs[0], &runs[3], &world.surface.areas, "mobilityCap")?,
                compare(&runs[0], &runs[4], &world.surface.areas, "mobilityCap")?,
            ];
            groups.push(json!({"seed":seed,"subdivision":subdivision,"regions":world.surface.areas.len(),
                "funding":funding,"initialCheckpoint":origin,
                "runs":runs.iter().map(|r| &r.report).collect::<Vec<_>>(),"comparisons":comparisons}));
            previous = Some(world);
        }
    }
    Ok(
        json!({"reportVersion":if operator == SurfaceOperator::CotangentWeakForm {"regional-soil-cotan-dense-report-1"} else {"regional-soil-dense-report-1"},"requestedDays":days,
        "scope":if operator == SurfaceOperator::CotangentWeakForm {
            "Dense finite-funded cotangent seasonal worlds; matched same-mesh cadence and mobility controls, including finite-body differences. Surface histories are weak-form numerical adjacency exchanges, not literal barycentric-face discharge. Not isolated spatial convergence, natural rainfall or calibrated hydraulics."
        } else {"Dense finite-funded generated worlds; matched same-mesh cadence and mobility controls. Not isolated spatial convergence, natural rainfall or calibrated hydraulics."},
        "groups":groups,"geographyComparisons":geography}),
    )
}
fn main() -> Result<(), String> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if !(args.len() == 2 || (args.len() == 3 && args[2] == "--cotangent")) {
        return Err("Usage: regional_soil_dense_report DAYS NEW_OUTPUT.json [--cotangent]".into());
    }
    let days: u32 = args[0].parse().map_err(|_| "Invalid day count.")?;
    if std::path::Path::new(&args[1]).exists() {
        return Err("Report output already exists.".into());
    }
    let result = if args.len() == 3 {
        report_with_operator(days, SurfaceOperator::CotangentWeakForm)?
    } else {
        report(days)?
    };
    let file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&args[1])
        .map_err(|e| e.to_string())?;
    let mut file = BufWriter::new(file);
    serde_json::to_writer_pretty(&mut file, &result).map_err(|e| e.to_string())?;
    file.write_all(b"\n").map_err(|e| e.to_string())?;
    file.flush().map_err(|e| e.to_string())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> (World, Checkpoint) {
        fixture_with_operator(SurfaceOperator::BarycentricTwoPoint)
    }
    fn fixture_with_operator(operator: SurfaceOperator) -> (World, Checkpoint) {
        let world = World::generate(recipe("first-light", 2).unwrap()).unwrap();
        let model = Model::from_world(
            &world,
            Settings {
                numerical_policy: NumericalPolicy::RetainDonor,
                surface_operator: operator,
                ..Default::default()
            },
            Default::default(),
            Default::default(),
        )
        .unwrap();
        let mut cp = model.initial_state().checkpoint();
        fund(&world, &mut cp).unwrap();
        (world, cp)
    }
    #[test]
    fn cotangent_branches_share_complete_funding_and_compare_every_owned_reservoir() {
        let (world, mut cp) = fixture_with_operator(SurfaceOperator::CotangentWeakForm);
        // The first main-ocean contact can be cold during the first month.
        // This directed soil assertion explicitly supplies warm forcing; the
        // generated dense cohort continues to use its unchanged default climate.
        cp.temperature_settings = seasonal_temperature::Settings {
            reference_temperature_celsius: 30.,
            sensitivity_celsius_per_watt_per_square_meter: 0.,
            lapse_rate_celsius_per_meter: 0.,
            ..Default::default()
        };
        assert_eq!(cp.schema_version, 2);
        assert_eq!(
            cp.surface_flow_model_version,
            "regional-surface-cotan-paired-2"
        );
        let mut runs = Vec::new();
        for (ceiling, cap) in [(900, 1e6), (450, 1e6), (225, 1e6), (900, 1e5), (900, 0.)] {
            let r = run(&world, &cp, 1, ceiling, cap).unwrap();
            assert!(r.complete);
            assert_eq!(r.report["checks"]["replayExact"], true);
            assert_eq!(r.report["checks"]["surfaceActivityMatchesCap"], true);
            assert_eq!(r.report["checks"]["soilExchangeActive"], true);
            let mut initial = r.initial_checkpoint.clone();
            initial.settings = cp.settings;
            assert_eq!(initial, cp);
            runs.push(r);
        }
        for (a, b, control) in [
            (0, 1, "cadence"),
            (1, 2, "cadence"),
            (0, 3, "mobilityCap"),
            (0, 4, "mobilityCap"),
        ] {
            let result = compare(&runs[a], &runs[b], &world.surface.areas, control).unwrap();
            assert_eq!(result["comparable"], true);
            let bodies = &result["stockDifferences"]["referenceBodies"];
            assert!(bodies.get("l1Kilograms").is_some());
            assert!(bodies.get("maximumColumnDifferenceMillimeters").is_none());
        }
    }
    #[test]
    fn comparison_refuses_different_initial_clock_stocks_histories_and_operator() {
        let (world, cp) = fixture_with_operator(SurfaceOperator::CotangentWeakForm);
        let a = run(&world, &cp, 1, 900, 1e6).unwrap();
        let mut b = run(&world, &cp, 1, 450, 1e6).unwrap();
        let initial = b.initial_checkpoint.clone();
        for variant in 0..5 {
            b.initial_checkpoint = initial.clone();
            match variant {
                0 => b.initial_checkpoint.elapsed_seconds += 900,
                1 => b.initial_checkpoint.liquid[0].high += 1.,
                2 => b.initial_checkpoint.surface_transfers[0].high += 1.,
                3 => b.initial_checkpoint.local_transfers[0].rain.high += 1.,
                _ => b.initial_checkpoint.reference_bodies[0].low += 1.,
            }
            assert!(
                compare(&a, &b, &world.surface.areas, "cadence").is_err(),
                "Accepted initial corruption {variant}"
            );
        }
        b.initial_checkpoint = initial;
        b.final_checkpoint.settings.surface_operator = SurfaceOperator::BarycentricTwoPoint;
        assert!(compare(&a, &b, &world.surface.areas, "cadence").is_err());
    }
    #[test]
    fn body_comparisons_keep_signed_tails_without_fabricated_areas() {
        let a = [Mass {
            high: 2_f64.powi(70),
            low: -0.5,
        }];
        let b = [Mass {
            high: 2_f64.powi(70),
            low: 0.5,
        }];
        assert_eq!(body_difference(&a, &b).unwrap()["l1Kilograms"], 1.);
        assert_eq!(
            body_difference(&a, &[Mass::default()]).unwrap()["relativeToSecondStock"],
            Value::Null
        );
        assert!(body_difference(&a, &[]).is_err());
        assert!(
            body_difference(&a, &b)
                .unwrap()
                .get("maximumColumnDifferenceMillimeters")
                .is_none()
        );
    }
    #[test]
    fn actual_contact_funding_is_finite_and_all_branches_share_the_same_input() {
        let (world, cp) = fixture();
        let (model, state) = Model::restore(cp.clone()).unwrap();
        assert!(model.budget(&state).unwrap().relative_global_residual < 1e-12);
        assert_eq!(cp.elapsed_seconds, START_SECONDS);
        for (r, transfers) in cp.local_transfers.iter().enumerate() {
            if transfers.liquid_evaporation.high > 0. {
                assert_eq!(world.water.body_ids[r], world.water.main_ocean_id);
            }
        }
        for (ceiling, cap) in [(900, 1e6), (450, 1e6), (225, 1e6), (900, 0.)] {
            let result = run(&world, &cp, 1, ceiling, cap).unwrap();
            assert!(result.complete);
            assert_eq!(
                result.final_checkpoint.elapsed_seconds,
                START_SECONDS + 86400
            );
            assert_eq!(result.report["checks"]["replayExact"], true);
            assert_eq!(result.report["checks"]["surfaceActivityMatchesCap"], true);
        }
    }
    #[test]
    fn differences_keep_signed_tails_and_do_not_invent_a_zero_stock_denominator() {
        let a = [Mass {
            high: 2_f64.powi(70),
            low: -0.5,
        }];
        let b = [Mass {
            high: 2_f64.powi(70),
            low: 0.5,
        }];
        let d = field_difference(&a, &b, &[2.]).unwrap();
        assert_eq!(d["l1Kilograms"], 1.);
        assert_eq!(d["maximumColumnDifferenceMillimeters"], 0.5);
        // Different leading components can nearly cancel through their signed
        // tails. Summing componentwise absolute differences would be misleading.
        let a = [Mass {
            high: 2_f64.powi(70),
            low: 131_071.5,
        }];
        let b = [Mass {
            high: 2_f64.powi(70) + 262_144.,
            low: -131_071.5,
        }];
        assert_eq!(field_difference(&a, &b, &[2.]).unwrap()["l1Kilograms"], 1.);
        assert_eq!(
            field_difference(&a, &[Mass::default()], &[2.]).unwrap()["relativeToSecondStock"],
            Value::Null
        );
        assert!(field_difference(&a, &[], &[2.]).is_err());
    }
    #[test]
    fn mismatched_forcing_geography_and_other_controls_cannot_be_compared() {
        let (world, cp) = fixture();
        let a = run(&world, &cp, 1, 900, 1e6).unwrap();
        let mut b = run(&world, &cp, 1, 450, 1e6).unwrap();
        assert_eq!(
            compare(&a, &b, &world.surface.areas, "cadence").unwrap()["comparable"],
            true
        );
        b.final_checkpoint.recipe.seed = "different".into();
        assert!(compare(&a, &b, &world.surface.areas, "cadence").is_err());
        b.final_checkpoint.recipe = a.final_checkpoint.recipe.clone();
        b.final_checkpoint
            .settings
            .surface_flow
            .maximum_diffusivity_square_meters_per_second = 1e5;
        assert!(compare(&a, &b, &world.surface.areas, "cadence").is_err());
        b.final_checkpoint.settings = a.final_checkpoint.settings;
        b.final_checkpoint
            .temperature_settings
            .reference_temperature_celsius += 1.;
        assert!(compare(&a, &b, &world.surface.areas, "cadence").is_err());
    }
    #[test]
    fn failed_or_unequal_time_endpoints_are_not_numerical_comparisons() {
        let (world, cp) = fixture();
        let a = run(&world, &cp, 1, 900, 1e6).unwrap();
        let mut b = run(&world, &cp, 2, 450, 1e6).unwrap();
        assert_eq!(
            compare(&a, &b, &world.surface.areas, "cadence").unwrap()["comparable"],
            false
        );
        b.final_checkpoint.elapsed_seconds = a.final_checkpoint.elapsed_seconds;
        b.complete = false;
        assert_eq!(
            compare(&a, &b, &world.surface.areas, "cadence").unwrap()["comparable"],
            false
        );
        assert!(report(0).is_err());
        assert!(report(366).is_err());
    }
    #[test]
    fn real_work_refusal_is_retained_with_atomic_state_and_exact_refusal_replay() {
        let mut recipe = recipe("first-light", 5).unwrap();
        recipe.radius_meters = 100_000.;
        let world = World::generate(recipe).unwrap();
        let model = Model::from_world(
            &world,
            Settings {
                numerical_policy: NumericalPolicy::RetainDonor,
                routing_enabled: false,
                precipitation_enabled: false,
                evaporation_enabled: false,
                ..Default::default()
            },
            Default::default(),
            Default::default(),
        )
        .unwrap();
        let cp = model.initial_state().checkpoint();
        let result = run(&world, &cp, 1, 900, 1e8).unwrap();
        assert!(!result.complete);
        assert_eq!(result.report["acceptedDays"], 0);
        assert_eq!(result.report["failure"]["firstRefusedDay"], 1);
        assert_eq!(result.report["failure"]["atomicRollback"], true);
        assert!(
            result.report["failure"]["message"]
                .as_str()
                .unwrap()
                .contains("surface flow exceeds")
        );
        assert_eq!(result.report["roundTripCompleteJsonExact"], true);
        assert_eq!(result.report["continuation"]["completeJsonExact"], true);
        let mut expected = cp;
        expected
            .settings
            .surface_flow
            .maximum_diffusivity_square_meters_per_second = 1e8;
        assert_eq!(result.final_checkpoint, expected);
    }
    #[test]
    fn common_directions_do_not_imply_equal_input_geography() {
        let coarse = World::generate(recipe("first-light", 2).unwrap()).unwrap();
        let fine = World::generate(recipe("first-light", 3).unwrap()).unwrap();
        let d = geography_difference(&coarse, &fine).unwrap();
        assert_eq!(d["exactCommonDirections"], true);
        assert!(
            d["areaWeightedMeanAbsoluteBedDifferenceMeters"]
                .as_f64()
                .unwrap()
                > 0.
        );
        assert_eq!(d["spatialConvergenceIsolated"], false);
        assert!(geography_difference(&fine, &coarse).is_err());
    }
}
