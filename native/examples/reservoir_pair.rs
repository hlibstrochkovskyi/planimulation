//! Bounded two-bowl developer experiment, not a desktop command.
use planimulation_core::reservoir_pair::{Checkpoint, ReservoirPair, Volumes};
use serde::Deserialize;
use serde_json::{Value, json};
use std::io::{Read, Write};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Request {
    checkpoint: Checkpoint,
    inputs_cubic_meters: Vec<Volumes>,
}

fn run(input: impl Read) -> Result<Value, Box<dyn std::error::Error>> {
    let mut bytes = Vec::new();
    input.take(32769).read_to_end(&mut bytes)?;
    if bytes.len() > 32768 {
        return Err("Pair request exceeds 32 KiB.".into());
    }
    let request: Request = serde_json::from_slice(&bytes)?;
    if request.inputs_cubic_meters.len() > 1024 {
        return Err("At most 1024 pair pulses are allowed.".into());
    }
    let mut pair = ReservoirPair::restore(request.checkpoint)?;
    let initial = pair.checkpoint();
    let initial_snapshot = pair.snapshot()?;
    let mut pulses = Vec::new();
    for input in request.inputs_cubic_meters {
        pulses.push(pair.add_input(input)?);
    }
    Ok(json!({ "reportVersion": 1,
        "scope": "Closed two-bowl one-sill equilibrium experiment; prescribed volume pulses, not timed discharge or planetary routing.",
        "initialCheckpoint": initial, "initialSnapshot": initial_snapshot,
        "capacitiesCubicMeters": pair.capacities_cubic_meters(),
        "pulses": pulses, "finalCheckpoint": pair.checkpoint() }))
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() != 1 {
        return Err("Usage: reservoir_pair <experiment.json | ->".into());
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
    const FIXTURE: &str = include_str!("../../docs/scenarios/reservoir-pair.json");
    #[test]
    fn fixture_reports_independent_fill_transfer_threshold_and_merge() {
        let report = run(FIXTURE.as_bytes()).unwrap();
        assert_eq!(report, run(FIXTURE.as_bytes()).unwrap());
        assert_eq!(report["capacitiesCubicMeters"], json!([12., 16.]));
        assert_eq!(report["pulses"][1]["leftToRightCubicMeters"], 4.);
        assert_eq!(report["pulses"][2]["snapshot"]["phase"], "atSill");
        assert_eq!(
            report["pulses"][3]["snapshot"]["levelsMeters"],
            json!([4., 4.])
        );
        assert_eq!(
            report["finalCheckpoint"]["inventory"]["storage"],
            json!({"connected": {"volume": 38.}})
        );
    }
    #[test]
    fn report_checkpoint_resumes_and_matches_uninterrupted_run() {
        let report = run(FIXTURE.as_bytes()).unwrap();
        let resumed = json!({"checkpoint": report["finalCheckpoint"], "inputsCubicMeters": [{"left": 22., "right": 0.}]});
        let resumed = run(serde_json::to_vec(&resumed).unwrap().as_slice()).unwrap();
        let mut full: Value = serde_json::from_str(FIXTURE).unwrap();
        full["inputsCubicMeters"]
            .as_array_mut()
            .unwrap()
            .push(json!({"left": 22., "right": 0.}));
        let full = run(serde_json::to_vec(&full).unwrap().as_slice()).unwrap();
        assert_eq!(resumed["finalCheckpoint"], full["finalCheckpoint"]);
    }
    #[test]
    fn malformed_and_excessive_requests_never_return_a_partial_report() {
        assert!(run(&b"{}"[..]).is_err());
        assert!(run(vec![b' '; 32769].as_slice()).is_err());
        let good: Value = serde_json::from_str(FIXTURE).unwrap();
        let mut bad = good.clone();
        bad["checkpoint"]["experimentVersion"] = "unknown".into();
        assert!(run(serde_json::to_vec(&bad).unwrap().as_slice()).is_err());
        for inputs in [
            json!([{"left": 1., "right": 0.}, {"left": -1., "right": 0.}]),
            json!([{"left": 1.}]),
            json!(vec![json!({"left": 0., "right": 0.}); 1025]),
        ] {
            let mut bad = good.clone();
            bad["inputsCubicMeters"] = inputs;
            assert!(run(serde_json::to_vec(&bad).unwrap().as_slice()).is_err());
        }
        let mut bad = good;
        bad["unknown"] = true.into();
        assert!(run(serde_json::to_vec(&bad).unwrap().as_slice()).is_err());
    }
}
