use planimulation_core::{
    Recipe, World,
    climate_normals::Normals as ClimateNormals,
    exact_initial_accounting::exact_units,
    prescribed_water_inventory::{Checkpoint, PrescribedWaterInventory},
    seasonal_moisture::{
        Checkpoint as MoistureCheckpoint, Model as MoistureModel, State as MoistureState,
    },
    seasonal_temperature::{Normals, Settings},
    seasonal_wind::{Normals as WindNormals, Settings as WindSettings},
    traversal::{Model as TraversalModel, Season as TraversalSeason},
    vegetation::{Model as VegetationModel, RegionYear, State as VegetationState},
    wild_food::{Model as WildFoodModel, State as WildFoodState},
    wire,
};
use serde::Deserialize;
use serde_json::json;
use std::io::{self, BufRead, Read};
mod seasonal_session;
use seasonal_session::SeasonalSession;

#[derive(Deserialize)]
#[serde(tag = "command", rename_all = "camelCase", deny_unknown_fields)]
enum Command {
    Generate {
        recipe: Recipe,
    },
    Advance {
        steps: u32,
    },
    PrescribeWater {
        region: usize,
        mode: WaterMode,
    },
    ExportWater,
    RestoreWater {
        checkpoint: Box<Checkpoint>,
    },
    SeasonalTemperature,
    SeasonalWind,
    ClimateNormals,
    InitializeVegetation,
    AdvanceVegetation {
        years: u32,
    },
    VegetationSeries {
        region: usize,
    },
    WildFood,
    WildFoodSeries {
        region: usize,
    },
    Forage {
        region: usize,
        people: f64,
        years: u32,
    },
    TraversalCosts,
    TravelTime {
        season: TraversalSeason,
        sources: Vec<usize>,
    },
    SeasonalMoisture {
        seconds: u32,
    },
    ExportMoisture,
    InitializeOrographicMoisture,
    InitializePreciseMoisture,
    InitializeRegionalMoisture,
    InitializeSoilMoisture,
    RestoreSoilMoisture {
        #[serde(rename = "checkpointJson")]
        checkpoint_json: String,
    },
    RestoreMoisture {
        #[serde(rename = "checkpointJson")]
        checkpoint_json: String,
    },
}

const MAX_COMMAND_BYTES: u64 = 8 * 1024 * 1024;
const MAX_SEASONAL_CHECKPOINT_BYTES: usize = wire::MAX_SEASONAL_CHECKPOINT_BYTES;
const MAX_SEASONAL_COMMAND_BYTES: u64 = 2 * MAX_SEASONAL_CHECKPOINT_BYTES as u64 + 1024;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
enum WaterMode {
    OneCubicKilometer,
    FillToSpill,
}

fn write_moisture(
    out: &mut impl std::io::Write,
    model: &MoistureModel,
    state: &MoistureState,
    step: Option<&planimulation_core::seasonal_moisture::Step>,
    seconds: u32,
) -> Result<(), String> {
    if model.model_version()
        == planimulation_core::seasonal_moisture::REGIONAL_SURFACE_MODEL_VERSION
    {
        wire::regional_moisture(out, model, state, step, seconds)
    } else {
        wire::seasonal_moisture(out, model, state, step, seconds)
    }
}

