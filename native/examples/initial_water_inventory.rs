//! Read-only generated-world inventory report; no input or water movement.
use planimulation_core::{Recipe, World, initial_water_inventory::InitialWaterInventory};
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
    let inventory = InitialWaterInventory::from_water(
        &world.surface,
        &world.terrain.elevation,
        &world.water,
        &world.basins,
    )?;
    let restored =
        inventory.reconstruct(&world.surface, &world.terrain.elevation, &world.basins)?;
    Ok(json!({
        "reportVersion": 1,
        "recipe": world.recipe,
        "scope": "Read-only initial-water inventory transfer. This is not a dynamic spill-network checkpoint and does not advance water.",
        "regionCount": world.surface.areas.len(),
        "bodyCount": restored.body_ids.iter().copied().max().unwrap_or(0),
        "activeStockCount": inventory.stocks.len(),
        "inventory": inventory,
    }))
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() != 1 {
        return Err("Usage: initial_water_inventory <recipe.json | ->".into());
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
    fn report_replays_and_rejects_invalid_recipes() {
        let report = run(RECIPE.as_bytes()).unwrap();
        assert_eq!(report, run(RECIPE.as_bytes()).unwrap());
        assert_eq!(
            report["inventory"]["importVersion"],
            "initial-water-inventory-1"
        );
        assert_eq!(report["regionCount"], 162);
        assert!(run(&b"{}"[..]).is_err());
        assert!(run(vec![b' '; 32769].as_slice()).is_err());
        assert!(run(RECIPE.replace("basins-1", "drainage-1").as_bytes()).is_err());
    }
}
