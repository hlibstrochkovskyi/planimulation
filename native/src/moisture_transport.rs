//! Conservative transport of a prescribed column-water stock on the native sphere.
//! No evaporation, condensation, air-mass solver, or surface-water exchange.
use crate::{Surface, add, area, cross, dot, norm, unit};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, f64::consts::PI};

pub const MODEL_VERSION: &str = "moisture-transport-1";

#[derive(Clone, Copy, Debug)]
pub struct BoundarySegment {
    pub midpoint: [f64; 3],
    /// Tangent unit conormal directed out of the lower-ID region.
    pub outward_normal: [f64; 3],
    pub arc_length_meters: f64,
    /// Integral weight 2 R sin(angle / 2), exact for rigid-rotation velocity.
    pub quadrature_weight_meters: f64,
}

#[derive(Clone, Debug)]
pub struct SharedBoundary {
    pub regions: [usize; 2],
    pub segments: [BoundarySegment; 2],
}

/// Two great-circle segments form each barycentric dual boundary.
/// Geometry is independent of longitude, rendering, terrain, and wind.
pub struct Geometry {
    areas_square_meters: Vec<f64>,
    boundaries: Vec<SharedBoundary>,
}

impl Geometry {
    pub fn from_surface(surface: &Surface, radius_meters: f64) -> Result<Self, String> {
        let n = surface.centers.len();
        if n < 4
            || !radius_meters.is_finite()
            || radius_meters <= 0.
            || surface.areas.len() != n
            || !surface.faces.len().is_multiple_of(3)
            || surface
                .centers
                .iter()
                .any(|&p| p.iter().any(|v| !v.is_finite()) || (norm(p) - 1.).abs() > 1e-12)
            || surface.areas.iter().any(|a| !a.is_finite() || *a <= 0.)
        {
            return Err("Invalid moisture-transport surface or radius.".into());
        }
        let mut contacts: BTreeMap<[usize; 2], Vec<[f64; 3]>> = BTreeMap::new();
        let mut expected_areas = vec![0.; n];
        for face in surface.faces.as_chunks::<3>().0 {
            let ids = [face[0] as usize, face[1] as usize, face[2] as usize];
            if ids.iter().any(|&id| id >= n)
                || ids[0] == ids[1]
                || ids[1] == ids[2]
                || ids[2] == ids[0]
            {
                return Err("Invalid moisture-transport face.".into());
            }
            let centers = ids.map(|id| surface.centers[id]);
            let centroid_sum = add(add(centers[0], centers[1]), centers[2]);
            if norm(centroid_sum) <= 1e-12 {
                return Err("Degenerate moisture-transport triangle.".into());
            }
            let centroid = unit(centroid_sum);
            for k in 0..3 {
                let i = ids[k];
                let j = ids[(k + 1) % 3];
                let midpoint_sum = add(surface.centers[i], surface.centers[j]);
                if norm(midpoint_sum) <= 1e-12 {
                    return Err("Antipodal moisture-transport neighbors.".into());
                }
                let midpoint = unit(midpoint_sum);
                expected_areas[i] += area(surface.centers[i], midpoint, centroid);
                expected_areas[j] += area(surface.centers[j], midpoint, centroid);
                contacts
                    .entry([i.min(j), i.max(j)])
                    .or_default()
                    .push(centroid);
            }
        }
        for (&solid_area, &physical_area) in expected_areas.iter().zip(&surface.areas) {
            let expected = solid_area * radius_meters * radius_meters;
            if !expected.is_finite()
                || expected <= 0.
                || (expected / physical_area - 1.).abs() > 1e-10
            {
                return Err(
                    "Moisture-transport area does not match dual geometry and radius.".into(),
                );
            }
        }
        let total = total_mass(&surface.areas);
        if (total / (4. * PI * radius_meters.powi(2)) - 1.).abs() > 1e-10 {
            return Err("Moisture transport requires a closed spherical surface.".into());
        }
        let mut boundaries = Vec::with_capacity(contacts.len());
        for (regions, centroids) in contacts {
            if centroids.len() != 2 {
                return Err("Each moisture boundary must have two incident triangles.".into());
            }
            let [i, j] = regions;
            let primal_midpoint = unit(add(surface.centers[i], surface.centers[j]));
            let toward_j =
                std::array::from_fn(|axis| surface.centers[j][axis] - surface.centers[i][axis]);
            let mut segments = Vec::with_capacity(2);
            for centroid in centroids {
                let segment_cross = cross(primal_midpoint, centroid);
                let angle = norm(segment_cross).atan2(dot(primal_midpoint, centroid));
                if !angle.is_finite() || angle <= 0. || angle >= PI {
                    return Err("Degenerate moisture-transport boundary.".into());
                }
                let mut outward_normal = unit(segment_cross);
                if dot(outward_normal, toward_j) < 0. {
                    outward_normal = outward_normal.map(|v| -v);
                }
                segments.push(BoundarySegment {
                    midpoint: unit(add(primal_midpoint, centroid)),
                    outward_normal,
                    arc_length_meters: radius_meters * angle,
                    quadrature_weight_meters: 2. * radius_meters * (angle / 2.).sin(),
                });
            }
            boundaries.push(SharedBoundary {
                regions,
                segments: segments.try_into().unwrap(),
            });
        }
        Ok(Self {
            areas_square_meters: surface.areas.clone(),
            boundaries,
        })
    }

