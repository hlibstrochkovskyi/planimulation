//! Bounded developer experiment; no desktop/world state is read or modified.
use planimulation_core::reservoir::{Checkpoint, Reservoir};
use serde::Deserialize;
use serde_json::{Value, json};
use std::io::{Read, Write};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Request {
    checkpoint: Checkpoint,
    inputs_cubic_meters: Vec<f64>,
}

fn run(input: impl Read) -> Result<Value, Box<dyn std::error::Error>> {
    let mut bytes = Vec::new();
    input.take(32769).read_to_end(&mut bytes)?;
    if bytes.len() > 32768 {
        return Err("Reservoir request exceeds 32 KiB.".into());
    }
    let request: Request = serde_json::from_slice(&bytes)?;
    if request.inputs_cubic_meters.len() > 1024 {
        return Err("At most 1024 input pulses are allowed.".into());
    }
    let mut reservoir = Reservoir::restore(request.checkpoint)?;
    let initial = reservoir.checkpoint();
    let mut pulses = Vec::new();
    for input in request.inputs_cubic_meters {
        pulses.push(reservoir.add_input(input)?);
    }
    Ok(json!({
        "reportVersion": 1,
        "scope": "Isolated column reservoir with instantaneous equilibration; external outflow is collected, not routed to another lake. Pulse index is not time.",
        "initialCheckpoint": initial,
        "capacityCubicMeters": reservoir.capacity_cubic_meters(),
        "pulses": pulses,
        "finalCheckpoint": reservoir.checkpoint(),
    }))
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() != 1 {
        return Err("Usage: reservoir <experiment.json | ->".into());
    }
    let input: Box<dyn Read> = if args[0] == "-" {
        Box::new(std::io::stdin())
    } else {
        Box::new(std::fs::File::open(&args[0])?)
    };
    // Complete and validate every pulse before publishing any JSON.
    let report = run(input)?;
    let mut out = std::io::stdout().lock();
    serde_json::to_writer_pretty(&mut out, &report)?;
    writeln!(out)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    const SILL: &str = include_str!("../../docs/scenarios/reservoir-sill.json");
    const CLOSED: &str = include_str!("../../docs/scenarios/reservoir-closed.json");

    #[test]
    fn fixtures_reproduce_and_account_for_the_collector_or_closed_storage() {
        let a = run(SILL.as_bytes()).unwrap();
        assert_eq!(
            serde_json::to_vec(&a).unwrap(),
            serde_json::to_vec(&run(SILL.as_bytes()).unwrap()).unwrap()
        );
        assert_eq!(a["capacityCubicMeters"], 12.);
        assert_eq!(
            a["finalCheckpoint"]["inventory"]["outflowVolumeCubicMeters"],
            4.
        );
        assert_eq!(
            a["finalCheckpoint"]["inventory"]["storedVolumeCubicMeters"],
            12.
        );
        let b = run(CLOSED.as_bytes()).unwrap();
        assert!(b["capacityCubicMeters"].is_null());
        assert_eq!(
            b["finalCheckpoint"]["inventory"]["outflowVolumeCubicMeters"],
            0.
        );
        assert_eq!(
            b["finalCheckpoint"]["inventory"]["storedVolumeCubicMeters"],
            16.
        );
    }

    #[test]
    fn report_checkpoint_resumes_exactly() {
        let a = run(SILL.as_bytes()).unwrap();
        let resumed =
            json!({ "checkpoint": a["finalCheckpoint"], "inputsCubicMeters": [0.13, 0.27] });
        let resumed = run(serde_json::to_vec(&resumed).unwrap().as_slice()).unwrap();
        let mut full: Value = serde_json::from_str(SILL).unwrap();
        full["inputsCubicMeters"]
            .as_array_mut()
            .unwrap()
            .extend([json!(0.13), json!(0.27)]);
        let full = run(serde_json::to_vec(&full).unwrap().as_slice()).unwrap();
        assert_eq!(resumed["finalCheckpoint"], full["finalCheckpoint"]);
    }

    #[test]
    fn invalid_version_fields_pulses_and_bounded_input_fail_without_a_report() {
        assert!(run(&b"{}"[..]).is_err());
        assert!(run(vec![b' '; 32769].as_slice()).is_err());
        let good: Value = serde_json::from_str(SILL).unwrap();
        for field in ["experimentVersion", "boundary", "columns", "inventory"] {
            let mut bad = good.clone();
            bad["checkpoint"][field] = Value::Null;
            assert!(run(serde_json::to_vec(&bad).unwrap().as_slice()).is_err());
        }
        for inputs in [json!([1., -1.]), json!([1., "NaN"]), json!(vec![0.; 1025])] {
            let mut bad = good.clone();
            bad["inputsCubicMeters"] = inputs;
            assert!(run(serde_json::to_vec(&bad).unwrap().as_slice()).is_err());
        }
        let mut bad = good;
        bad["unexpected"] = true.into();
        assert!(run(serde_json::to_vec(&bad).unwrap().as_slice()).is_err());
    }
}
