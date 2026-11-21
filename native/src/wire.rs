use crate::{
    World, prescribed_water_inventory::WaterDisplay, seasonal_temperature::Normals,
    seasonal_wind::Normals as WindNormals,
};
use serde_json::json;
use std::io::{self, Write};

mod regional_water;
mod soil_water;
pub use regional_water::{
    regional_moisture, regional_moisture_checkpoint, regional_moisture_settings,
};
pub use soil_water::{soil_moisture, soil_moisture_checkpoint, soil_moisture_settings};

pub const MAX_SEASONAL_CHECKPOINT_BYTES: usize = 64 * 1024 * 1024;

fn f64s(out: &mut Vec<u8>, values: impl Iterator<Item = f64>) {
    for v in values {
        out.extend_from_slice(&v.to_le_bytes());
    }
}
fn u32s(out: &mut Vec<u8>, values: &[u32]) {
    for v in values {
        out.extend_from_slice(&v.to_le_bytes());
    }
}
pub fn arrays(w: &World) -> Vec<u8> {
    let s = &w.surface;
    let mut out = Vec::new();
    f64s(&mut out, s.centers.iter().flatten().copied());
    u32s(&mut out, &s.faces);
    u32s(&mut out, &s.offsets);
    u32s(&mut out, &s.neighbors);
    f64s(&mut out, s.distances.iter().copied());
    f64s(&mut out, s.areas.iter().copied());
    u32s(&mut out, &s.boundary_offsets);
    f64s(&mut out, s.boundaries.iter().flatten().copied());
    f64s(&mut out, w.field.iter().copied());
    let t = &w.tectonics;
    u32s(&mut out, &t.owners);
    u32s(&mut out, &t.seeds);
    f64s(&mut out, t.angular_velocities.iter().flatten().copied());
    u32s(&mut out, &t.boundary_cells);
    f64s(&mut out, t.boundary_directions.iter().flatten().copied());
    f64s(&mut out, t.boundary_motion.iter().copied());
    u32s(&mut out, &t.boundary_types);
    let c = &w.crust;
    f64s(&mut out, std::iter::once(c.threshold));
    f64s(&mut out, c.potential.iter().copied());
    f64s(&mut out, c.continentality.iter().copied());
    f64s(&mut out, c.thickness_meters.iter().copied());
    f64s(&mut out, c.density_kg_per_cubic_meter.iter().copied());
    let t = &w.terrain;
    for field in [&t.baseline, &t.convergence, &t.divergence, &t.detail] {
        f64s(&mut out, field.iter().copied());
    }
    if w.recipe.terrain_preparation_passes.is_some() {
        f64s(&mut out, t.preparation.iter().copied());
    }
    if w.recipe.model_version == crate::coastal_lowlands::MODEL_VERSION {
        f64s(&mut out, t.lowland.iter().copied());
    }
    f64s(&mut out, t.elevation.iter().copied());
    f64s(
        &mut out,
        [w.water.level_meters, w.water.resolved_volume_cubic_meters].into_iter(),
    );
    f64s(&mut out, w.water.depth_meters.iter().copied());
    u32s(&mut out, &w.water.body_ids);
    u32s(&mut out, &[w.water.main_ocean_id]);
    u32s(&mut out, &w.drainage.receivers);
    u32s(&mut out, &w.drainage.outlets);
    u32s(&mut out, &w.drainage.flat_steps);
    f64s(&mut out, w.drainage.contributing_area.iter().copied());
    let nodes = w.basins.nodes();
    u32s(
        &mut out,
        &w.basins
            .region_nodes()
            .iter()
            .map(|&id| id as u32)
            .collect::<Vec<_>>(),
    );
    u32s(
        &mut out,
        &nodes
            .iter()
            .enumerate()
            .map(|(id, node)| node.parent.unwrap_or(id) as u32)
            .collect::<Vec<_>>(),
    );
    f64s(&mut out, nodes.iter().map(|node| node.birth_level_meters));
    f64s(
        &mut out,
        nodes
            .iter()
            .map(|node| node.spill_level_meters.unwrap_or(node.birth_level_meters)),
    );
    for endpoint in 0..2 {
        u32s(
            &mut out,
            &nodes
                .iter()
                .map(|node| {
                    node.spill_edge
                        .map_or(s.areas.len() as u32, |edge| edge[endpoint])
                })
                .collect::<Vec<_>>(),
        );
    }
    f64s(
        &mut out,
        nodes.iter().map(|node| node.support_area_square_meters),
    );
    f64s(
        &mut out,
        nodes
            .iter()
            .map(|node| node.capacity_cubic_meters.unwrap_or(0.)),
    );
    out
}
/// v9: adds exact prescribed-water checkpoint transfer; world arrays are unchanged.
pub fn send(out: &mut impl Write, header: serde_json::Value, bytes: &[u8]) -> io::Result<()> {
    send_versioned(out, header, bytes, 9)
}
/// Export the complete resumable state, never the eighteen-field display frame.
pub fn seasonal_checkpoint(
    out: &mut impl Write,
    model: &crate::seasonal_moisture::Model,
    state: &crate::seasonal_moisture::State,
) -> Result<(), String> {
    if model.settings().reference_water_pool.is_some() {
        return Err(
            "Reference-body exchange is headless-only; no desktop checkpoint protocol is defined."
                .into(),
        );
    }
    if model.settings().soil_numerics.is_some() && model.settings().terminal_numerics.is_none() {
        return Err("The compensated-soil candidate is headless-only; no desktop checkpoint protocol is defined.".into());
    }
    model.budget(state)?;
    let bytes = serde_json::to_vec(&state.checkpoint()).map_err(|e| e.to_string())?;
    // Reserve one byte for the desktop file's final newline.
    if bytes.len() >= MAX_SEASONAL_CHECKPOINT_BYTES {
        return Err("Seasonal checkpoint exceeds 64 MiB.".into());
    }
    send_versioned(
        out,
        json!({"kind":if model.settings().surface_numerics.is_some() { "preciseMoistureCheckpoint" } else if model.settings().orography.is_some() { "orographicMoistureCheckpoint" } else { "moistureCheckpoint" }, "schemaVersion":model.checkpoint_schema_version(),
        "modelVersion":model.model_version(),
        "elapsedSeconds":state.elapsed_seconds()}),
        &bytes,
        if model.settings().surface_numerics.is_some() { 13 } else if model.settings().orography.is_some() { 12 } else { 11 },
    )
    .map_err(|e| e.to_string())
}
fn send_versioned(
    out: &mut impl Write,
    header: serde_json::Value,
    bytes: &[u8],
    protocol: u32,
) -> io::Result<()> {
    let mut header = header;
    header["protocol"] = json!(protocol);
    header["byteLength"] = json!(bytes.len());
    let encoded = serde_json::to_vec(&header)?;
    out.write_all(&(encoded.len() as u32).to_le_bytes())?;
    out.write_all(&encoded)?;
    out.write_all(bytes)?;
    out.flush()
}
pub fn snapshot(out: &mut impl Write, w: &World) -> io::Result<()> {
    let bytes = arrays(w);
    // Native fingerprints intentionally use a new version and serialization.
    let mut fingerprint = serde_json::to_vec(&w.recipe)?;
    fingerprint.extend_from_slice(&bytes);
    let total: f64 = w.surface.areas.iter().sum();
    let prepared = w.recipe.terrain_preparation_passes.is_some();
    let mut header = json!({"kind":"world", "recipe":w.recipe,
        "boundarySegmentCount":w.tectonics.boundary_types.len(),
        "basinNodeCount":w.basins.nodes().len(), "basinAnalysisVersion":crate::basins::ANALYSIS_VERSION,
        "checksum":format!("{:08x}",crate::hash(&fingerprint)),
        "stats": {"regionCount": w.field.len(), "faceCount": w.surface.faces.len()/3,
          "edgeCount":w.surface.neighbors.len()/2, "totalAreaSquareMeters":total,
          "relativeAreaError":(total/(4.*std::f64::consts::PI*w.recipe.radius_meters.powi(2))-1.).abs(),
          "minimumAreaSquareMeters":w.surface.areas.iter().copied().fold(f64::INFINITY,f64::min),
          "maximumAreaSquareMeters":w.surface.areas.iter().copied().fold(0.,f64::max),
          "arrayBytes": bytes.len()}});
    if prepared {
        header["terrainPreparation"] = json!({"appliedPasses":w.terrain.applied_passes,
            "transportedCubicMeters":w.terrain.transported_cubic_meters});
    }
    send_versioned(out, header, &bytes, if prepared { 10 } else { 9 })
}
pub fn frame(out: &mut impl Write, w: &World) -> io::Result<()> {
    let mut bytes = Vec::with_capacity(w.field.len() * 8);
    f64s(&mut bytes, w.field.iter().copied());
    send(
        out,
        json!({"kind":"frame","tick":w.tick,
        "relativeMassError":(w.mass()/w.initial_mass-1.).abs()}),
        &bytes,
    )
}