    pub fn areas_square_meters(&self) -> &[f64] {
        &self.areas_square_meters
    }
    pub fn boundaries(&self) -> &[SharedBoundary] {
        &self.boundaries
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Settings {
    pub max_outgoing_fraction: f64,
    pub max_substeps: usize,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            max_outgoing_fraction: 0.8,
            max_substeps: 4096,
        }
    }
}

impl Settings {
    pub fn validate(self) -> Result<(), String> {
        if !self.max_outgoing_fraction.is_finite()
            || !(0. ..=0.9).contains(&self.max_outgoing_fraction)
            || self.max_outgoing_fraction == 0.
            || !(1..=1_000_000).contains(&self.max_substeps)
        {
            return Err("Invalid moisture-transport stability settings.".into());
        }
        Ok(())
    }
}

#[derive(Clone, Copy)]
struct Transfer {
    source: usize,
    recipient: usize,
    swept_area_square_meters_per_second: f64,
}

/// Frozen prescribed velocity for one caller-defined interval.
pub struct Flow {
    areas: Vec<f64>,
    transfers: Vec<Transfer>,
    max_outgoing_rate_per_second: f64,
}

impl Flow {
    /// Sample a continuous 3D tangent velocity in m/s at each boundary segment.
    /// Oppositely directed segment fluxes retain their separate upwind donors.
    pub fn sample(
        geometry: &Geometry,
        velocity: impl Fn([f64; 3]) -> [f64; 3],
    ) -> Result<Self, String> {
        let mut transfers = Vec::with_capacity(geometry.boundaries.len() * 2);
        let mut outgoing = vec![0.; geometry.areas_square_meters.len()];
        for boundary in &geometry.boundaries {
            for segment in boundary.segments {
                let wind = velocity(segment.midpoint);
                if wind.iter().any(|v| !v.is_finite())
                    || dot(wind, segment.midpoint).abs() > 1e-10 * norm(wind).max(1.)
                {
                    return Err("Moisture transport requires finite tangent velocities.".into());
                }
                let rate = dot(wind, segment.outward_normal) * segment.quadrature_weight_meters;
                if !rate.is_finite() {
                    return Err("Moisture edge flux overflow.".into());
                }
                if rate == 0. {
                    continue;
                }
                let [i, j] = boundary.regions;
                let (source, recipient) = if rate > 0. { (i, j) } else { (j, i) };
                outgoing[source] += rate.abs();
                transfers.push(Transfer {
                    source,
                    recipient,
                    swept_area_square_meters_per_second: rate.abs(),
                });
            }
        }
        let max_rate = outgoing
            .iter()
            .zip(&geometry.areas_square_meters)
            .map(|(rate, area)| rate / area)
            .fold(0., f64::max);
        if !max_rate.is_finite() {
            return Err("Moisture outgoing rate overflow.".into());
        }
        Ok(Self {
            areas: geometry.areas_square_meters.clone(),
            transfers,
            max_outgoing_rate_per_second: max_rate,
        })
    }

