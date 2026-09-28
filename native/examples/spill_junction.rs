//! Bounded shared-sill allocation report, not a desktop or world command.
use planimulation_core::{
    nested_reservoir::Stock,
    spill_junction::{Checkpoint, Setup, SpillJunction},
};
use serde::Deserialize;
use serde_json::{Value, json};
use std::io::{Read, Write};

// External tagging preserves scoped raw-decimal checkpoint readers.
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
    inputs: Vec<Vec<Stock>>,
}

fn run(input: impl Read) -> Result<Value, Box<dyn std::error::Error>> {
    let mut bytes = Vec::new();
    input.take(32769).read_to_end(&mut bytes)?;
    if bytes.len() > 32768 {
        return Err("Junction request exceeds 32 KiB.".into());
    }
    let request: Request = serde_json::from_slice(&bytes)?;
    if request.inputs.len() > 1024 {
        return Err("At most 1024 junction batches are allowed.".into());
    }
    let mut junction = match request.start {
        Start::Dry(setup) => SpillJunction::new(setup)?,
        Start::Checkpoint(checkpoint) => SpillJunction::restore(checkpoint)?,
    };
    let initial = junction.checkpoint();
    let initial_snapshot = junction.snapshot()?;
    let mut pulses = Vec::new();
    for input in request.inputs {
        pulses.push(junction.add_input(&input)?);
    }
    Ok(json!({ "reportVersion": 1,
        "scope": "Closed single-plateau leaf junction with prescribed receiver weights and simultaneous volume batches; not hydraulic conductance, nested routing, or planetary dynamics.",
        "initialCheckpoint": initial, "initialSnapshot": initial_snapshot,
        "analysis": junction.connections(), "plateau": junction.plateau(),
        "capacities": junction.capacities(), "pulses": pulses, "finalCheckpoint": junction.checkpoint() }))
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() != 1 {
        return Err("Usage: spill_junction <experiment.json | ->".into());
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
    const FIXTURE: &str = include_str!("../../docs/scenarios/spill-junction.json");
    #[test]
    fn fixture_reports_weighted_capped_allocation_then_common_storage() {
        let report = run(FIXTURE.as_bytes()).unwrap();
        assert_eq!(report, run(FIXTURE.as_bytes()).unwrap());
        assert_eq!(
            report["pulses"][1]["spillReceived"][1]["volumeCubicMeters"],
            0.5
        );
        assert_eq!(
            report["pulses"][1]["spillReceived"][2]["volumeCubicMeters"],
            1.5
        );
        assert_eq!(report["pulses"][3]["snapshot"]["phase"], "atSill");
        assert_eq!(report["pulses"][4]["snapshot"]["levelsMeters"], json!([5.]));
        assert_eq!(
            report["pulses"][4]["snapshot"]["totalStoredCubicMeters"],
            13.
        );
    }
    #[test]
    fn checkpoint_report_resumes_with_identical_results() {
        let report = run(FIXTURE.as_bytes()).unwrap();
        let input = json!([{"branch":0,"volumeCubicMeters":0.12345678901234568},{"branch":1,"volumeCubicMeters":0.},{"branch":2,"volumeCubicMeters":0.}]);
        let request =
            json!({"start":{"checkpoint":report["finalCheckpoint"]},"inputs":[input.clone()]});
        let resumed = run(serde_json::to_vec(&request).unwrap().as_slice()).unwrap();
        let mut full: Value = serde_json::from_str(FIXTURE).unwrap();
        full["inputs"].as_array_mut().unwrap().push(input);
        let full = run(serde_json::to_vec(&full).unwrap().as_slice()).unwrap();
        assert_eq!(resumed["finalCheckpoint"], full["finalCheckpoint"]);
    }
    #[test]
    fn invalid_versions_fields_batches_and_limits_fail_without_report() {
        assert!(run(&b"{}"[..]).is_err());
        assert!(run(vec![b' '; 32769].as_slice()).is_err());
        let good: Value = serde_json::from_str(FIXTURE).unwrap();
        let mut bad = good.clone();
        bad["start"]["dry"]["policyVersion"] = "future".into();
        assert!(run(serde_json::to_vec(&bad).unwrap().as_slice()).is_err());
        let mut bad = good.clone();
        bad["unknown"] = true.into();
        assert!(run(serde_json::to_vec(&bad).unwrap().as_slice()).is_err());
        let mut bad = good.clone();
        bad["inputs"][4][0]["volumeCubicMeters"] = (-1.).into();
        assert!(run(serde_json::to_vec(&bad).unwrap().as_slice()).is_err());
        let mut bad = good.clone();
        // Stay below the byte limit to exercise the batch-count guard itself.
        bad["inputs"] = json!(vec![json!([]); 1025]);
        assert!(
            run(serde_json::to_vec(&bad).unwrap().as_slice())
                .unwrap_err()
                .to_string()
                .contains("1024")
        );
        let mut bad = good;
        bad["inputs"][0] = json!([]);
        assert!(run(serde_json::to_vec(&bad).unwrap().as_slice()).is_err());
    }
}
