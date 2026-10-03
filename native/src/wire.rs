use crate::{World, prescribed_water_inventory::WaterDisplay};
use serde_json::json;
use std::io::{self, Write};

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
