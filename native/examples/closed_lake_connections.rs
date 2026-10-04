//! Static geometric recipient certificates. No seasonal advancement or water transfer.
use planimulation_core::{Recipe, World, seasonal_moisture::closed_lake::spill};
use serde_json::json;

fn main() -> Result<(), String> {
    if std::env::args().len() != 1 {
        return Err("Usage: closed_lake_connections (fixed retained reference)".into());
    }
    let mut recipe: Recipe = serde_json::from_str(include_str!(
        "../../docs/scenarios/seasonal-temperature.json"
    ))
    .map_err(|e| e.to_string())?;
    recipe.subdivision = 2;
    let world = World::generate(recipe)?;
    let connections = spill::first_connections(&world)?;
    println!("{}",serde_json::to_string_pretty(&json!({"reportVersion":1,"analysisVersion":spill::ANALYSIS_VERSION,"recipe":world.recipe,"scope":"Static first-connection certificates for closed minimum leaves. Canonical sill passages followed by existing downhill receivers identify actual wet-body contacts or closed terminals. All alternatives retained; no water moves, no active neighboring capacity/backpressure is checked, no hydraulic time, allocation policy, merge or simulation checkpoint is created.","connections":connections})).map_err(|e|e.to_string())?);
    Ok(())
}
