use planimulation_core::{
    Recipe, World, exact_initial_accounting::exact_units,
    prescribed_water_inventory::PrescribedWaterInventory, wire,
};
use serde::Deserialize;
use serde_json::json;
use std::io::{self, BufRead, Read};

#[derive(Deserialize)]
#[serde(tag = "command", rename_all = "camelCase", deny_unknown_fields)]
enum Command {
    Generate { recipe: Recipe },
    Advance { steps: u32 },
    PrescribeWater { region: usize, mode: WaterMode },
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
enum WaterMode {
    OneCubicKilometer,
    FillToSpill,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut input = io::stdin().lock();
    let mut output = io::stdout().lock();
    let mut world: Option<World> = None;
    let mut water_state: Option<PrescribedWaterInventory> = None;
    loop {
        let mut line = Vec::new();
        let count = input.by_ref().take(32769).read_until(b'\n', &mut line)?;
        if count == 0 {
            break;
        }
        if count > 32768 {
            return Err("Command exceeds 32 KiB".into());
        }
        let result = (|| -> Result<(), String> {
            let command: Command = serde_json::from_slice(&line).map_err(|e| e.to_string())?;
            match command {
                Command::Generate { recipe } => {
                    let next = World::generate(recipe)?;
                    wire::snapshot(&mut output, &next).map_err(|e| e.to_string())?;
                    world = Some(next);
                    water_state = None;
                }
                Command::Advance { steps } => {
                    let w = world.as_mut().ok_or("Generate a world first.")?;
                    w.advance(steps)?;
                    wire::frame(&mut output, w).map_err(|e| e.to_string())?;
                }
                Command::PrescribeWater { region, mode } => {
                    let w = world.as_ref().ok_or("Generate a world first.")?;
                    if region >= w.surface.areas.len() {
                        return Err("Runoff source is outside the generated world.".into());
                    }
                    if water_state.is_none() {
                        water_state = Some(PrescribedWaterInventory::from_world(w)?);
                    }
                    let state = water_state.as_mut().unwrap();
                    let one_km3 = exact_units(1_000_000_000.)?;
                    let units = match mode {
                        WaterMode::OneCubicKilometer => one_km3,
                        WaterMode::FillToSpill => state
                            .remaining_before_spill_units(state.runoff_branch(region)?)?
                            .ok_or("The closed global basin has no spill threshold.")?
                            .checked_add(one_km3)
                            .ok_or("Prescribed runoff amount overflowed.")?,
                    };
                    let previous = state.checkpoint();
                    let mut runoff = vec![0; w.surface.areas.len()];
                    runoff[region] = units;
                    let step = state.apply_runoff(&runoff)?;
                    let display = match state.display(w) {
                        Ok(display) => display,
                        Err(error) => {
                            *state = PrescribedWaterInventory::restore(previous)?;
                            return Err(error);
                        }
                    };
                    let checkpoint = state.checkpoint();
                    wire::water_frame(
                        &mut output,
                        &display,
                        checkpoint.step,
                        step.input_units,
                        &checkpoint.accepted_input_units,
                    )
                    .map_err(|e| e.to_string())?;
                }
            }
            Ok(())
        })();
        if let Err(error) = result {
            wire::send(&mut output, json!({"kind":"error","message":error}), &[])?;
        }
    }
    Ok(())
}