/// Seasonal normals are a separate read-only product, never part of the initial-world fingerprint.
pub fn seasonal_temperature(out: &mut impl Write, normals: &Normals) -> io::Result<()> {
    let count = normals.annual_mean_celsius.len();
    let mut bytes = Vec::with_capacity(count * 15 * 8);
    for month in &normals.monthly_temperature_celsius {
        f64s(&mut bytes, month.iter().copied());
    }
    for field in [
        &normals.annual_mean_celsius,
        &normals.annual_minimum_celsius,
        &normals.annual_maximum_celsius,
    ] {
        f64s(&mut bytes, field.iter().copied());
    }
    send(
        out,
        json!({"kind":"temperature", "temperatureModelVersion": normals.model_version,
        "settings":normals.settings, "daysPerYear":crate::seasonal_temperature::DAYS_PER_YEAR,
        "monthlyDayCounts":normals.monthly_day_counts, "regionCount":count}),
        &bytes,
    )
}

/// Prescribed wind vectors are a separate read-only product on the initial sphere.
pub fn seasonal_wind(out: &mut impl Write, normals: &WindNormals) -> io::Result<()> {
    let count = normals.monthly_east_meters_per_second[0].len();
    let mut bytes = Vec::with_capacity(count * 24 * 8);
    for month in &normals.monthly_east_meters_per_second {
        f64s(&mut bytes, month.iter().copied());
    }
    for month in &normals.monthly_north_meters_per_second {
        f64s(&mut bytes, month.iter().copied());
    }
    send(
        out,
        json!({"kind":"wind", "windModelVersion":normals.model_version,
        "temperatureModelVersion":normals.temperature_model_version,
        "axialTiltDegrees":normals.axial_tilt_degrees, "settings":normals.settings,
        "daysPerYear":crate::seasonal_temperature::DAYS_PER_YEAR,
        "monthlyDayCounts":normals.monthly_day_counts, "regionCount":count}),
        &bytes,
    )
}