    /// Pure, atomic advance: input stocks are never modified, including on failure.
    /// Stocks are kg of column water, concentrations kg/m²; no atmospheric depth
    /// or air-density assumption is required by this passive transport equation.
    pub fn advance(
        &self,
        stock_kilograms: &[f64],
        seconds: f64,
        settings: Settings,
    ) -> Result<Step, String> {
        settings.validate()?;
        if stock_kilograms.len() != self.areas.len()
            || stock_kilograms.iter().any(|v| !v.is_finite() || *v < 0.)
            || !seconds.is_finite()
            || seconds <= 0.
        {
            return Err("Invalid moisture stock or interval.".into());
        }
        let needed = (seconds * self.max_outgoing_rate_per_second / settings.max_outgoing_fraction)
            .ceil()
            .max(1.);
        if !needed.is_finite() || needed > settings.max_substeps as f64 {
            return Err("Moisture interval exceeds the declared transport substep limit.".into());
        }
        let substeps = needed as usize;
        let dt = seconds / substeps as f64;
        let initial_mass = total_mass(stock_kilograms);
        if !initial_mass.is_finite() {
            return Err("Moisture total stock overflow.".into());
        }
        let mut stocks = stock_kilograms.to_vec();
        let mut outgoing = vec![0.; stocks.len()];
        let mut incoming = vec![0.; stocks.len()];
        let mut moved = 0.;
        for _ in 0..substeps {
            outgoing.fill(0.);
            incoming.fill(0.);
            for transfer in &self.transfers {
                let amount = stocks[transfer.source] / self.areas[transfer.source]
                    * transfer.swept_area_square_meters_per_second
                    * dt;
                outgoing[transfer.source] += amount;
                incoming[transfer.recipient] += amount;
            }
            for region in 0..stocks.len() {
                if !outgoing[region].is_finite() || outgoing[region] > stocks[region] {
                    return Err("Moisture outgoing flux exceeds available stock.".into());
                }
                stocks[region] = (stocks[region] - outgoing[region]) + incoming[region];
                if !stocks[region].is_finite() || stocks[region] < 0. {
                    return Err("Invalid moisture stock after transport.".into());
                }
            }
            moved += total_mass(&outgoing);
        }
        let final_mass = total_mass(&stocks);
        let residual = final_mass - initial_mass;
        if !moved.is_finite()
            || !final_mass.is_finite()
            || residual.abs() > 1e-12 * initial_mass.max(1.)
        {
            return Err("Moisture transport mass-budget tolerance exceeded.".into());
        }
        Ok(Step {
            stock_kilograms: stocks,
            budget: Budget {
                initial_kilograms: initial_mass,
                final_kilograms: final_mass,
                residual_kilograms: residual,
                transported_kilograms: moved,
                interval_seconds: seconds,
                substeps,
                max_outgoing_fraction: dt * self.max_outgoing_rate_per_second,
            },
        })
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Budget {
    pub initial_kilograms: f64,
    pub final_kilograms: f64,
    pub residual_kilograms: f64,
    /// Gross throughput; a parcel can cross multiple boundaries during an interval.
    pub transported_kilograms: f64,
    pub interval_seconds: f64,
    pub substeps: usize,
    pub max_outgoing_fraction: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Step {
    pub stock_kilograms: Vec<f64>,
    pub budget: Budget,
}

/// Compensated summation for stock and reference-area budgets.
pub fn total_mass(values: &[f64]) -> f64 {
    let mut sum = 0.;
    let mut correction = 0.;
    for &value in values {
        let adjusted = value - correction;
        let next = sum + adjusted;
        correction = (next - sum) - adjusted;
        sum = next;
    }
    sum
}
