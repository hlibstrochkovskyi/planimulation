//! Bounded headless continuation from a generated initial-water state.
use planimulation_core::{
    Recipe, World,
    nested_reservoir::Input,
    spill_network::simultaneous::multi_entry::seeded::{
        self, Checkpoint, SeededNetwork, UNIT_WEIGHT_POLICY_VERSION,
    },
};
use serde::Deserialize;
use serde_json::{Value, json};
use std::io::{Read, Write};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Generated {
    recipe: Recipe,
    weight_policy_version: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
enum Start {
    Generated(Generated),
    Checkpoint(Box<Checkpoint>),
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
        return Err("Seeded-network request exceeds 32 KiB.".into());
    }
    let request: Request = serde_json::from_slice(&bytes)?;
    if request.intervals.len() > 1024 {
        return Err("At most 1024 forcing intervals are allowed.".into());
    }
    let mut model = match request.start {
        Start::Generated(generated) => {
            if generated.weight_policy_version != UNIT_WEIGHT_POLICY_VERSION {
                return Err("Unsupported generated-world weight policy.".into());
            }
            if generated.recipe.subdivision > 5 {
                return Err(
                    "Generated world exceeds the bounded seeded-network laboratory.".into(),
                );
            }
            let world = World::generate(generated.recipe)?;
            let setup = seeded::generated_unit_setup(&world)?;
            SeededNetwork::from_generated(&world, setup)?
        }
        Start::Checkpoint(checkpoint) => SeededNetwork::restore(*checkpoint)?,
    };
    let initial_checkpoint = model.checkpoint();
    let initial_snapshot = model.snapshot()?;
    let mut intervals = Vec::new();
    for inputs in request.intervals {
        intervals.push(model.add_interval(inputs)?);
    }
    Ok(json!({
        "reportVersion": 1,
        "scope": "Closed <=128-region seeded network, unit branch/entry weights, prescribed normalized intervals. No climate, finite-rate hydraulics, planet-scale solver or desktop dynamics.",
        "initialCheckpoint": initial_checkpoint,
        "initialSnapshot": initial_snapshot,
        "intervals": intervals,
        "finalCheckpoint": model.checkpoint(),
    }))
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() != 1 {
        return Err("Usage: seeded_network <experiment.json | ->".into());
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
    const FIXTURE: &str = include_str!("../../docs/scenarios/seeded-network.json");

    #[test]
    fn generated_recipe_replays_and_keeps_ledgers_distinct() {
        let report = run(FIXTURE.as_bytes()).unwrap();
        assert_eq!(report, run(FIXTURE.as_bytes()).unwrap());
        assert_eq!(
            report["finalCheckpoint"]["experimentVersion"],
            "seeded-multi-entry-network-1"
        );
        assert_eq!(
            report["initialCheckpoint"]["inventory"]["inputCubicMeters"],
            0.
        );
        assert_eq!(
            report["finalCheckpoint"]["inventory"]["inputCubicMeters"],
            2e14
        );
        assert_eq!(report["intervals"].as_array().unwrap().len(), 1);
        assert_eq!(
            report["intervals"][0]["events"].as_array().unwrap().len(),
            3
        );
        assert_eq!(report["intervals"][0]["events"][1]["merged"][0], 7);
        let next = json!({"start":{"checkpoint":report["finalCheckpoint"]},
            "intervals":[[{"region":0,"volumeCubicMeters":1e12}]]});
        let resumed = run(serde_json::to_vec(&next).unwrap().as_slice()).unwrap();
        let mut uninterrupted: Value = serde_json::from_str(FIXTURE).unwrap();
        uninterrupted["intervals"]
            .as_array_mut()
            .unwrap()
            .push(json!([{"region":0,"volumeCubicMeters":1e12}]));
        let full = run(serde_json::to_vec(&uninterrupted).unwrap().as_slice()).unwrap();
        assert_eq!(resumed["finalCheckpoint"], full["finalCheckpoint"]);
    }

    #[test]
    fn invalid_version_size_recipe_and_late_input_fail_before_report() {
        assert!(run(&b"{}"[..]).is_err());
        assert!(run(vec![b' '; 32769].as_slice()).is_err());
        let mut request: Value = serde_json::from_str(FIXTURE).unwrap();
        request["start"]["generated"]["weightPolicyVersion"] = json!("future");
        assert!(run(serde_json::to_vec(&request).unwrap().as_slice()).is_err());
        let mut request: Value = serde_json::from_str(FIXTURE).unwrap();
        request["start"]["generated"]["recipe"]["subdivision"] = json!(6);
        assert!(run(serde_json::to_vec(&request).unwrap().as_slice()).is_err());
        let mut request: Value = serde_json::from_str(FIXTURE).unwrap();
        request["intervals"]
            .as_array_mut()
            .unwrap()
            .push(json!([{"region":0,"volumeCubicMeters":-1}]));
        assert!(run(serde_json::to_vec(&request).unwrap().as_slice()).is_err());
    }
}