/// The replayed vegetation history of `region` at the current year, computed once.
fn cached_series<'a>(
    cache: &'a mut Option<(usize, u32, Vec<RegionYear>)>,
    model: &VegetationModel,
    state: &VegetationState,
    region: usize,
) -> Result<&'a [RegionYear], String> {
    if !matches!(cache, Some((r, y, _)) if *r == region && *y == state.year) {
        *cache = Some((region, state.year, model.region_series(state, region)?));
    }
    Ok(&cache.as_ref().expect("series was just cached").2)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut input = io::stdin().lock();
    let mut output = io::stdout().lock();
    let mut world: Option<World> = None;
    let mut water_state: Option<PrescribedWaterInventory> = None;
    let mut moisture_state: Option<SeasonalSession> = None;
    let mut vegetation: Option<(VegetationModel, VegetationState)> = None;
    // Wild food advances with vegetation; the last replayed region history is kept
    // because food series and foraging experiments reuse it.
    let mut food: Option<(WildFoodModel, WildFoodState)> = None;
    let mut vegetation_series: Option<(usize, u32, Vec<RegionYear>)> = None;
    // Climate normals of the current world, shared by the biome layers, vegetation and traversal.
    let mut climate: Option<ClimateNormals> = None;
    let mut traversal: Option<TraversalModel> = None;
    loop {
        let mut line = Vec::new();
        let count = input
            .by_ref()
            .take(MAX_SEASONAL_COMMAND_BYTES + 1)
            .read_until(b'\n', &mut line)?;
        if count == 0 {
            break;
        }
        if count as u64 > MAX_SEASONAL_COMMAND_BYTES {
            return Err("Command exceeds the seasonal restore envelope limit.".into());
        }
        let result = (|| -> Result<(), String> {
            let command: Command = serde_json::from_slice(&line).map_err(|e| e.to_string())?;
            if count as u64 > MAX_COMMAND_BYTES
                && !matches!(
                    command,
                    Command::RestoreMoisture { .. } | Command::RestoreSoilMoisture { .. }
                )
            {
                return Err("Command exceeds 8 MiB.".into());
            }
            match command {
                Command::Generate { recipe } => {
                    let next = World::generate(recipe)?;
                    wire::snapshot(&mut output, &next).map_err(|e| e.to_string())?;
                    world = Some(next);
                    water_state = None;
                    moisture_state = None;
                    vegetation = None;
                    food = None;
                    vegetation_series = None;
                    climate = None;
                    traversal = None;
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
                Command::ClimateNormals => {
                    let w = world.as_ref().ok_or("Generate a world first.")?;
                    if climate.is_none() {
                        climate = Some(ClimateNormals::from_world(w)?);
                    }
                    let normals = climate
                        .as_ref()
                        .expect("climate normals were just computed");
                    wire::climate_normals(&mut output, normals).map_err(|e| e.to_string())?;
                }
                Command::InitializeVegetation => {
                    let w = world.as_ref().ok_or("Generate a world first.")?;
                    if climate.is_none() {
                        climate = Some(ClimateNormals::from_world(w)?);
                    }
                    let normals = climate
                        .as_ref()
                        .expect("climate normals were just computed");
                    let (model, state) = VegetationModel::new(w, normals, Default::default())?;
                    let wild = WildFoodModel::new(w, normals, &model, &state, Default::default())?;
                    wire::vegetation(&mut output, &model, &state).map_err(|e| e.to_string())?;
                    vegetation = Some((model, state));
                    food = Some(wild);
                    vegetation_series = None;
                    traversal = None;
                }
                Command::AdvanceVegetation { years } => {
                    let (model, state) = vegetation
                        .as_mut()
                        .ok_or("Initialize vegetation before advancing it.")?;
                    let (food_model, food_state) = food
                        .as_mut()
                        .ok_or("Initialize vegetation before advancing it.")?;
                    use planimulation_core::vegetation::{
                        MAXIMUM_YEARS, MAXIMUM_YEARS_PER_REQUEST,
                    };
                    if years == 0 || years > MAXIMUM_YEARS_PER_REQUEST {
                        return Err(format!(
                            "Vegetation accepts 1–{MAXIMUM_YEARS_PER_REQUEST} years per request."
                        ));
                    }
                    if state.year + years > MAXIMUM_YEARS {
                        return Err(format!(
                            "Vegetation runs are limited to {MAXIMUM_YEARS} years."
                        ));
                    }
                    let (mut next, mut next_food) = (state.clone(), food_state.clone());
                    for _ in 0..years {
                        model.advance(&mut next, 1)?;
                        food_model.step(model, &next, &mut next_food)?;
                    }
                    *state = next;
                    *food_state = next_food;
                    vegetation_series = None;
                    traversal = None;
                    wire::vegetation(&mut output, model, state).map_err(|e| e.to_string())?;
                }
                Command::VegetationSeries { region } => {
                    let (model, state) = vegetation
                        .as_ref()
                        .ok_or("Initialize vegetation before inspecting it.")?;
                    let series = cached_series(&mut vegetation_series, model, state, region)?;
                    wire::vegetation_series(&mut output, region, series)
                        .map_err(|e| e.to_string())?;
                }
                Command::WildFood => {
                    let (model, state) = vegetation
                        .as_ref()
                        .ok_or("Initialize vegetation before reading wild food.")?;
                    let (food_model, food_state) = food
                        .as_ref()
                        .ok_or("Initialize vegetation before reading wild food.")?;
                    let regions = food_model.regions(model, state, food_state);
                    wire::wild_food(&mut output, food_model, food_state, &regions)
                        .map_err(|e| e.to_string())?;
                }
                Command::WildFoodSeries { region } => {
                    let (model, state) = vegetation
                        .as_ref()
                        .ok_or("Initialize vegetation before inspecting wild food.")?;
                    let (food_model, _) = food
                        .as_ref()
                        .ok_or("Initialize vegetation before inspecting wild food.")?;
                    let series = cached_series(&mut vegetation_series, model, state, region)?;
                    let foods = food_model.region_series(region, series)?;
                    wire::wild_food_series(&mut output, region, &foods)
                        .map_err(|e| e.to_string())?;
                }
                Command::Forage {
                    region,
                    people,
                    years,
                } => {
                    let (model, state) = vegetation
                        .as_ref()
                        .ok_or("Initialize vegetation before foraging.")?;
                    let (food_model, _) = food
                        .as_ref()
                        .ok_or("Initialize vegetation before foraging.")?;
                    let series = cached_series(&mut vegetation_series, model, state, region)?;
                    let rows = food_model.forage(region, series, people, years)?;
                    wire::forage(&mut output, region, people, years, &rows)
                        .map_err(|e| e.to_string())?;
                }
                command @ (Command::TraversalCosts | Command::TravelTime { .. }) => {
                    let w = world.as_ref().ok_or("Generate a world first.")?;
                    if traversal.is_none() {
                        let normals = match &mut climate {
                            Some(normals) => normals,
                            None => climate.insert(ClimateNormals::from_world(w)?),
                        };
                        traversal = Some(TraversalModel::new(
                            w,
                            normals,
                            vegetation.as_ref().map(|(_, state)| state),
                            Default::default(),
                        )?);
                    }
                    let model = traversal
                        .as_ref()
                        .expect("traversal model was just computed");
                    match command {
                        Command::TraversalCosts => wire::traversal_costs(&mut output, model),
                        Command::TravelTime { season, sources } => {
                            let field = model.travel_time_multi(season, &sources)?;
                            wire::traversal_field(&mut output, model, &field)
                        }
                        _ => unreachable!(),
                    }
                    .map_err(|e| e.to_string())?;
                }
                Command::SeasonalWind => {
                    let w = world.as_ref().ok_or("Generate a world first.")?;
                    let normals = WindNormals::from_world(
                        w,
                        WindSettings::default(),
                        Settings::default().axial_tilt_degrees,
                    )?;
                    wire::seasonal_wind(&mut output, &normals).map_err(|e| e.to_string())?;
                }
                Command::SeasonalMoisture { seconds } => {
                    let w = world.as_ref().ok_or("Generate a world first.")?;
                    if seconds > 86400 {
                        return Err("Seasonal water accepts 0–86400 seconds per request.".into());
                    }
                    if let Some(session) = &mut moisture_state {
                        session.advance(&mut output, seconds)?;
                    } else {
                        if water_state
                            .as_ref()
                            .is_some_and(|state| state.checkpoint().step > 0)
                        {
                            return Err("Regenerate before starting seasonal water after manual water input.".into());
                        }
                        if seconds != 0 {
                            return Err(
                                "Initialize seasonal water before advancing its clock.".into()
                            );
                        }
                        let model = MoistureModel::from_world(
                            w,
                            Default::default(),
                            Default::default(),
                            Default::default(),
                        )?;
                        let state = model.initial_state();
                        wire::seasonal_moisture(&mut output, &model, &state, None, 0)
                            .map_err(|e| e.to_string())?;
                        water_state = None;
                        moisture_state = Some(SeasonalSession::Legacy(model, state));
                    }
                }
                command @ (Command::InitializeOrographicMoisture
                | Command::InitializePreciseMoisture
                | Command::InitializeRegionalMoisture) => {
                    let precise = matches!(command, Command::InitializePreciseMoisture);
                    let regional = matches!(command, Command::InitializeRegionalMoisture);
                    let w = world.as_ref().ok_or("Generate a world first.")?;
                    if moisture_state.is_some()
                        || water_state
                            .as_ref()
                            .is_some_and(|s| s.checkpoint().step > 0)
                    {
                        return Err(
                            "Regenerate before choosing a different seasonal-water model.".into(),
                        );
                    }
                    let settings = if regional {
                        wire::regional_moisture_settings()
                    } else {
                        planimulation_core::seasonal_moisture::Settings {
                        orography: Some(Default::default()),
                        soil_numerics: precise.then_some(
                            planimulation_core::seasonal_moisture::SoilNumerics::Compensated,
                        ),
                        surface_numerics: precise.then_some(
                            planimulation_core::seasonal_moisture::SurfaceNumerics::Compensated,
                        ),
                        terminal_numerics: precise.then_some(
                            planimulation_core::seasonal_moisture::TerminalNumerics::Compensated,
                        ),
                        ..Default::default()
                    }
                    };
                    let model = MoistureModel::from_world(
                        w,
                        settings,
                        Default::default(),
                        Default::default(),
                    )?;
                    let state = model.initial_state();
                    write_moisture(&mut output, &model, &state, None, 0)?;
                    water_state = None;
                    moisture_state = Some(SeasonalSession::Legacy(model, state));
                }
                Command::ExportMoisture => {
                    let session = moisture_state
                        .as_ref()
                        .ok_or("Initialize seasonal water before saving it.")?;
                    session.export(&mut output)?;
                }
                Command::RestoreMoisture { checkpoint_json } => {
                    let w = world.as_ref().ok_or("Generate a world first.")?;
                    if moisture_state.is_some()
                        || water_state
                            .as_ref()
                            .is_some_and(|state| state.checkpoint().step > 0)
                    {
                        return Err(
                            "Restore seasonal water in a separate generated session.".into()
                        );
                    }
                    if checkpoint_json.len() > MAX_SEASONAL_CHECKPOINT_BYTES {
                        return Err("Seasonal checkpoint exceeds 64 MiB.".into());
                    }
                    let checkpoint: MoistureCheckpoint =
                        serde_json::from_str(&checkpoint_json).map_err(|e| e.to_string())?;
                    if checkpoint.recipe != w.recipe {
                        return Err(
                            "Seasonal checkpoint recipe does not match the generated world.".into(),
                        );
                    }
                    let desktop_settings = if checkpoint.model_version
                        == planimulation_core::seasonal_moisture::REGIONAL_SURFACE_MODEL_VERSION
                    {
                        wire::regional_moisture_settings()
                    } else {
                        planimulation_core::seasonal_moisture::Settings {
                            orography: checkpoint.settings.orography.map(|_| Default::default()),
                            soil_numerics: checkpoint.settings.surface_numerics.map(|_| {
                                planimulation_core::seasonal_moisture::SoilNumerics::Compensated
                            }),
                            surface_numerics: checkpoint.settings.surface_numerics,
                            terminal_numerics: checkpoint.settings.terminal_numerics,
                            ..Default::default()
                        }
                    };
                    if checkpoint.settings != desktop_settings
                        || checkpoint.temperature_settings != Default::default()
                        || checkpoint.wind_settings != Default::default()
                    {
                        return Err(
                            "Desktop seasonal checkpoints require the pinned default settings."
                                .into(),
                        );
                    }
                    let (model, state) = MoistureModel::restore(checkpoint)?;
                    write_moisture(&mut output, &model, &state, None, 0)?;
                    water_state = None;
                    moisture_state = Some(SeasonalSession::Legacy(model, state));
                }
                Command::InitializeSoilMoisture => {
                    let w = world.as_ref().ok_or("Generate a world first.")?;
                    if moisture_state.is_some()
                        || water_state
                            .as_ref()
                            .is_some_and(|s| s.checkpoint().step > 0)
                    {
                        return Err(
                            "Regenerate before choosing a different seasonal-water model.".into(),
                        );
                    }
                    let model =
                        planimulation_core::seasonal_moisture::regional_soil::Model::from_world(
                            w,
                            wire::soil_moisture_settings(),
                            Default::default(),
                            Default::default(),
                        )?;
                    let state = model.initial_state();
                    wire::soil_moisture(&mut output, &model, &state, None, None)?;
                    water_state = None;
                    moisture_state = Some(SeasonalSession::Soil(model, state));
                }
                Command::RestoreSoilMoisture { checkpoint_json } => {
                    let w = world.as_ref().ok_or("Generate a world first.")?;
                    if moisture_state.is_some()
                        || water_state
                            .as_ref()
                            .is_some_and(|s| s.checkpoint().step > 0)
                    {
                        return Err(
                            "Restore seasonal water in a separate generated session.".into()
                        );
                    }
                    if checkpoint_json.len() > MAX_SEASONAL_CHECKPOINT_BYTES {
                        return Err("Seasonal checkpoint exceeds 64 MiB.".into());
                    }
                    let cp: planimulation_core::seasonal_moisture::regional_soil::Checkpoint =
                        serde_json::from_str(&checkpoint_json).map_err(|e| e.to_string())?;
                    if cp.recipe != w.recipe
                        || cp.settings != wire::soil_moisture_settings()
                        || cp.temperature_settings != Default::default()
                        || cp.wind_settings != Default::default()
                    {
                        return Err("Soil-water checkpoint requires matching geography and pinned product settings.".into());
                    }
                    let (model, state) =
                        planimulation_core::seasonal_moisture::regional_soil::Model::restore(cp)?;
                    wire::soil_moisture(&mut output, &model, &state, None, None)?;
                    water_state = None;
                    moisture_state = Some(SeasonalSession::Soil(model, state));
                }
                Command::PrescribeWater { region, mode } => {
                    if moisture_state.is_some() {
                        return Err("Regenerate before switching seasonal water to the separate manual inventory.".into());
                    }
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
                    if moisture_state.is_some() {
                        return Err("A manual water checkpoint does not contain seasonal water; regenerate to use that inventory.".into());
                    }
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
                    if moisture_state.is_some() {
                        return Err("Restore manual water in a separate generated session.".into());
                    }
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
