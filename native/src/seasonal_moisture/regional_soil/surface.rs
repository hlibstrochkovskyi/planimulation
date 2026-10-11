//! The production paired face stage, shared with isolated verification.
use super::{Mass, ResolutionBudget, cycle::move_water, surface_flow::Layout, surface_gradient};

// Standalone verification selections, not seasonal or persistence selectors.
#[derive(Clone, Copy, PartialEq)]
pub(super) enum VerificationStage {
    LegacyDirectional,
    FullGradient,
    FlatCenteredDepth,
    FlatIncidentMobility,
}

pub(super) struct Stocks<'a> {
    pub by_region: &'a [Option<usize>],
    pub liquid: &'a mut [Mass],
    pub bodies: &'a mut [Mass],
    pub transfers: &'a mut [Mass],
    pub resolution: &'a mut Option<ResolutionBudget>,
}

#[derive(Clone, Copy, PartialEq)]
enum CoefficientRule {
    SourceDepth,
    CenteredDepth,
    IncidentMobility,
}

// Callers own validation/atomic publication. Keep canonical face order, frozen
// leading depths and transfer arithmetic identical to the seasonal stage.
pub(super) fn advance(layout: &Layout, stocks: Stocks<'_>, seconds: f64) -> Result<usize, String> {
    advance_with_gradient(layout, None, CoefficientRule::SourceDepth, stocks, seconds)
}

// Standalone explicit candidate only: no existing seasonal/save selector reaches it.
pub(super) fn advance_full_gradient(
    prepared: &surface_gradient::Prepared,
    stocks: Stocks<'_>,
    seconds: f64,
) -> Result<usize, String> {
    advance_with_gradient(
        prepared.layout(),
        Some(prepared),
        CoefficientRule::SourceDepth,
        stocks,
        seconds,
    )
}

// Only the validated flat/closed wrapper may call this coefficient experiment.
pub(super) fn advance_flat_centered(
    prepared: &surface_gradient::Prepared,
    stocks: Stocks<'_>,
    seconds: f64,
) -> Result<usize, String> {
    advance_with_gradient(
        prepared.layout(),
        Some(prepared),
        CoefficientRule::CenteredDepth,
        stocks,
        seconds,
    )
}

pub(super) fn advance_flat_incident(
    prepared: &surface_gradient::Prepared,
    stocks: Stocks<'_>,
    seconds: f64,
) -> Result<usize, String> {
    advance_with_gradient(
        prepared.layout(),
        Some(prepared),
        CoefficientRule::IncidentMobility,
        stocks,
        seconds,
    )
}

fn advance_with_gradient(
    layout: &Layout,
    gradient: Option<&surface_gradient::Prepared>,
    coefficient: CoefficientRule,
    stocks: Stocks<'_>,
    seconds: f64,
) -> Result<usize, String> {
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
        let slopes = if coefficient == CoefficientRule::IncidentMobility {
            None
        } else {
            gradient.map(|g| g.edge_slopes(&depth)).transpose()?
        };
        let mobilities = if coefficient == CoefficientRule::IncidentMobility {
            Some(
                gradient
                    .ok_or("Missing incident-mobility geometry.")?
                    .incident_mobilities(&depth)?,
            )
        } else {
            None
        };
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
            let beta = mobilities.as_ref().map_or_else(
                || {
                    let slope = slopes
                        .as_ref()
                        .map_or_else(|| drive / face.distance_meters, |s| s[face_id]);
                    let interface_depth = if coefficient == CoefficientRule::CenteredDepth {
                        0.5 * depth[source] + 0.5 * depth[target]
                    } else {
                        over_crest
                    };
                    (interface_depth.powf(5. / 3.) / (layout.settings.roughness * slope.sqrt()))
                        .min(layout.settings.maximum_diffusivity_square_meters_per_second)
                },
                |m| m[face_id],
            );
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
