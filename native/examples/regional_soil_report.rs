//! Recorded seasonal qualification, including every refusal and whole-state rollback.
use planimulation_core::{
    Recipe, World,
    moisture_transport::{Geometry, total_mass},
    seasonal_moisture::regional_soil::{
        COTANGENT_MODEL_VERSION, Checkpoint, MODEL_VERSION, Model, NumericalPolicy, Settings,
        SurfaceOperator,
    },
    surface_water::ponded_soil::Mass,
};
use serde_json::{Value, json};
use std::io::{BufWriter, Write};

fn add(m: &mut Mass, amount: f64) {
    let sum = m.high + amount;
    let v = sum - m.high;
    let tail = m.low + ((m.high - (sum - v)) + (amount - v));
    let high = sum + tail;
    let v = high - sum;
    m.high = high;
    m.low = (sum - (high - v)) + (tail - v);
}
// Finite-funded synthetic history through one actual adjacent atmospheric contact.
// This does not assert that the supplied history arose under the prescribed winds.
fn fund(world: &World, cp: &mut Checkpoint) -> Result<usize, String> {
    let geometry = Geometry::from_surface(&world.surface, world.recipe.radius_meters)?;
    let (face, source, target) = geometry
        .boundaries()
        .iter()
        .enumerate()
        .find_map(|(f, b)| {
            let [a, c] = b.regions;
            if world.water.body_ids[a] > 0 && world.water.body_ids[c] == 0 {
                Some((f, a, c))
            } else if world.water.body_ids[c] > 0 && world.water.body_ids[a] == 0 {
                Some((f, c, a))
            } else {
                None
            }
        })
        .ok_or("No wet/land face for synthetic funding.")?;
    let mut ids = world.water.body_ids.clone();
    ids.sort_unstable();
    ids.dedup();
    ids.retain(|&id| id > 0);
    let body = ids.binary_search(&world.water.body_ids[source]).unwrap();
    let amount = world.surface.areas[target] * 50_000.;
    if amount >= cp.reference_bodies[body].high {
        return Err("Reference body cannot fund the directed input.".into());
    }
    add(&mut cp.reference_bodies[body], -amount);
    add(&mut cp.local_transfers[source].liquid_evaporation, amount);
    add(
        &mut cp.atmospheric_transfers[2 * face + usize::from(source > target)],
        amount,
    );
    add(&mut cp.local_transfers[target].rain, amount);
    add(&mut cp.liquid[target], amount);
    cp.elapsed_seconds = 900;
    Ok(target)
}
fn sum(masses: &[Mass]) -> f64 {
    total_mass(
        &masses
            .iter()
            .flat_map(|v| [v.high, v.low])
            .collect::<Vec<_>>(),
    )
}
fn run(
    seed: &str,
    coverage: f64,
    subdivision: u32,
    limit: u32,
    funded: bool,
    days: u32,
    settings: Settings,
) -> Result<Value, String> {
    let mut recipe: Recipe =
        serde_json::from_str(include_str!("../../docs/scenarios/spill-connections.json"))
            .map_err(|e| e.to_string())?;
    recipe.seed = seed.into();
    recipe.subdivision = subdivision;
    recipe.water = planimulation_core::water::WaterSettings::Coverage { fraction: coverage };
    let world = World::generate(recipe)?;
    let settings = Settings {
        max_coupled_step_seconds: limit,
        ..settings
    };
    let initial_model =
        Model::from_world(&world, settings, Default::default(), Default::default())?;
    let mut checkpoint = initial_model.initial_state().checkpoint();
    let source = if funded {
        Some(fund(&world, &mut checkpoint)?)
    } else {
        None
    };
    let (model, mut state) = Model::restore(checkpoint.clone())?;
    let mut failure = Value::Null;
    let (mut global, mut local): (f64, f64) = (0., 0.);
    for day in 1..=days {
        let before = state.clone();
        match model.advance(&mut state, 86400) {
            Ok(step) => {
                global = global.max(step.budget.relative_global_residual);
                local = local.max(step.budget.maximum_relative_local_residual);
            }
            Err(message) => {
                failure = json!({"day":day,"message":message,"atomicRollback":state == before});
                break;
            }
        }
    }
    let final_cp = state.checkpoint();
    let serialized = serde_json::to_string(&final_cp).map_err(|e| e.to_string())?;
    let (resumed, mut restored) =
        Model::restore(serde_json::from_str(&serialized).map_err(|e| e.to_string())?)?;
    let round_trip = restored == state;
    let mut direct = state.clone();
    let continuation = match (
        model.advance(&mut direct, 3600),
        resumed.advance(&mut restored, 3600),
    ) {
        (Ok(_), Ok(_)) => json!({"accepted":true,"exact":direct == restored}),
        (Err(a), Err(b)) => {
            json!({"accepted":false,"exact":a == b && direct == restored && direct == state,"message":a})
        }
        _ => json!({"accepted":false,"exact":false,"message":"Replay acceptance mismatch."}),
    };
    Ok(
        json!({"seed":seed,"coverage":coverage,"regions":world.surface.areas.len(),"coupledCeilingSeconds":limit,
        "fundedRegion":source,"fundedKilograms":source.map(|r| 50_000. * world.surface.areas[r]),
        "requestedDays":days,"failure":failure,"maximumRelativeGlobalResidual":global,"maximumRelativeLocalResidual":local,
        "roundTripExact":round_trip,"continuation":continuation,
        "activeSurfaceDirections":final_cp.surface_transfers.iter().filter(|v| v.high > 0.).count(),
        "surfaceCrossingsKilograms":sum(&final_cp.surface_transfers),"drainageCrossingsKilograms":sum(&final_cp.drainage_sent),
        "liquidKilograms":sum(&final_cp.liquid),"soilKilograms":sum(&final_cp.soil),"vaporKilograms":sum(&final_cp.vapor),
        "finalCheckpoint":final_cp}),
    )
}
fn main() -> Result<(), String> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if !(2..=4).contains(&args.len())
        || args[2..]
            .iter()
            .any(|s| s != "--retain-donor" && s != "--cotangent")
        || (args.len() == 4 && args[2] == args[3])
    {
        return Err(
            "Usage: regional_soil_report DAYS NEW_OUTPUT.json [--retain-donor] [--cotangent]"
                .into(),
        );
    }
    let days: u32 = args[0].parse().map_err(|_| "Invalid day count.")?;
    if !(1..=3650).contains(&days) {
        return Err("Report supports 1–3650 days.".into());
    }
    if std::path::Path::new(&args[1]).exists() {
        return Err("Report output already exists.".into());
    }
    let policy = if args[2..].iter().any(|s| s == "--retain-donor") {
        NumericalPolicy::RetainDonor
    } else {
        NumericalPolicy::RejectUnrepresentable
    };
    let operator = if args[2..].iter().any(|s| s == "--cotangent") {
        SurfaceOperator::CotangentWeakForm
    } else {
        SurfaceOperator::BarycentricTwoPoint
    };
    let mut cases = Vec::new();
    let settings = Settings {
        numerical_policy: policy,
        surface_operator: operator,
        ..Default::default()
    };
    for seed in ["first-light", "readiness-01", "receiver-2", "readiness-03"] {
        for coverage in [0.71, 0.3] {
            cases.push(run(seed, coverage, 2, 900, false, days, settings)?);
        }
    }
    cases.push(run("first-light", 0.3, 3, 900, false, days, settings)?);
    for limit in [900, 450, 225] {
        cases.push(run("first-light", 0.3, 2, limit, true, days, settings)?);
    }
    let report = json!({"reportVersion":if operator == SurfaceOperator::CotangentWeakForm {"regional-soil-cotan-report-1"} else {"regional-soil-report-1"},
        "modelVersion":if operator == SurfaceOperator::CotangentWeakForm {COTANGENT_MODEL_VERSION} else {MODEL_VERSION},
        "numericalPolicy":policy,"cases":cases});
    let file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&args[1])
        .map_err(|e| e.to_string())?;
    let mut file = BufWriter::new(file);
    serde_json::to_writer_pretty(&mut file, &report).map_err(|e| e.to_string())?;
    file.write_all(b"\n").map_err(|e| e.to_string())?;
    file.flush().map_err(|e| e.to_string())?;
    Ok(())
}