/// Prescribed climate normals and climate-class biomes; read-only, never part of the world fingerprint.
pub fn climate_normals(
    out: &mut impl Write,
    normals: &crate::climate_normals::Normals,
) -> io::Result<()> {
    let count = normals.biome.len();
    let mut bytes = Vec::with_capacity(count * 68);
    for field in [
        &normals.annual_mean_celsius,
        &normals.warmest_month_celsius,
        &normals.coldest_month_celsius,
        &normals.annual_precipitation_mm,
        &normals.driest_month_mm,
        &normals.zonal_precipitation_mm,
        &normals.moisture_supply,
        &normals.lift,
    ] {
        f64s(&mut bytes, field.iter().copied());
    }
    u32s(
        &mut bytes,
        &normals
            .biome
            .iter()
            .map(|&b| u32::from(b))
            .collect::<Vec<_>>(),
    );
    send(
        out,
        json!({"kind":"climate", "climateModelVersion":normals.model_version, "regionCount":count}),
        &bytes,
    )
}

/// Climate-normal runoff and discharge, including sparse analytical spill records.
pub fn rivers(out: &mut impl Write, frame: &crate::rivers::Rivers) -> io::Result<()> {
    let count = frame.runoff_mm.len();
    let mut bytes =
        Vec::with_capacity(count * 240 + frame.lakes.len() * 76 + frame.spill_segments.len() * 20);
    for field in [
        &frame.pet_mm,
        &frame.aet_mm,
        &frame.runoff_mm,
        &frame.storage_mm,
        &frame.mean_discharge,
    ] {
        f64s(&mut bytes, field.iter().copied());
    }
    for field in [&frame.monthly_runoff_mm, &frame.monthly_discharge] {
        f64s(&mut bytes, field.iter().flatten().copied());
    }
    u32s(&mut bytes, &frame.class);
    u32s(&mut bytes, &frame.terminal);
    for lake in &frame.lakes {
        let [from, to] = lake.outlet.unwrap_or([count as u32; 2]);
        u32s(&mut bytes, &[lake.region, from, to]);
        f64s(
            &mut bytes,
            [
                lake.sill_elevation,
                lake.area,
                lake.inflow,
                lake.precipitation,
                lake.potential_evaporation,
                lake.evaporation,
                lake.spill,
                lake.storage,
            ]
            .into_iter(),
        );
    }
    for segment in &frame.spill_segments {
        u32s(&mut bytes, &[segment.from, segment.to, segment.class]);
        f64s(&mut bytes, std::iter::once(segment.mean_discharge));
    }
    send(
        out,
        json!({
            "kind": "rivers", "riverModelVersion": crate::rivers::MODEL_VERSION,
            "climateModelVersion": crate::climate_normals::MODEL_VERSION,
            "settings": frame.settings, "monthDays": frame.month_days, "regionCount": count,
            "oceanTerminal": frame.ocean_terminal, "lakeCount": frame.lakes.len(),
            "spillSegmentCount": frame.spill_segments.len(), "totals": frame.totals,
        }),
        &bytes,
    )
}

