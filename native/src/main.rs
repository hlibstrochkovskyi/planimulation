use planimulation_core::{
    Recipe, World,
    exact_initial_accounting::exact_units,
    prescribed_water_inventory::{Checkpoint, PrescribedWaterInventory},
    seasonal_temperature::{Normals, Settings},
    wire,
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
    ExportWater,
    RestoreWater { checkpoint: Box<Checkpoint> },
    SeasonalTemperature,
}

const MAX_COMMAND_BYTES: u64 = 8 * 1024 * 1024;

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
        let count = input
            .by_ref()
            .take(MAX_COMMAND_BYTES + 1)
            .read_until(b'\n', &mut line)?;
        if count == 0 {
            break;
        }
        if count as u64 > MAX_COMMAND_BYTES {
            return Err("Command exceeds 8 MiB".into());
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
                Command::SeasonalTemperature => {
                    let w = world.as_ref().ok_or("Generate a world first.")?;
                    let normals = Normals::from_world(w, Settings::default())?;
                    wire::seasonal_temperature(&mut output, &normals).map_err(|e| e.to_string())?;
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
                Command::ExportWater => {
                    let w = world.as_ref().ok_or("Generate a world first.")?;
                    if water_state.is_none() {
                        water_state = Some(PrescribedWaterInventory::from_world(w)?);
                    }
                    let bytes = serde_json::to_vec(&water_state.as_ref().unwrap().checkpoint())
                        .map_err(|e| e.to_string())?;
                    if bytes.len() > MAX_COMMAND_BYTES as usize {
                        return Err("Prescribed-water checkpoint exceeds 8 MiB.".into());
                    }
                    wire::send(&mut output, json!({"kind":"checkpoint"}), &bytes)
                        .map_err(|e| e.to_string())?;
                }
                Command::RestoreWater { checkpoint } => {
                    let w = world.as_ref().ok_or("Generate a world first.")?;
                    let restored = PrescribedWaterInventory::restore_on_world(*checkpoint, w)?;
                    let display = restored.display(w)?;
                    let saved = restored.checkpoint();
                    wire::water_frame(
                        &mut output,
                        &display,
                        saved.step,
                        0,
                        &saved.accepted_input_units,
                    )
                    .map_err(|e| e.to_string())?;
                    water_state = Some(restored);
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
