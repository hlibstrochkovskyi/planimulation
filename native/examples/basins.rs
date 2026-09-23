//! Standalone analysis report. Does not change desktop state or the wire protocol.
use planimulation_core::{Recipe, World, basins::Basins};
use serde_json::json;
use std::io::{Read, Write};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() != 1 {
        return Err("Usage: basins <recipe.json | ->".into());
    }
    let input: Box<dyn Read> = if args[0] == "-" {
        Box::new(std::io::stdin())
    } else {
        Box::new(std::fs::File::open(&args[0])?)
    };
    let mut bytes = Vec::new();
    input.take(32769).read_to_end(&mut bytes)?;
    if bytes.len() > 32768 {
        return Err("Recipe exceeds 32 KiB.".into());
    }
    let recipe: Recipe = serde_json::from_slice(&bytes)?;
    let world = World::generate(recipe)?;
    let analysis = Basins::build(&world.surface, &world.terrain.elevation)?;
    let report = json!({ "reportVersion": 1, "recipe": world.recipe,
        "scope": "Static bed connectivity and prism storage; no water movement or external outlet.",
        "regionCount": world.surface.areas.len(),
        "leafCount": analysis.nodes().iter().filter(|node| node.children.is_empty()).count(),
        "analysis": analysis });
    let mut out = std::io::stdout().lock();
    serde_json::to_writer_pretty(&mut out, &report)?;
    writeln!(out)?;
    Ok(())
}
