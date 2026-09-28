//! Report structural integration blockers without constructing the bounded solver.
use planimulation_core::{Recipe, World, spill_readiness::Screening};
use serde_json::{Value, json};
use std::io::{Read, Write};

fn run(input: impl Read) -> Result<Value, Box<dyn std::error::Error>> {
    let mut bytes = Vec::new();
    input.take(32769).read_to_end(&mut bytes)?;
    if bytes.len() > 32768 {
        return Err("Recipe exceeds 32 KiB.".into());
    }
    let recipe: Recipe = serde_json::from_slice(&bytes)?;
    let world = World::generate(recipe)?;
    let screening = Screening::build(&world.surface, &world.terrain.elevation)?;
    Ok(json!({"reportVersion":1, "recipe":world.recipe,
        "scope":"Read-only C3f/C3g structural screening; passing checks does not establish solver readiness. No water movement or initial-inventory import.",
        "screening":screening}))
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() != 1 {
        return Err("Usage: spill_readiness <recipe.json | ->".into());
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
    const RECIPE: &str = include_str!("../../docs/scenarios/spill-connections.json");
    #[test]
    fn recipe_report_is_reproducible_and_does_not_claim_dynamic_readiness() {
        let a = run(RECIPE.as_bytes()).unwrap();
        assert_eq!(a, run(RECIPE.as_bytes()).unwrap());
        assert_eq!(a["screening"]["analysisVersion"], "spill-readiness-1");
        assert_eq!(a["screening"]["regionCount"], 162);
        assert_eq!(a["screening"]["withinLaboratoryRegionLimit"], false);
        assert!(a["screening"]["notChecked"].as_array().unwrap().len() >= 6);
        let mut wet: Value = serde_json::from_str(RECIPE).unwrap();
        wet["water"]["fraction"] = json!(0.);
        assert_eq!(
            a["screening"],
            run(serde_json::to_vec(&wet).unwrap().as_slice()).unwrap()["screening"]
        );
    }
    #[test]
    fn malformed_oversized_and_legacy_recipes_fail_before_output() {
        assert!(run(&b"{}"[..]).is_err());
        assert!(run(vec![b' '; 32769].as_slice()).is_err());
        assert!(run(RECIPE.replace("basins-1", "drainage-1").as_bytes()).is_err());
        let mut recipe: Value = serde_json::from_str(RECIPE).unwrap();
        recipe["unknown"] = true.into();
        assert!(run(serde_json::to_vec(&recipe).unwrap().as_slice()).is_err());
    }
}
