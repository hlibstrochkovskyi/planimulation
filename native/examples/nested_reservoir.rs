//! Bounded closed binary-hierarchy experiment; no desktop/world mutation.
use planimulation_core::nested_reservoir::{Checkpoint, Geometry, Input, NestedReservoir};
use serde::Deserialize;
use serde_json::{Value, json};
use std::io::{Read, Write};

#[derive(Deserialize)]
// Externally tagged variants preserve the raw decimal readers in checkpoints.
#[serde(rename_all = "camelCase", deny_unknown_fields)]
enum Start {
    Dry(Geometry),
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
        return Err("Nested request exceeds 32 KiB.".into());
    }
    let request: Request = serde_json::from_slice(&bytes)?;
    if request.inputs.len() > 1024 {
        return Err("At most 1024 nested pulses are allowed.".into());
    }
    let mut experiment = match request.start {
        Start::Dry(geometry) => NestedReservoir::new(geometry)?,
        Start::Checkpoint(checkpoint) => NestedReservoir::restore(checkpoint)?,
    };
    let initial = experiment.checkpoint();
    let initial_snapshot = experiment.snapshot()?;
    let mut pulses = Vec::new();
    for input in request.inputs {
        pulses.push(experiment.add_input(input)?);
    }
    Ok(json!({ "reportVersion": 1,
        "scope": "Closed <=128-region binary-hierarchy experiment with unique sill contacts; ordered volume pulses, not timed discharge or planetary hydrology.",
        "initialCheckpoint": initial, "initialSnapshot": initial_snapshot,
        "analysis": experiment.connections(), "routes": experiment.routes(),
        "pulses": pulses, "finalCheckpoint": experiment.checkpoint() }))
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() != 1 {
        return Err("Usage: nested_reservoir <experiment.json | ->".into());
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
    const FIXTURE: &str = include_str!("../../docs/scenarios/nested-reservoir.json");

    #[test]
    fn fixture_fills_near_child_then_merges_inner_and_outer_reservoirs() {
        let report = run(FIXTURE.as_bytes()).unwrap();
        assert_eq!(report, run(FIXTURE.as_bytes()).unwrap());
        let pulses = report["pulses"].as_array().unwrap();
        assert_eq!(
            pulses
                .iter()
                .map(|p| p["snapshot"]["active"].as_array().unwrap().len())
                .collect::<Vec<_>>(),
            [3, 3, 2, 1, 1]
        );
        assert_eq!(pulses[4]["snapshot"]["totalStoredCubicMeters"], 23.);
        assert_eq!(pulses[4]["snapshot"]["active"][0]["waterLevelMeters"], 6.);
        assert_eq!(report["analysis"]["analysisVersion"], "spill-connections-1");
    }

    #[test]
    fn report_checkpoint_continues_exactly() {
        let report = run(FIXTURE.as_bytes()).unwrap();
        let input = json!({"region": 2, "volumeCubicMeters": 0.12345678901234568});
        let resumed =
            json!({"start": {"checkpoint": report["finalCheckpoint"]}, "inputs": [input.clone()]});
        let resumed = run(serde_json::to_vec(&resumed).unwrap().as_slice()).unwrap();
        let mut full: Value = serde_json::from_str(FIXTURE).unwrap();
        full["inputs"].as_array_mut().unwrap().push(input);
        let full = run(serde_json::to_vec(&full).unwrap().as_slice()).unwrap();
        assert_eq!(resumed["finalCheckpoint"], full["finalCheckpoint"]);
    }

    #[test]
    fn invalid_and_excessive_requests_do_not_produce_partial_reports() {
        assert!(run(&b"{}"[..]).is_err());
        assert!(run(vec![b' '; 32769].as_slice()).is_err());
        let good: Value = serde_json::from_str(FIXTURE).unwrap();
        for inputs in [
            json!([{"region": 0, "volumeCubicMeters": 1.}, {"region": 4, "volumeCubicMeters": -1.}]),
            json!([{"region": 0}]),
            json!(vec![json!({"region": 0, "volumeCubicMeters": 0.}); 1025]),
        ] {
            let mut bad = good.clone();
            bad["inputs"] = inputs;
            assert!(run(serde_json::to_vec(&bad).unwrap().as_slice()).is_err());
        }
        let mut bad = good.clone();
        bad["start"]["unknown"] = true.into();
        assert!(run(serde_json::to_vec(&bad).unwrap().as_slice()).is_err());
        let mut bad = good;
        bad["unknown"] = true.into();
        assert!(run(serde_json::to_vec(&bad).unwrap().as_slice()).is_err());
        let report = run(FIXTURE.as_bytes()).unwrap();
        let mut checkpoint = report["finalCheckpoint"].clone();
        checkpoint["experimentVersion"] = "future".into();
        let bad = json!({"start": {"checkpoint": checkpoint}, "inputs": []});
        assert!(run(serde_json::to_vec(&bad).unwrap().as_slice()).is_err());
    }
}
