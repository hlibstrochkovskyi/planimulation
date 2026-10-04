use crate::{
    World, prescribed_water_inventory::WaterDisplay, seasonal_temperature::Normals,
    seasonal_wind::Normals as WindNormals,
};
use serde_json::json;
use std::io::{self, Write};

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
    model.budget(state)?;
    let bytes = serde_json::to_vec(&state.checkpoint()).map_err(|e| e.to_string())?;
    // Reserve one byte for the desktop file's final newline.
    if bytes.len() >= MAX_SEASONAL_CHECKPOINT_BYTES {
        return Err("Seasonal checkpoint exceeds 64 MiB.".into());
    }
    send_versioned(
        out,
        json!({"kind":"moistureCheckpoint", "schemaVersion":3,
        "modelVersion":crate::seasonal_moisture::MODEL_VERSION,
        "elapsedSeconds":state.elapsed_seconds()}),
        &bytes,
        11,
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

/// Protocol 11 is a display snapshot of finite seasonal water, not lake geometry
/// or a complete checkpoint. Initial world and existing frame layouts stay unchanged.
pub fn seasonal_moisture(
    out: &mut impl Write,
    model: &crate::seasonal_moisture::Model,
    state: &crate::seasonal_moisture::State,
    step: Option<&crate::seasonal_moisture::Step>,
    interval_seconds: u32,
) -> Result<(), String> {
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
    send_versioned(out, json!({
        "kind":"moisture", "regionCount":count,
        "modelVersion":crate::seasonal_moisture::MODEL_VERSION,
        "surfaceModelVersion":crate::surface_water::MODEL_VERSION,
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
    }), &bytes, 11).map_err(|e| e.to_string())
}
