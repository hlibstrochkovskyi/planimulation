//! Bounded ordered spill-network experiment; no desktop/world mutation.
use planimulation_core::nested_reservoir::Input;
use planimulation_core::spill_network::{Checkpoint, Setup, SpillNetwork};
use serde::Deserialize;
use serde_json::{Value, json};
use std::io::{Read, Write};

#[derive(Deserialize)]
// Externally tagged variants preserve the raw decimal readers in checkpoints.
#[serde(rename_all = "camelCase", deny_unknown_fields)]
enum Start {
    Dry(Setup),
    Checkpoint(Checkpoint),
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Request {
    start: Start,
    inputs: Vec<Input>,
}

fn run(input: impl Read) -> Result<Value, Box<dyn std::error::Error>> {
    let mut bytes = Vec::new();
    input.take(32769).read_to_end(&mut bytes)?;
    if bytes.len() > 32768 {
        return Err("Network request exceeds 32 KiB.".into());
    }
    let request: Request = serde_json::from_slice(&bytes)?;
    if request.inputs.len() > 1024 {
        return Err("At most 1024 network pulses are allowed.".into());
    }
    let mut experiment = match request.start {
        Start::Dry(geometry) => SpillNetwork::new(geometry)?,
        Start::Checkpoint(checkpoint) => SpillNetwork::restore(checkpoint)?,
    };
    let initial = experiment.checkpoint();
    let initial_snapshot = experiment.snapshot()?;
    let mut pulses = Vec::new();
    for input in request.inputs {
        pulses.push(experiment.add_input(input)?);
    }
    Ok(json!({ "reportVersion": 1,
        "scope": "Closed <=128-region event-driven receiving-frontier experiment; ordered volume pulses, not simultaneous rainfall or timed hydraulics.",
        "initialCheckpoint": initial, "initialSnapshot": initial_snapshot,
        "analysis": experiment.connections(),
        "pulses": pulses, "finalCheckpoint": experiment.checkpoint() }))
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() != 1 {
        return Err("Usage: spill_network <experiment.json | ->".into());
    }
    let input: Box<dyn Read> = if args[0] == "-" {
        Box::new(std::io::stdin())
    } else {
        Box::new(std::fs::File::open(&args[0])?)
    };
    // Do not publish a partial report if any later pulse or restoration fails.
    let report = run(input)?;
    let mut out = std::io::stdout().lock();
    serde_json::to_writer_pretty(&mut out, &report)?;
    writeln!(out)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    const FIXTURE: &str = include_str!("../../docs/scenarios/spill-network.json");
    #[test]
    fn fixture_combines_nested_entry_and_distinct_sill_transit() {
        let report = run(FIXTURE.as_bytes()).unwrap();
        assert_eq!(report, run(FIXTURE.as_bytes()).unwrap());
        let pulses = report["pulses"].as_array().unwrap();
        assert_eq!(
            pulses
                .iter()
                .map(|p| p["snapshot"]["active"].as_array().unwrap().len())
                .collect::<Vec<_>>(),
            [5, 5, 5, 5, 4, 1, 1]
        );
        assert_eq!(pulses[6]["snapshot"]["totalStoredCubicMeters"], 30.5);
        assert_eq!(pulses[6]["snapshot"]["active"][0]["waterLevelMeters"], 6.);
        assert_eq!(
            report["finalCheckpoint"]["setup"]["policyVersion"],
            "frontier-weighted-events-1"
        );
    }
    #[test]
    fn report_checkpoint_continues_exactly() {
        let report = run(FIXTURE.as_bytes()).unwrap();
        let input = json!({"region":0,"volumeCubicMeters":9.});
        let request =
            json!({"start":{"checkpoint":report["finalCheckpoint"]},"inputs":[input.clone()]});
        let resumed = run(serde_json::to_vec(&request).unwrap().as_slice()).unwrap();
        let mut full: Value = serde_json::from_str(FIXTURE).unwrap();
        full["inputs"].as_array_mut().unwrap().push(input);
        let full = run(serde_json::to_vec(&full).unwrap().as_slice()).unwrap();
        assert_eq!(resumed["finalCheckpoint"], full["finalCheckpoint"]);
        assert_eq!(
            resumed["pulses"][0]["snapshot"]["totalStoredCubicMeters"],
            39.5
        );
    }
    #[test]
    fn bad_requests_limits_and_late_failures_return_no_report() {
        assert!(run(&b"{}"[..]).is_err());
        assert!(run(vec![b' '; 32769].as_slice()).is_err());
        let good: Value = serde_json::from_str(FIXTURE).unwrap();
        let mut bad = good.clone();
        bad["unknown"] = true.into();
        assert!(run(serde_json::to_vec(&bad).unwrap().as_slice()).is_err());
        let mut bad = good.clone();
        bad["start"]["dry"]["policyVersion"] = "future".into();
        assert!(run(serde_json::to_vec(&bad).unwrap().as_slice()).is_err());
        let mut bad = good.clone();
        bad["inputs"][6]["volumeCubicMeters"] = (-1.).into();
        assert!(run(serde_json::to_vec(&bad).unwrap().as_slice()).is_err());
        let mut bad = good.clone();
        bad["inputs"] = json!(vec![json!({"region":0,"volumeCubicMeters":0}); 1025]);
        assert!(run(serde_json::to_vec(&bad).unwrap().as_slice()).is_err());
        let mut bad = good;
        bad["start"]["dry"]["weights"][0]["weight"] = 0.into();
        assert!(run(serde_json::to_vec(&bad).unwrap().as_slice()).is_err());
    }
}
