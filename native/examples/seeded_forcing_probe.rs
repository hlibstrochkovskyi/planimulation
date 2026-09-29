//! Bounded repeated prescribed forcing on generated water; observational only.
use planimulation_core::{
    Recipe, World,
    nested_reservoir::Input,
    spill_network::simultaneous::multi_entry::seeded::{
        SeededNetwork, UNIT_WEIGHT_POLICY_VERSION, generated_unit_setup,
    },
};
use serde::Deserialize;
use serde_json::{Value, json};
use std::io::{Read, Write};
use std::time::Instant;

const PROBE_VERSION: &str = "cyclic-prescribed-forcing-probe-1";

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Request {
    recipe: Recipe,
    weight_policy_version: String,
    profiles: Vec<Vec<Input>>,
    interval_count: usize,
    /// Serialize and restore after this many accepted intervals, then compare
    /// both continuations exactly at every later interval.
    replay_after: Option<usize>,
}

fn peak_rss_kib() -> Option<u64> {
    std::fs::read_to_string("/proc/self/status")
        .ok()?
        .lines()
        .find_map(|line| {
            line.strip_prefix("VmHWM:")?
                .split_whitespace()
                .next()?
                .parse()
                .ok()
        })
}

fn run(input: impl Read) -> Result<Value, Box<dyn std::error::Error>> {
    let mut bytes = Vec::new();
    input.take(32769).read_to_end(&mut bytes)?;
    if bytes.len() > 32768 {
        return Err("Forcing probe request exceeds 32 KiB.".into());
    }
    let request: Request = serde_json::from_slice(&bytes)?;
    if request.weight_policy_version != UNIT_WEIGHT_POLICY_VERSION
        || request.recipe.subdivision > 5
        || request.interval_count == 0
        || request.interval_count > 1000
        || request.profiles.is_empty()
        || request.profiles.len() > 16
        || request
            .profiles
            .iter()
            .any(|p| p.is_empty() || p.len() > 64)
        || request
            .replay_after
            .is_some_and(|at| at == 0 || at >= request.interval_count)
    {
        return Err("Invalid bounded forcing probe request.".into());
    }
    let started = Instant::now();
    let world = World::generate(request.recipe)?;
    for profile in &request.profiles {
        if profile.iter().any(|input| {
            input.region >= world.surface.areas.len()
                || !input.volume_cubic_meters.is_finite()
                || input.volume_cubic_meters < 0.
        }) {
            return Err("Invalid forcing profile region or volume.".into());
        }
        let mut regions: Vec<_> = profile.iter().map(|input| input.region).collect();
        regions.sort_unstable();
        if regions.windows(2).any(|pair| pair[0] == pair[1]) {
            return Err("Forcing profile contains a duplicate region.".into());
        }
    }
    let mut report = json!({
        "reportVersion": 1,
        "probeVersion": PROBE_VERSION,
        "scope": "Repeated cyclic prescribed input over normalized intervals; no climate, timed discharge, desktop dynamics, or statistical acceptance claim.",
        "recipe": world.recipe,
        "regionCount": world.surface.areas.len(),
        "branchCount": world.basins.nodes().len(),
        "intervalCount": request.interval_count,
        "profilePeriod": request.profiles.len(),
        "profiles": request.profiles,
        "replayAfter": request.replay_after,
        "initialVolumeCubicMeters": world.water.resolved_volume_cubic_meters,
    });
    let setup = match generated_unit_setup(&world) {
        Ok(setup) => setup,
        Err(reason) => {
            report["status"] = json!("rejectedDuringPreparation");
            report["reason"] = json!(reason);
            report["elapsedMilliseconds"] = json!(started.elapsed().as_secs_f64() * 1000.);
            report["peakRssKib"] = json!(peak_rss_kib());
            return Ok(report);
        }
    };
    let mut model = match SeededNetwork::from_generated(&world, setup) {
        Ok(model) => model,
        Err(reason) => {
            report["status"] = json!("rejectedDuringPreparation");
            report["reason"] = json!(reason);
            report["elapsedMilliseconds"] = json!(started.elapsed().as_secs_f64() * 1000.);
            report["peakRssKib"] = json!(peak_rss_kib());
            return Ok(report);
        }
    };
    report["experimentVersion"] = json!(model.experiment_version());
    let initial_snapshot = model.snapshot()?;
    report["initialStoredCubicMeters"] = json!(initial_snapshot.total_stored_cubic_meters);
    let mut maximum_budget_residual = initial_snapshot.budget_residual_cubic_meters.abs();
    let mut replay: Option<SeededNetwork> = None;
    let mut replay_checked = 0usize;
    let mut serialized_bytes = None;
    let mut completed = 0usize;
    let mut event_count = 0usize;
    let mut rejected = None;
    for index in 0..request.interval_count {
        let profile = &request.profiles[index % request.profiles.len()];
        let before = model.checkpoint();
        let outcome = model.add_interval(profile.clone());
        if let Some(restored) = &mut replay {
            let replay_before = restored.checkpoint();
            let continued = restored.add_interval(profile.clone());
            if outcome != continued || model.checkpoint() != restored.checkpoint() {
                return Err("Serialized forcing continuation diverged.".into());
            }
            if outcome.is_err() && restored.checkpoint() != replay_before {
                return Err("Rejected replay interval changed its checkpoint.".into());
            }
            replay_checked += 1;
        }
        match outcome {
            Ok(interval) => {
                completed += 1;
                event_count += interval.events.len();
                maximum_budget_residual = maximum_budget_residual
                    .max(interval.snapshot.budget_residual_cubic_meters.abs());
                if Some(completed) == request.replay_after {
                    let saved = serde_json::to_vec(&model.checkpoint())?;
                    let restored = SeededNetwork::restore(serde_json::from_slice(&saved)?)?;
                    if restored.checkpoint() != model.checkpoint()
                        || restored.snapshot()? != model.snapshot()?
                    {
                        return Err("Serialized forcing checkpoint did not restore exactly.".into());
                    }
                    serialized_bytes = Some(saved.len());
                    replay = Some(restored);
                }
            }
            Err(reason) => {
                if model.checkpoint() != before {
                    return Err("Rejected forcing interval changed its checkpoint.".into());
                }
                rejected = Some((index + 1, reason));
                break;
            }
        }
    }
    let snapshot = model.snapshot()?;
    let checkpoint = model.checkpoint();
    report["status"] = json!(if rejected.is_some() {
        "rejectedDuringInterval"
    } else {
        "accepted"
    });
    if let Some((index, reason)) = rejected {
        report["firstRejectedInterval"] = json!(index);
        report["reason"] = json!(reason);
    }
    report["completedIntervals"] = json!(completed);
    report["replayCheckedIntervals"] = json!(replay_checked);
    report["serializedCheckpointBytes"] = json!(serialized_bytes);
    report["totalEvents"] = json!(event_count);
    report["acceptedExternalInputCubicMeters"] = json!(checkpoint.inventory.input_cubic_meters);
    report["finalStoredCubicMeters"] = json!(snapshot.total_stored_cubic_meters);
    report["budgetResidualCubicMeters"] = json!(snapshot.budget_residual_cubic_meters);
    report["maximumAbsoluteBudgetResidualCubicMeters"] = json!(maximum_budget_residual);
    report["elapsedMilliseconds"] = json!(started.elapsed().as_secs_f64() * 1000.);
    report["peakRssKib"] = json!(peak_rss_kib());
    Ok(report)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() != 1 {
        return Err("Usage: seeded_forcing_probe <experiment.json | ->".into());
    }
    let input: Box<dyn Read> = if args[0] == "-" {
        Box::new(std::io::stdin())
    } else {
        Box::new(std::fs::File::open(&args[0])?)
    };
    let report = run(input)?;
    let mut out = std::io::stdout().lock();
    serde_json::to_writer_pretty(&mut out, &report)?;
    writeln!(out)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    const FIXTURE: &str = include_str!("../../docs/scenarios/seeded-forcing-probe.json");
    const SMALL_INPUT: &str = include_str!("../../docs/scenarios/seeded-forcing-small-input.json");

    #[test]
    fn probe_reports_replay_and_atomic_precision_failure() {
        let mut small: Value = serde_json::from_str(FIXTURE).unwrap();
        small["recipe"]["subdivision"] = json!(2);
        small["intervalCount"] = json!(8);
        small["replayAfter"] = json!(4);
        let accepted = run(serde_json::to_vec(&small).unwrap().as_slice()).unwrap();
        assert_eq!(accepted["status"], "accepted");
        assert_eq!(accepted["completedIntervals"], 8);
        assert_eq!(accepted["replayCheckedIntervals"], 4);
        assert!(accepted["serializedCheckpointBytes"].as_u64().unwrap() > 0);
        let mut tiny = small;
        tiny["intervalCount"] = json!(1);
        tiny["replayAfter"] = Value::Null;
        tiny["profiles"] = json!([[{"region":3,"volumeCubicMeters":0.0001}]]);
        let rejected = run(serde_json::to_vec(&tiny).unwrap().as_slice()).unwrap();
        assert_eq!(rejected["status"], "rejectedDuringInterval");
        assert_eq!(rejected["firstRejectedInterval"], 1);
        assert_eq!(rejected["completedIntervals"], 0);
        assert_eq!(rejected["acceptedExternalInputCubicMeters"], 0.);
        assert_eq!(
            rejected["finalStoredCubicMeters"],
            rejected["initialStoredCubicMeters"]
        );
    }

    #[test]
    fn default_world_replays_and_measured_event_roundoff_is_retained() {
        let accepted = run(FIXTURE.as_bytes()).unwrap();
        assert_eq!(accepted["status"], "accepted");
        assert_eq!(
            accepted["experimentVersion"],
            "seeded-multi-entry-network-4"
        );
        assert_eq!(accepted["completedIntervals"], 100);
        assert_eq!(accepted["replayCheckedIntervals"], 50);
        assert_eq!(accepted["acceptedExternalInputCubicMeters"], 4e14);
        let mut witness: Value = serde_json::from_str(FIXTURE).unwrap();
        witness["recipe"]["seed"] = json!("profile-15");
        witness["intervalCount"] = json!(2);
        witness["replayAfter"] = Value::Null;
        let rejected = run(serde_json::to_vec(&witness).unwrap().as_slice()).unwrap();
        assert_eq!(rejected["status"], "rejectedDuringInterval");
        assert_eq!(rejected["completedIntervals"], 1);
        assert_eq!(rejected["firstRejectedInterval"], 2);
        assert_eq!(
            rejected["reason"],
            "Concurrent event budget does not balance."
        );
        assert_eq!(rejected["acceptedExternalInputCubicMeters"], 4e12);
        let mut small: Value = serde_json::from_str(SMALL_INPUT).unwrap();
        small["recipe"]["seed"] = json!("profile-03");
        small["intervalCount"] = json!(1);
        small["replayAfter"] = Value::Null;
        let level_failure = run(serde_json::to_vec(&small).unwrap().as_slice()).unwrap();
        assert_eq!(level_failure["status"], "rejectedDuringInterval");
        assert_eq!(level_failure["firstRejectedInterval"], 1);
        assert_eq!(level_failure["completedIntervals"], 0);
        assert_eq!(
            level_failure["reason"],
            "Shared level cannot represent the requested storage precisely."
        );
        assert_eq!(level_failure["acceptedExternalInputCubicMeters"], 0.);
    }

    #[test]
    fn invalid_requests_fail_before_a_report() {
        assert!(run(&b"{}"[..]).is_err());
        assert!(run(vec![b' '; 32769].as_slice()).is_err());
        let mut invalid: Value = serde_json::from_str(FIXTURE).unwrap();
        invalid["intervalCount"] = json!(1001);
        assert!(run(serde_json::to_vec(&invalid).unwrap().as_slice()).is_err());
        invalid["intervalCount"] = json!(2);
        invalid["replayAfter"] = json!(2);
        assert!(run(serde_json::to_vec(&invalid).unwrap().as_slice()).is_err());
        invalid["replayAfter"] = Value::Null;
        invalid["profiles"] = json!([]);
        assert!(run(serde_json::to_vec(&invalid).unwrap().as_slice()).is_err());
        invalid["profiles"] = json!([[{"region":3,"volumeCubicMeters":-1.}]]);
        assert!(run(serde_json::to_vec(&invalid).unwrap().as_slice()).is_err());
        invalid["profiles"] = json!([
            [{"region":3,"volumeCubicMeters":1.}, {"region":3,"volumeCubicMeters":2.}]
        ]);
        assert!(run(serde_json::to_vec(&invalid).unwrap().as_slice()).is_err());
    }
}