/// Vegetation display frame: seven per-region f64 fields, labels and the yearly
/// global summaries.
pub fn vegetation(
    out: &mut impl Write,
    model: &crate::vegetation::Model,
    state: &crate::vegetation::State,
) -> io::Result<()> {
    let count = state.label.len();
    let (woody_capacity, cover_capacity) = model.capacities(state);
    let productivity = model.productivities(state);
    let mut bytes = Vec::with_capacity(count * 76 + state.summaries.len() * 48);
    for field in [
        &state.woody,
        &state.herb,
        &state.precipitation_factor,
        &state.root_moisture,
        &state.established_cover,
        &woody_capacity,
        &cover_capacity,
        &state.woody_change,
        &productivity,
    ] {
        f64s(&mut bytes, field.iter().copied());
    }
    u32s(
        &mut bytes,
        &state
            .label
            .iter()
            .map(|&l| u32::from(l))
            .collect::<Vec<_>>(),
    );
    for summary in &state.summaries {
        f64s(
            &mut bytes,
            [
                summary.precipitation_factor,
                summary.woody_cover,
                summary.total_cover,
                summary.forest_share,
                f64::from(summary.label_changes),
                summary.productivity_petagrams,
            ]
            .into_iter(),
        );
    }
    send(
        out,
        json!({"kind":"vegetation", "vegetationModelVersion":crate::vegetation::MODEL_VERSION,
            "climateModelVersion":crate::climate_normals::MODEL_VERSION, "settings":model.settings(),
            "year":state.year, "regionCount":count}),
        &bytes,
    )
}

/// One region replayed from year 0: nine f64 values per year.
pub fn vegetation_series(
    out: &mut impl Write,
    region: usize,
    series: &[crate::vegetation::RegionYear],
) -> io::Result<()> {
    let mut bytes = Vec::with_capacity(series.len() * 72);
    for y in series {
        f64s(
            &mut bytes,
            [
                y.precipitation_factor,
                y.root_moisture,
                y.established_cover,
                y.woody,
                y.herb,
                y.woody_capacity,
                y.cover_capacity,
                y.productivity,
                f64::from(y.label),
            ]
            .into_iter(),
        );
    }
    send(
        out,
        json!({"kind":"vegetationSeries", "vegetationModelVersion":crate::vegetation::MODEL_VERSION,
            "region":region, "year":series.len() - 1}),
        &bytes,
    )
}

