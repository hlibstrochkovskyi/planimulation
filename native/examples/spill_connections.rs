//! Standalone topology report with an optional, bounded passage query.
use planimulation_core::{Recipe, World, spill_connections::SpillConnections};
use serde::Serialize;
use serde_json::{Value, json};
use std::io::{Read, Write};

#[derive(Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
struct Query {
    source: usize,
    receiver: usize,
    plateau: usize,
}

fn run(input: impl Read, query: Option<Query>) -> Result<Value, Box<dyn std::error::Error>> {
    let mut bytes = Vec::new();
    input.take(32769).read_to_end(&mut bytes)?;
    if bytes.len() > 32768 {
        return Err("Recipe exceeds 32 KiB.".into());
    }
    let recipe: Recipe = serde_json::from_slice(&bytes)?;
    let world = World::generate(recipe)?;
    let analysis = SpillConnections::build(&world.surface, &world.terrain.elevation)?;
    let receivers = query.map(|q| analysis.receivers(q.source)).transpose()?;
    let passage = query
        .map(|q| analysis.passage(q.source, q.receiver, q.plateau))
        .transpose()?;
    Ok(json!({ "reportVersion": 1, "recipe": world.recipe,
        "scope": "Static geometric sill connections and potential passages; no receiver allocation, water movement, or external outlet.",
        "analysis": analysis, "query": query, "receivers": receivers, "passage": passage }))
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() != 1 && args.len() != 4 {
        return Err("Usage: spill_connections <recipe.json | -> [source receiver plateau]".into());
    }
    let query = if args.len() == 4 {
        Some(Query {
            source: args[1].parse()?,
            receiver: args[2].parse()?,
            plateau: args[3].parse()?,
        })
    } else {
        None
    };
    let input: Box<dyn Read> = if args[0] == "-" {
        Box::new(std::io::stdin())
    } else {
        Box::new(std::fs::File::open(&args[0])?)
    };
    // Publish only after both the topology and optional query succeed.
    let report = run(input, query)?;
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
    fn report_reproduces_and_optional_query_has_real_region_contacts() {
        let a = run(RECIPE.as_bytes(), None).unwrap();
        assert_eq!(a, run(RECIPE.as_bytes(), None).unwrap());
        assert_eq!(a["analysis"]["analysisVersion"], "spill-connections-1");
        let (plateau, source, receiver) = a["analysis"]["plateaus"]
            .as_array()
            .unwrap()
            .iter()
            .enumerate()
            .find_map(|(i, p)| {
                let cs = p["contacts"].as_array().unwrap();
                let source = cs[0]["childBranch"].as_u64().unwrap() as usize;
                cs.iter()
                    .find(|c| c["childBranch"].as_u64().unwrap() as usize != source)
                    .map(|c| (i, source, c["childBranch"].as_u64().unwrap() as usize))
            })
            .unwrap();
        let queried = run(
            RECIPE.as_bytes(),
            Some(Query {
                source,
                receiver,
                plateau,
            }),
        )
        .unwrap();
        assert_eq!(a["analysis"], queried["analysis"]);
        assert!(queried["passage"]["regions"].as_array().unwrap().len() >= 3);
        assert_eq!(queried["passage"]["sourceBranch"], source);
    }
    #[test]
    fn invalid_query_and_recipe_fail_without_a_report() {
        assert!(
            run(
                RECIPE.as_bytes(),
                Some(Query {
                    source: usize::MAX,
                    receiver: 0,
                    plateau: 0
                })
            )
            .is_err()
        );
        assert!(run(&b"{}"[..], None).is_err());
        assert!(run(RECIPE.replace("basins-1", "drainage-1").as_bytes(), None).is_err());
        assert!(run(vec![b' '; 32769].as_slice(), None).is_err());
    }
}
