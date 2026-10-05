//! Fixed annual qualification cohort. Failures are evidence, never filtered out.
use planimulation_core::{
    Recipe, World,
    seasonal_moisture::{
        ClosedLakeExchange, Model, ReferenceWaterPool, Settings, SoilNumerics, SurfaceNumerics,
        TerminalNumerics, surface_flow,
    },
};
use serde_json::{Value, json};
use std::io::Write;

#[derive(Clone, Copy)]
struct Case<'a> {
    seed: &'a str,
    coverage: f64,
    relief: f64,
    subdivision: u32,
    limit: u32,
    diffusivity: f64,
    inputs: &'a [(usize, f64)],
}
impl<'a> Case<'a> {
    fn new(seed: &'a str, coverage: f64) -> Self {
        Self {
            seed,
            coverage,
            relief: 1.,
            subdivision: 2,
            limit: 900,
            diffusivity: 1e6,
            inputs: &[],
        }
    }
}
fn credit(high: &mut f64, low: &mut f64, value: f64) {
    let sum = *high + value;
    let virtual_value = sum - *high;
    let error = (*high - (sum - virtual_value)) + (value - virtual_value);
    let tail = *low + error;
    let h = sum + tail;
    let virtual_tail = h - sum;
    *high = h;
    *low = (sum - (h - virtual_tail)) + (tail - virtual_tail);
}
// Explicit synthetic rain, not spontaneous weather. Every kilogram is debited
// from a real finite body and carries matching contact and regional histories.
fn fund(
    cp: &mut planimulation_core::seasonal_moisture::Checkpoint,
    world: &World,
    inputs: &[(usize, f64)],
) -> Result<(), String> {
    if inputs.is_empty() {
        return Ok(());
    }
    cp.elapsed_seconds = 900;
    let mut ids = world.water.body_ids.clone();
    ids.sort_unstable();
    ids.dedup();
    ids.retain(|&id| id > 0);
    let mut used = vec![false; world.surface.areas.len()];
    for &(r, amount) in inputs {
        if world.water.body_ids.get(r) != Some(&0) || !amount.is_finite() || amount <= 0. {
            return Err("Invalid funded regional rain.".into());
        }
        let body = cp
            .reference_body_high_kilograms
            .as_ref()
            .unwrap()
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.total_cmp(b.1))
            .ok_or("No finite body to fund rain.")?
            .0;
        let contact = world
            .water
            .body_ids
            .iter()
            .enumerate()
            .find_map(|(i, &id)| (id == ids[body] && !used[i]).then_some(i))
            .ok_or("No unused physical funding contact.")?;
        used[contact] = true;
        if cp.reference_body_high_kilograms.as_ref().unwrap()[body] <= amount {
            return Err("Funded rain would exhaust the reference body.".into());
        }
        credit(
            &mut cp.reference_body_high_kilograms.as_mut().unwrap()[body],
            &mut cp.reference_body_low_kilograms.as_mut().unwrap()[body],
            -amount,
        );
        cp.cumulative_surface_transfers[contact].liquid_evaporation += amount;
        cp.cumulative_evaporation_kilograms[contact] += amount;
        cp.cumulative_surface_transfers[r].rain += amount;
        cp.cumulative_precipitation_kilograms[r] += amount;
        credit(
            &mut cp.cumulative_lake_capture_kilograms.as_mut().unwrap()[r],
            &mut cp.cumulative_lake_capture_low_kilograms.as_mut().unwrap()[r],
            amount,
        );
        credit(
            &mut cp.terminal_water_kilograms[r],
            &mut cp.terminal_low_kilograms.as_mut().unwrap()[r],
            amount,
        );
    }
    Ok(())
}
fn configuration(limit: u32, diffusivity: f64) -> Settings {
    Settings {
        initial_active_surface_depth_meters: 10.,
        orography: Some(Default::default()),
        soil_numerics: Some(SoilNumerics::Compensated),
        surface_numerics: Some(SurfaceNumerics::Compensated),
        terminal_numerics: Some(TerminalNumerics::Compensated),
        reference_water_pool: Some(ReferenceWaterPool::FastConnectedBody),
        closed_lake_exchange: Some(ClosedLakeExchange::FrozenRegionalSurfaceFlow(
            surface_flow::Settings {
                maximum_diffusivity_square_meters_per_second: diffusivity,
                ..Default::default()
            },
        )),
        max_coupled_step_seconds: limit,
        ..Default::default()
    }
}
fn run(case: Case<'_>, days: u32) -> Result<Value, String> {
    let Case {
        seed,
        coverage,
        relief,
        subdivision,
        limit,
        diffusivity,
        inputs,
    } = case;
    let mut r: Recipe =
        serde_json::from_str(include_str!("../../docs/scenarios/spill-connections.json")).unwrap();
    r.seed = seed.into();
    r.water = planimulation_core::water::WaterSettings::Coverage { fraction: coverage };
    r.relief_scale = relief;
    r.detail_amplitude_meters = 300. * relief;
    r.subdivision = subdivision;
    let world = World::generate(r)?;
    let configuration = configuration(limit, diffusivity);
    let m = Model::from_world(
        &world,
        configuration,
        Default::default(),
        Default::default(),
    )?;
    let mut initial = m.initial_state().checkpoint();
    fund(&mut initial, &world, inputs)?;
    let (m, mut s) = Model::restore(initial)?;
    let mut accepted = 0;
    let mut failure = None;
    let mut flow = surface_flow::FlowBudget::default();
    let mut global: f64 = 0.;
    let mut local: f64 = 0.;
    for day in 0..days {
        let before = s.clone();
        match m.advance(&mut s, 86400) {
            Ok(step) => {
                let f = step.regional_surface_flow.unwrap();
                flow.substeps += f.substeps;
                flow.transferred_kilograms += f.transferred_kilograms;
                flow.deferred_requests += f.deferred_requests;
                flow.deferred_request_kilograms += f.deferred_request_kilograms;
                flow.maximum_deferred_request_kilograms = flow
                    .maximum_deferred_request_kilograms
                    .max(f.maximum_deferred_request_kilograms);
                flow.deferred_evaporation_requests += f.deferred_evaporation_requests;
                flow.deferred_evaporation_request_kilograms +=
                    f.deferred_evaporation_request_kilograms;
                flow.maximum_deferred_evaporation_kilograms = flow
                    .maximum_deferred_evaporation_kilograms
                    .max(f.maximum_deferred_evaporation_kilograms);
                global = global.max(
                    step.budget.residual_kilograms.abs()
                        / step.budget.initial_mobile_water_kilograms.max(1.),
                );
                local = local.max(step.budget.maximum_relative_local_surface_ledger_residual);
                accepted += 1;
            }
            Err(error) => {
                failure =
                    Some(json!({"day":day+1,"message":error,"completeStateUnchanged":before==s}));
                break;
            }
        }
    }
    let cp = s.checkpoint();
    let view = m.regional_surface_observation(&s)?;
    let (restored, mut resumed) =
        Model::restore(serde_json::from_str(&serde_json::to_string(&cp).unwrap()).unwrap())?;
    let round_trip = s == resumed;
    let before_continuation = s.clone();
    let first = m.advance(&mut s, 3600);
    let second = restored.advance(&mut resumed, 3600);
    let replay = json!({"fullCheckpointRoundTripExact":round_trip,"nextHourContinuationExact":s==resumed,
        "continuationError":first.as_ref().err(),"restoredContinuationError":second.as_ref().err(),
        "refusedContinuationStateUnchanged":first.is_err().then_some(s==before_continuation)});
    let active_faces = cp
        .regional_surface_flow
        .as_ref()
        .unwrap()
        .directed_transfers
        .high_kilograms
        .iter()
        .filter(|&&v| v > 0.)
        .count();
    eprintln!(
        "{seed}: coverage={coverage}, relief={relief}, regions={}, step={limit}, diffusivity={diffusivity}: {accepted}/{days} days, {active_faces} directed contacts, failure={failure:?}",
        world.surface.areas.len()
    );
    Ok(
        json!({"recipe":world.recipe,"settings":configuration,"regions":world.surface.areas.len(),"requestedDays":days,
        "initialFundedRain":inputs,"acceptedDays":accepted,"failure":failure,"flow":flow,"activeDirectedContacts":active_faces,
        "maximumRelativeGlobalResidual":global,"maximumRelativeLocalResidual":local,
        "finalBudget":m.budget(&before_continuation)?,"regionalHighKilograms":cp.terminal_water_kilograms,
        "regionalLowKilograms":cp.terminal_low_kilograms,"regionalDepthMeters":view.depth_meters,
        "explicitStabilityBoundSeconds":view.explicit_stability_bound_seconds,"replay":replay}),
    )
}
fn main() -> Result<(), String> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() != 4 || args[0] != "--days" || args[2] != "--output" {
        return Err("Usage: regional_surface_flow_report --days 1..365 --output NEW_PATH".into());
    }
    let days: u32 = args[1].parse().map_err(|_| "Invalid report day count.")?;
    if !(1..=365).contains(&days) {
        return Err("Report day count must be 1..365.".into());
    }
    // Refuse overwrites before spending computation. Write only the complete report.
    if std::path::Path::new(&args[3]).exists() {
        return Err("Report target already exists.".into());
    }
    let seeds = ["first-light", "readiness-00", "readiness-01", "receiver-2"];
    let mut cases = Vec::new();
    for seed in seeds {
        for coverage in [0.71, 0.3, 0.1] {
            cases.push(run(Case::new(seed, coverage), days)?);
        }
    }
    for seed in seeds {
        cases.push(run(
            Case {
                relief: 0.01,
                ..Case::new(seed, 0.3)
            },
            days,
        )?);
    }
    for diffusivity in [0., 1e5] {
        cases.push(run(
            Case {
                relief: 0.01,
                diffusivity,
                ..Case::new("first-light", 0.3)
            },
            days,
        )?);
    }
    cases.push(run(
        Case {
            relief: 0.01,
            limit: 450,
            ..Case::new("first-light", 0.3)
        },
        days,
    )?);
    cases.push(run(
        Case {
            subdivision: 3,
            ..Case::new("first-light", 0.71)
        },
        days,
    )?);
    let inputs = [(8, 1.4e17), (156, 2.4e17)];
    for limit in [900, 450, 225] {
        cases.push(run(
            Case {
                limit,
                inputs: &inputs,
                ..Case::new("first-light", 0.3)
            },
            days,
        )?);
    }
    for diffusivity in [0., 1e5] {
        cases.push(run(
            Case {
                diffusivity,
                inputs: &inputs,
                ..Case::new("first-light", 0.3)
            },
            days,
        )?);
    }
    let completed = cases
        .iter()
        .filter(|c| c["acceptedDays"] == days && c["failure"].is_null())
        .count();
    let report = json!({"reportVersion":"regional-surface-flow-qualification-1","modelVersion":"seasonal-moisture-15",
        "scope":"Fixed regional finite-volume seasonal cohort, prescribed finite reference-body levels, capped Manning-inspired mobility. Not instantaneous basin merging, a calibrated climate or hydrodynamic forecast, or desktop promotion.",
        "caseCount":cases.len(),"completedCases":completed,"cases":cases});
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&args[3])
        .map_err(|e| e.to_string())?;
    file.write_all(serde_json::to_string_pretty(&report).unwrap().as_bytes())
        .map_err(|e| e.to_string())?;
    file.write_all(b"\n").map_err(|e| e.to_string())?;
    Ok(())
}