/// Wild-food display frame: seven per-region f64 fields, lean months and the yearly
/// land summaries.
pub fn wild_food(
    out: &mut impl Write,
    model: &crate::wild_food::Model,
    state: &crate::wild_food::State,
    regions: &[crate::wild_food::RegionFood],
) -> io::Result<()> {
    let mut bytes = Vec::with_capacity(regions.len() * 60 + state.summaries.len() * 32);
    let fields: [fn(&crate::wild_food::RegionFood) -> f64; 7] = [
        |r| r.capacity,
        |r| r.stored_capacity,
        |r| r.plant_kcal,
        |r| r.game_kcal,
        |r| r.game,
        |r| r.game_capacity,
        |r| r.access,
    ];
    for field in fields {
        f64s(&mut bytes, regions.iter().map(field));
    }
    u32s(
        &mut bytes,
        &regions
            .iter()
            .map(|r| u32::from(r.lean_month))
            .collect::<Vec<_>>(),
    );
    for s in &state.summaries {
        f64s(
            &mut bytes,
            [s.capacity_people, s.plant_kcal, s.game_kcal, s.game_ratio].into_iter(),
        );
    }
    send(
        out,
        json!({"kind":"wildFood", "wildFoodModelVersion":crate::wild_food::MODEL_VERSION,
            "vegetationModelVersion":crate::vegetation::MODEL_VERSION, "settings":model.settings(),
            "year":state.year, "regionCount":regions.len()}),
        &bytes,
    )
}

/// One region's wild food from year 0: eight f64 values and twelve monthly plant shares
/// per year.
pub fn wild_food_series(
    out: &mut impl Write,
    region: usize,
    series: &[crate::wild_food::RegionFood],
) -> io::Result<()> {
    let mut bytes = Vec::with_capacity(series.len() * 160);
    for y in series {
        f64s(
            &mut bytes,
            [
                y.capacity,
                y.stored_capacity,
                y.plant_kcal,
                y.game_kcal,
                y.game,
                y.game_capacity,
                y.access,
                f64::from(y.lean_month),
            ]
            .into_iter()
            .chain(y.season),
        );
    }
    send(
        out,
        json!({"kind":"wildFoodSeries", "wildFoodModelVersion":crate::wild_food::MODEL_VERSION,
            "region":region, "year":series.len() - 1}),
        &bytes,
    )
}

/// A foraging experiment in one region: game stock and unmet need per year.
pub fn forage(
    out: &mut impl Write,
    region: usize,
    people: f64,
    years: u32,
    rows: &[crate::wild_food::ForageYear],
) -> io::Result<()> {
    let mut bytes = Vec::with_capacity(rows.len() * 16);
    f64s(&mut bytes, rows.iter().flat_map(|r| [r.game, r.shortfall]));
    send(
        out,
        json!({"kind":"forage", "wildFoodModelVersion":crate::wild_food::MODEL_VERSION,
            "region":region, "people":people, "years":years, "year":rows.len() - 1}),
        &bytes,
    )
}

pub fn water_frame(
    out: &mut impl Write,
    display: &WaterDisplay,
    step: u64,
    input_units: i128,
    accepted_input_units: &str,
) -> io::Result<()> {
    let mut bytes = Vec::with_capacity(display.depth_meters.len() * 20);
    f64s(&mut bytes, display.depth_meters.iter().copied());
    f64s(&mut bytes, display.surface_levels_meters.iter().copied());
    u32s(&mut bytes, &display.body_ids);
    send(
        out,
        json!({"kind":"water", "step":step, "inputUnits":input_units.to_string(),
            "acceptedInputUnits":accepted_input_units, "mainOceanId":display.main_ocean_id}),
        &bytes,
    )
}

