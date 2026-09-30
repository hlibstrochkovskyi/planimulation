//! Read-only, opt-in exact initial-water accounting report; no water movement.
use planimulation_core::{Recipe, World, exact_initial_accounting::ExactInitialAccounting};
use serde_json::{Value, json};
use std::io::{Read, Write};

fn run(input: impl Read) -> Result<Value, Box<dyn std::error::Error>> {
    let mut bytes = Vec::new();
    input.take(32_769).read_to_end(&mut bytes)?;
    if bytes.len() > 32_768 {
        return Err("Recipe exceeds 32 KiB.".into());
    }
    let recipe: Recipe = serde_json::from_slice(&bytes)?;
    let world = World::generate(recipe)?;
    let accounting = ExactInitialAccounting::from_world(&world)?;
    Ok(json!({
        "reportVersion": 1,
        "scope": "Read-only exact accounting of represented initial-water contributions; no dynamic routing or simulation checkpoint.",
        "accounting": accounting,
    }))
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() != 1 {
        return Err("Usage: exact_initial_accounting <recipe.json | ->".into());
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
    fn report_replays_and_rejects_invalid_inputs() {
        let report = run(RECIPE.as_bytes()).unwrap();
        assert_eq!(report, run(RECIPE.as_bytes()).unwrap());
        assert_eq!(
            report["accounting"]["accountingVersion"],
            "exact-initial-accounting-1"
        );
        assert!(report["accounting"]["reconciliationUnits"].is_string());
        assert!(run(&b"{}"[..]).is_err());
        assert!(run(vec![b' '; 32_769].as_slice()).is_err());
        assert!(run(RECIPE.replace("basins-1", "drainage-1").as_bytes()).is_err());
    }
}
