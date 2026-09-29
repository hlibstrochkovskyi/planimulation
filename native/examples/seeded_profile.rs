//! Compact, observational profile of one generated seeded-network interval.
use planimulation_core::{
    Recipe, World,
    nested_reservoir::Input,
    spill_network::simultaneous::multi_entry::seeded::{
        SHARED_STORAGE_EXPERIMENT_VERSION, SeededNetwork, UNIT_WEIGHT_POLICY_VERSION,
        generated_unit_setup,
    },
};
use serde::Deserialize;
use serde_json::{Value, json};
use std::io::{Read, Write};
use std::time::Instant;

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
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Request {
    start: Start,
    intervals: Vec<Vec<Input>>,
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

fn curve_references(world: &World) -> Result<usize, String> {
    let nodes = world.basins.nodes();
    let mut members = vec![0usize; nodes.len()];
    for &owner in world.basins.region_nodes() {
        members[owner] += 1;
    }
    let mut total = 0usize;
    for (id, node) in nodes.iter().enumerate() {
        for &child in &node.children {
            members[id] = members[id]
                .checked_add(members[child])
                .ok_or("Profile reference count overflowed.")?;
        }
        total = total
            .checked_add(members[id])
            .ok_or("Profile reference count overflowed.")?;
    }
    Ok(total)
}

fn run(input: impl Read) -> Result<Value, Box<dyn std::error::Error>> {
    let mut bytes = Vec::new();
    input.take(32769).read_to_end(&mut bytes)?;
    if bytes.len() > 32768 {
        return Err("Profile request exceeds 32 KiB.".into());
    }
    let request: Request = serde_json::from_slice(&bytes)?;
    let Start::Generated(generated) = request.start;
    if generated.weight_policy_version != UNIT_WEIGHT_POLICY_VERSION
        || generated.recipe.subdivision > 5
        || request.intervals.len() != 1
    {
        return Err("Profile requires one supported generated interval and unit weights.".into());
    }
    let started = Instant::now();
    let world = World::generate(generated.recipe)?;
    let generation_ms = started.elapsed().as_secs_f64() * 1000.;
    let references = curve_references(&world)?;
    let initial_volume = world.water.resolved_volume_cubic_meters;
    let baseline = Instant::now();
    let setup = generated_unit_setup(&world)?;
    let setup_ms = baseline.elapsed().as_secs_f64() * 1000.;
    let baseline = Instant::now();
    let prepared = SeededNetwork::from_generated(&world, setup);
    let preparation_ms = baseline.elapsed().as_secs_f64() * 1000.;
    let common = json!({
        "reportVersion": 2,
        "scope": "Observational single-process timings and Linux peak RSS when available; one normalized interval, no desktop or climate model.",
        "recipe": world.recipe,
        "regionCount": world.surface.areas.len(),
        "branchCount": world.basins.nodes().len(),
        "logicalSubtreeColumnReferences": references,
        "initialVolumeCubicMeters": initial_volume,
        "generationMilliseconds": generation_ms,
        "setupMilliseconds": setup_ms,
        "preparationMilliseconds": preparation_ms,
    });
    let mut report = common;
    match prepared {
        Err(reason) => {
            report["status"] = json!("rejectedDuringPreparation");
            report["reason"] = json!(reason);
        }
        Ok(mut model) => {
            report["experimentVersion"] = json!(model.experiment_version());
            report["storageRepresentation"] = json!(if model.experiment_version()
                == SHARED_STORAGE_EXPERIMENT_VERSION
            {
                "shared-subtree-region-index-1"
            } else {
                "duplicated-branch-curves-1"
            });
            let baseline = Instant::now();
            match model.add_interval(request.intervals.into_iter().next().unwrap()) {
                Err(reason) => {
                    report["status"] = json!("rejectedDuringInterval");
                    report["reason"] = json!(reason);
                }
                Ok(interval) => {
                    report["status"] = json!("accepted");
                    report["inputCubicMeters"] = json!(
                        interval
                            .inputs
                            .iter()
                            .map(|input| input.volume_cubic_meters)
                            .sum::<f64>()
                    );
                    report["finalStoredCubicMeters"] =
                        json!(interval.snapshot.total_stored_cubic_meters);
                    report["budgetResidualCubicMeters"] =
                        json!(interval.snapshot.budget_residual_cubic_meters);
                    report["eventCount"] = json!(interval.events.len());
                }
            }
            report["intervalMilliseconds"] = json!(baseline.elapsed().as_secs_f64() * 1000.);
        }
    }
    report["peakRssKib"] = json!(peak_rss_kib());
    Ok(report)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() != 1 {
        return Err("Usage: seeded_profile <experiment.json | ->".into());
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
    fn profile_reports_conservative_generated_run_and_rejection() {
        let accepted = run(FIXTURE.as_bytes()).unwrap();
        assert_eq!(accepted["status"], "accepted");
        assert_eq!(accepted["reportVersion"], 2);
        assert_eq!(accepted["regionCount"], 42);
        assert_eq!(accepted["logicalSubtreeColumnReferences"], 72);
        assert_eq!(
            accepted["storageRepresentation"],
            "duplicated-branch-curves-1"
        );
        assert_eq!(
            accepted["finalStoredCubicMeters"],
            4_592_913_244_410_118_f64
        );
        let mut tiny: Value = serde_json::from_str(FIXTURE).unwrap();
        tiny["intervals"][0][0]["volumeCubicMeters"] = json!(1e-4);
        let rejected = run(serde_json::to_vec(&tiny).unwrap().as_slice()).unwrap();
        assert_eq!(rejected["status"], "rejectedDuringInterval");
        assert!(rejected["reason"].as_str().unwrap().contains("precision"));
        assert_eq!(
            rejected["initialVolumeCubicMeters"],
            accepted["initialVolumeCubicMeters"]
        );
        let mut expanded: Value = serde_json::from_str(FIXTURE).unwrap();
        expanded["start"]["generated"]["recipe"]["subdivision"] = json!(2);
        let shared = run(serde_json::to_vec(&expanded).unwrap().as_slice()).unwrap();
        assert_eq!(shared["status"], "accepted");
        assert_eq!(
            shared["experimentVersion"],
            SHARED_STORAGE_EXPERIMENT_VERSION
        );
        assert_eq!(
            shared["storageRepresentation"],
            "shared-subtree-region-index-1"
        );
        assert_eq!(shared["logicalSubtreeColumnReferences"], 879);
    }

    #[test]
    fn malformed_and_oversized_requests_fail_before_report() {
        assert!(run(&b"{}"[..]).is_err());
        assert!(run(vec![b' '; 32769].as_slice()).is_err());
        let mut invalid: Value = serde_json::from_str(FIXTURE).unwrap();
        invalid["start"]["generated"]["recipe"]["subdivision"] = json!(6);
        assert!(run(serde_json::to_vec(&invalid).unwrap().as_slice()).is_err());
        let mut invalid: Value = serde_json::from_str(FIXTURE).unwrap();
        invalid["intervals"] = json!([]);
        assert!(run(serde_json::to_vec(&invalid).unwrap().as_slice()).is_err());
    }
}
