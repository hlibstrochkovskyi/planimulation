//! The production paired face stage, shared with isolated verification.
use super::{Mass, ResolutionBudget, cycle::move_water, surface_flow::Layout};

pub(super) struct Stocks<'a> {
    pub by_region: &'a [Option<usize>],
    pub liquid: &'a mut [Mass],
    pub bodies: &'a mut [Mass],
    pub transfers: &'a mut [Mass],
    pub resolution: &'a mut Option<ResolutionBudget>,
}

// Callers own validation/atomic publication. Keep canonical face order, frozen
// leading depths and transfer arithmetic identical to the seasonal stage.
pub(super) fn advance(layout: &Layout, stocks: Stocks<'_>, seconds: f64) -> Result<usize, String> {
    let needed = (seconds / layout.stable_seconds).ceil().max(1.);
    if !needed.is_finite() || needed > 16384. {
        return Err("Paired surface flow exceeds its substep bound.".into());
    }
    let count = needed as usize;
    let dt = seconds / needed;
    for _ in 0..count {
        let depth: Vec<_> = layout
            .areas
            .iter()
            .enumerate()
            .map(|(r, &area)| {
                if layout.land[r] {
                    stocks.liquid[r].high / (1000. * area)
                } else {
                    (layout.reference_level - layout.beds[r]).max(0.)
                }
            })
            .collect();
        for (face_id, face) in layout.faces.iter().enumerate() {
            let [a, b] = face.regions;
            let difference = layout.beds[a] - layout.beds[b] + depth[a] - depth[b];
            let direction = usize::from(difference < 0.);
            let source = face.regions[direction];
            let target = face.regions[1 - direction];
            if !layout.land[source] || difference == 0. {
                continue;
            }
            let over_crest = depth[source] - (layout.beds[target] - layout.beds[source]).max(0.);
            let drive = difference.abs().min(over_crest);
            if drive <= 0. {
                continue;
            }
            let beta = (over_crest.powf(5. / 3.)
                / (layout.settings.roughness * (drive / face.distance_meters).sqrt()))
            .min(layout.settings.maximum_diffusivity_square_meters_per_second);
            let request = 1000. * dt * beta * face.width_over_distance * drive;
            if !request.is_finite() || request < 0. {
                return Err("Invalid paired surface-flow request.".into());
            }
            let mut donor = stocks.liquid[source];
            let recipient = if let Some(b) = stocks.by_region[target] {
                &mut stocks.bodies[b]
            } else {
                &mut stocks.liquid[target]
            };
            let (grant, deferred) = move_water(
                &mut donor,
                recipient,
                request,
                &mut stocks.transfers[2 * face_id + direction],
                stocks.resolution,
            )?;
            if grant != request && !deferred {
                return Err("Paired surface-flow request violates positivity.".into());
            }
            stocks.liquid[source] = donor;
        }
    }
    Ok(count)
}