/// Protocols 11/12/13 display finite seasonal water, not lake geometry
/// or a complete checkpoint. Initial world and existing frame layouts stay unchanged.
pub fn seasonal_moisture(
    out: &mut impl Write,
    model: &crate::seasonal_moisture::Model,
    state: &crate::seasonal_moisture::State,
    step: Option<&crate::seasonal_moisture::Step>,
    interval_seconds: u32,
) -> Result<(), String> {
    if model.settings().reference_water_pool.is_some() {
        return Err(
            "Reference-body exchange is headless-only; no desktop display protocol is defined."
                .into(),
        );
    }
    if model.settings().soil_numerics.is_some() && model.settings().terminal_numerics.is_none() {
        return Err("The compensated-soil candidate is headless-only; no desktop display protocol is defined.".into());
    }
    if interval_seconds > 86400
        || u64::from(interval_seconds) > state.elapsed_seconds()
        || step.is_some() != (interval_seconds > 0)
        || step.is_some_and(|s| {
            s.surface_transfers.len() != state.surface_kilograms().len()
                || s.runoff_transfers.len() != state.surface_kilograms().len()
        })
    {
        return Err("Invalid seasonal-water display interval or transfer shape.".into());
    }
    let budget = model.budget(state)?;
    let count = state.surface_kilograms().len();
    let mut bytes = Vec::with_capacity(count * 18 * 8);
    for field in [
        state.surface_kilograms(),
        state.snow_kilograms(),
        state.soil_kilograms(),
        state.pending_runoff_kilograms(),
        state.terminal_water_kilograms(),
        state.vapor_kilograms(),
    ] {
        f64s(&mut bytes, field.iter().copied());
    }
    // Eight surface and four runoff transfers, field-major in their canonical
    // values() order. A zero-second observation has no interval flow records.
    for component in 0..8 {
        f64s(
            &mut bytes,
            (0..count).map(|i| step.map_or(0., |s| s.surface_transfers[i].values()[component])),
        );
    }
    for component in 0..4 {
        f64s(
            &mut bytes,
            (0..count).map(|i| step.map_or(0., |s| s.runoff_transfers[i].values()[component])),
        );
    }
    let mut header = json!({
        "kind":if model.settings().surface_numerics.is_some() { "preciseMoisture" } else if model.settings().orography.is_some() { "orographicMoisture" } else { "moisture" }, "regionCount":count,
        "modelVersion":model.model_version(),
        "surfaceModelVersion":model.surface_model_version(),
        "runoffModelVersion":crate::runoff_transport::MODEL_VERSION,
        "transportModelVersion":crate::moisture_transport::MODEL_VERSION,
        "temperatureModelVersion":crate::seasonal_temperature::MODEL_VERSION,
        "windModelVersion":crate::seasonal_wind::MODEL_VERSION,
        "settings":model.settings(), "temperatureSettings":model.temperature_settings(),
        "windSettings":model.wind_settings(),
        "elapsedSeconds":state.elapsed_seconds(), "intervalSeconds":interval_seconds,
        "coupledSubsteps":step.map_or(0, |s| s.coupled_substeps),
        "transportSubsteps":step.map_or(0, |s| s.transport_substeps),
        "maximumAbsoluteLocalExchangeResidualKilograms":step.map_or(0., |s| s.maximum_absolute_local_exchange_residual_kilograms),
        "maximumAbsoluteRoutingResidualKilograms":step.map_or(0., |s| s.maximum_absolute_routing_residual_kilograms),
        "budget":budget,
    });
    if model.settings().orography.is_some() {
        header["orographicModelVersion"] = json!(crate::orographic_response::MODEL_VERSION);
    }
    if model.settings().terminal_numerics.is_some() {
        header["terminalStockModelVersion"] =
            json!(crate::seasonal_moisture::TERMINAL_STOCK_MODEL_VERSION);
    }
    send_versioned(
        out,
        header,
        &bytes,
        if model.settings().surface_numerics.is_some() {
            13
        } else if model.settings().orography.is_some() {
            12
        } else {
            11
        },
    )
    .map_err(|e| e.to_string())
}
