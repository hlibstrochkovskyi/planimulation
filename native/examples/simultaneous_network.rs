//! Concurrent-forcing laboratory; never mutates desktop or planetary water.
use planimulation_core::nested_reservoir::Input;
use planimulation_core::spill_network::simultaneous::{Checkpoint, Setup, SimultaneousNetwork};
use serde::Deserialize;
use serde_json::{Value, json};
use std::io::{Read, Write};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
enum Start {
    Dry(Setup),
    Checkpoint(Checkpoint),
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Request {
    start: Start,
    intervals: Vec<Vec<Input>>,
}

fn run(input: impl Read) -> Result<Value, Box<dyn std::error::Error>> {
    let mut bytes = Vec::new();
    input.take(32769).read_to_end(&mut bytes)?;
    if bytes.len() > 32768 {
        return Err("Concurrent request exceeds 32 KiB.".into());
    }
    let request: Request = serde_json::from_slice(&bytes)?;
    if request.intervals.len() > 1024 {
        return Err("At most 1024 forcing intervals are allowed.".into());
    }
    let mut model = match request.start {
        Start::Dry(s) => SimultaneousNetwork::new(s)?,
        Start::Checkpoint(c) => SimultaneousNetwork::restore(c)?,
    };
    let initial = model.checkpoint();
    let mut intervals = Vec::new();
    for inputs in request.intervals {
        intervals.push(model.add_interval(inputs)?);
    }
    Ok(json!({"reportVersion":1,
        "scope":"Closed <=128-region constant concurrent forcing experiment; normalized intervals, instantaneous routing, not finite-rate hydraulics or desktop dynamics.",
        "initialCheckpoint":initial,"analysis":model.connections(),"intervals":intervals,"finalCheckpoint":model.checkpoint()}))
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() != 1 {
        return Err("Usage: simultaneous_network <experiment.json | ->".into());
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
    const FIXTURE: &str = include_str!("../../docs/scenarios/simultaneous-network.json");
    #[test]
    fn fixture_and_reordered_inputs_have_identical_reports() {
        let report = run(FIXTURE.as_bytes()).unwrap();
        assert_eq!(
            report["intervals"][0]["events"].as_array().unwrap().len(),
            3
        );
        assert_eq!(
            report["intervals"][1]["snapshot"]["totalStoredCubicMeters"],
            17.
        );
        assert_eq!(
            report["intervals"][1]["snapshot"]["active"][0]["waterLevelMeters"],
            5.
        );
        let mut reversed: Value = serde_json::from_str(FIXTURE).unwrap();
        reversed["intervals"][0].as_array_mut().unwrap().reverse();
        assert_eq!(
            report,
            run(serde_json::to_vec(&reversed).unwrap().as_slice()).unwrap()
        );
    }
    #[test]
    fn checkpoint_continuation_matches_uninterrupted_execution() {
        let report = run(FIXTURE.as_bytes()).unwrap();
        let interval = json!([{"region":0,"volumeCubicMeters":7}]);
        let resumed = json!({"start":{"checkpoint":report["finalCheckpoint"]},"intervals":[interval.clone()]});
        let resumed = run(serde_json::to_vec(&resumed).unwrap().as_slice()).unwrap();
        let mut full: Value = serde_json::from_str(FIXTURE).unwrap();
        full["intervals"].as_array_mut().unwrap().push(interval);
        let full = run(serde_json::to_vec(&full).unwrap().as_slice()).unwrap();
        assert_eq!(full["finalCheckpoint"], resumed["finalCheckpoint"]);
        assert_eq!(
            resumed["intervals"][0]["snapshot"]["totalStoredCubicMeters"],
            24.
        );
    }
    #[test]
    fn invalid_requests_and_late_failures_do_not_publish_reports() {
        assert!(run(&b"{}"[..]).is_err());
        assert!(run(vec![b' '; 32769].as_slice()).is_err());
        let good: Value = serde_json::from_str(FIXTURE).unwrap();
        for (key, value) in [
            ("unknown", json!(true)),
            ("intervals", json!([[{"region":0,"volumeCubicMeters":-1}]])),
        ] {
            let mut bad = good.clone();
            bad[key] = value;
            assert!(run(serde_json::to_vec(&bad).unwrap().as_slice()).is_err());
        }
        let mut bad = good.clone();
        bad["intervals"][1][0]["volumeCubicMeters"] = json!(-1);
        assert!(run(serde_json::to_vec(&bad).unwrap().as_slice()).is_err());
        let mut bad = good.clone();
        bad["start"]["dry"]["policyVersion"] = json!("future");
        assert!(run(serde_json::to_vec(&bad).unwrap().as_slice()).is_err());
        let mut bad = good;
        bad["intervals"] = json!(vec![Vec::<Input>::new(); 1025]);
        let bytes = serde_json::to_vec(&bad).unwrap();
        assert!(bytes.len() < 32768);
        assert!(
            run(bytes.as_slice())
                .unwrap_err()
                .to_string()
                .contains("1024")
        );
    }
}
