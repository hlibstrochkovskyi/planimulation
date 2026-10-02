//! Bounded, area-accounted dry slope relaxation before water fitting.
//! This kernel is not yet part of the versioned generated-world pipeline.
use crate::Surface;

pub const MAX_PASSES: u32 = 16;
pub const SLOPE_THRESHOLD: f64 = 0.004;
pub const RELAXATION_FRACTION: f64 = 0.25;
pub const MAX_CHANGE_PER_PASS_METERS: f64 = 100.;

#[derive(Clone, Debug, PartialEq)]
pub struct PreparedTerrain {
    pub elevation_meters: Vec<f64>,
    pub eroded_meters: Vec<f64>,
    pub deposited_meters: Vec<f64>,
    pub transported_cubic_meters: f64,
    pub requested_passes: u32,
    pub applied_passes: u32,
}

#[derive(Clone, Copy)]
struct Transfer {
    source: usize,
    recipient: usize,
    requested_cubic_meters: f64,
}

/// Redistribute a reference-area-equivalent bed volume along steep edges.
/// Every pass reads one immutable height snapshot and applies simultaneous
/// transfers. No water, climate, density stratification, or geological time
/// is inferred from the pass count.
pub fn prepare(
    surface: &Surface,
    initial_elevation_meters: &[f64],
    passes: u32,
) -> Result<PreparedTerrain, String> {
    let n = surface.centers.len();
    if passes > MAX_PASSES
        || initial_elevation_meters.len() != n
        || surface.areas.len() != n
        || surface.offsets.len() != n + 1
        || surface.neighbors.len() != surface.distances.len()
        || surface.offsets.first() != Some(&0)
        || surface.offsets.last().copied() != Some(surface.neighbors.len() as u32)
        || initial_elevation_meters.iter().any(|h| !h.is_finite())
        || surface
            .areas
            .iter()
            .any(|area| !area.is_finite() || *area <= 0.)
        || surface
            .distances
            .iter()
            .any(|distance| !distance.is_finite() || *distance <= 0.)
    {
        return Err("Invalid terrain-preparation input or pass count.".into());
    }
    for source in 0..n {
        let start = surface.offsets[source] as usize;
        let end = surface.offsets[source + 1] as usize;
        if start > end || end > surface.neighbors.len() {
            return Err("Invalid terrain-preparation neighbor offsets.".into());
        }
    }
    for source in 0..n {
        for edge in surface.offsets[source] as usize..surface.offsets[source + 1] as usize {
            let neighbor = surface.neighbors[edge] as usize;
            if neighbor >= n || neighbor == source {
                return Err("Invalid terrain-preparation neighbor.".into());
            }
            let matching = (surface.offsets[neighbor] as usize
                ..surface.offsets[neighbor + 1] as usize)
                .filter(|&reverse| {
                    surface.neighbors[reverse] as usize == source
                        && (surface.distances[reverse] - surface.distances[edge]).abs()
                            <= 1e-12 * surface.distances[edge].max(1.)
                })
                .count();
            if matching != 1 {
                return Err("Terrain-preparation edges must have one matching reverse.".into());
            }
        }
    }
    let mut elevation = initial_elevation_meters.to_vec();
    let mut eroded = vec![0.; n];
    let mut deposited = vec![0.; n];
    let mut transported = 0.;
    let mut applied_passes = 0;
    for _ in 0..passes {
        let mut transfers = Vec::new();
        let mut outgoing = vec![0.; n];
        let mut incoming = vec![0.; n];
        for source in 0..n {
            let start = surface.offsets[source] as usize;
            let end = surface.offsets[source + 1] as usize;
            for edge in start..end {
                let neighbor = surface.neighbors[edge] as usize;
                if neighbor <= source {
                    continue;
                }
                let difference = elevation[source] - elevation[neighbor];
                let excess = difference.abs() - SLOPE_THRESHOLD * surface.distances[edge];
                if excess <= 0. {
                    continue;
                }
                let (donor, receiver) = if difference > 0. {
                    (source, neighbor)
                } else {
                    (neighbor, source)
                };
                let requested = RELAXATION_FRACTION * excess
                    / (1. / surface.areas[donor] + 1. / surface.areas[receiver]);
                if !requested.is_finite() || requested <= 0. {
                    return Err("Invalid terrain-preparation transfer.".into());
                }
                outgoing[donor] += requested;
                incoming[receiver] += requested;
                transfers.push(Transfer {
                    source: donor,
                    recipient: receiver,
                    requested_cubic_meters: requested,
                });
            }
        }
        if transfers.is_empty() {
            break;
        }
        if outgoing
            .iter()
            .chain(&incoming)
            .any(|volume| !volume.is_finite())
        {
            return Err("Terrain-preparation flow exceeded its numerical bound.".into());
        }
        applied_passes += 1;
        let mut change = vec![0.; n];
        for transfer in transfers {
            let source = transfer.source;
            let recipient = transfer.recipient;
            let source_cap =
                (MAX_CHANGE_PER_PASS_METERS * surface.areas[source] / outgoing[source]).min(1.);
            let recipient_cap = (MAX_CHANGE_PER_PASS_METERS * surface.areas[recipient]
                / incoming[recipient])
                .min(1.);
            let volume = transfer.requested_cubic_meters * source_cap.min(recipient_cap);
            let removal = volume / surface.areas[source];
            let addition = volume / surface.areas[recipient];
            change[source] -= removal;
            change[recipient] += addition;
            eroded[source] += removal;
            deposited[recipient] += addition;
            transported += volume;
        }
        for region in 0..n {
            elevation[region] += change[region];
            if !elevation[region].is_finite()
                || change[region].abs() > MAX_CHANGE_PER_PASS_METERS + 1e-9
            {
                return Err("Terrain-preparation update exceeded its bound.".into());
            }
        }
    }
    Ok(PreparedTerrain {
        elevation_meters: elevation,
        eroded_meters: eroded,
        deposited_meters: deposited,
        transported_cubic_meters: transported,
        requested_passes: passes,
        applied_passes,
    })
}
