//! Regional finite-volume surface flow, not an instantaneous basin frontier.
//! Manning-inspired face mobility is capped explicitly; no momentum solver.
use super::{Checkpoint as SeasonalCheckpoint, leaf_spill::Components, reference_pool};
use crate::{Surface, World, moisture_transport, surface_water::CompensatedStock};
use serde::{Deserialize, Serialize};

pub const MODEL_VERSION: &str = "regional-surface-flow-1";
const RHO: f64 = 1000.;
pub(super) const COURANT: f64 = 0.45;
const MAX_SUBSTEPS: usize = 16_384;

#[derive(Clone, Copy, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Settings {
    /// Manning n, seconds per metre^(1/3); an uncalibrated uniform roughness.
    pub roughness: f64,
    /// Explicit mobility regularization, not a claim of resolved hydraulics.
    pub maximum_diffusivity_square_meters_per_second: f64,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            roughness: 0.04,
            maximum_diffusivity_square_meters_per_second: 1e6,
        }
    }
}
impl Settings {
    pub fn validate(self) -> Result<(), String> {
        if !self.roughness.is_finite()
            || !(0.005..=1.).contains(&self.roughness)
            || !self
                .maximum_diffusivity_square_meters_per_second
                .is_finite()
            || !(0. ..=1e8).contains(&self.maximum_diffusivity_square_meters_per_second)
        {
            return Err("Invalid regional surface-flow roughness or diffusivity.".into());
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Checkpoint {
    pub model_version: String,
    /// Two directed contacts per sorted physical face. Gross flows, not stocks.
    pub directed_transfers: Components,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Observation {
    pub observation_version: String,
    pub simulation_model_version: String,
    pub elapsed_seconds: u64,
    /// These IDs belong to this resolved recipe, never renderer geometry.
    pub recipe: crate::Recipe,
    pub regional_liquid_high_kilograms: Vec<f64>,
    pub regional_liquid_low_kilograms: Vec<f64>,
    pub depth_meters: Vec<f64>,
    pub levels_meters: Vec<Option<f64>>,
    pub explicit_stability_bound_seconds: f64,
}

/// Rounded leading cumulative graph fields; these are flows, never stocks.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TransferObservation {
    pub observation_version: String,
    pub cumulative_incoming_kilograms: Vec<f64>,
    pub cumulative_outgoing_kilograms: Vec<f64>,
    pub cumulative_transferred_kilograms: f64,
}

#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FlowBudget {
    pub substeps: usize,
    pub transferred_kilograms: f64,
    /// Unapplied requests; water stays at the donor, never counted as transfer.
    /// Repeated requests can refer to the same water; this is not an inventory.
    pub deferred_request_kilograms: f64,
    pub deferred_requests: usize,
    pub maximum_deferred_request_kilograms: f64,
    pub deferred_evaporation_requests: usize,
    pub deferred_evaporation_request_kilograms: f64,
    pub maximum_deferred_evaporation_kilograms: f64,
}
impl FlowBudget {
    pub(super) fn accumulate(&mut self, other: Self) {
        self.substeps += other.substeps;
        self.transferred_kilograms += other.transferred_kilograms;
        self.deferred_request_kilograms += other.deferred_request_kilograms;
        self.deferred_requests += other.deferred_requests;
        self.maximum_deferred_request_kilograms = self
            .maximum_deferred_request_kilograms
            .max(other.maximum_deferred_request_kilograms);
        self.deferred_evaporation_requests += other.deferred_evaporation_requests;
        self.deferred_evaporation_request_kilograms += other.deferred_evaporation_request_kilograms;
        self.maximum_deferred_evaporation_kilograms = self
            .maximum_deferred_evaporation_kilograms
            .max(other.maximum_deferred_evaporation_kilograms);
    }
}
#[derive(Clone, Debug)]
pub(super) struct Face {
    pub regions: [usize; 2],
    pub width_over_distance: f64,
    pub distance_meters: f64,
}
#[derive(Clone)]
pub(super) struct Layout {
    pub areas: Vec<f64>,
    pub beds: Vec<f64>,
    pub land: Vec<bool>,
    pub reference_level: f64,
    pub faces: Vec<Face>,
    pub settings: Settings,
    pub stable_seconds: f64,
}
impl Layout {
    pub fn observe_transfers(
        &self,
        cp: &SeasonalCheckpoint,
    ) -> Result<TransferObservation, String> {
        let incoming = self.incoming(cp)?;
        let history = &cp
            .regional_surface_flow
            .as_ref()
            .unwrap()
            .directed_transfers;
        let mut outgoing = vec![CompensatedStock::new(0., 0., f64::MAX)?; self.areas.len()];
        for (f, face) in self.faces.iter().enumerate() {
            for direction in 0..2 {
                let flow = history.stock(2 * f + direction)?;
                let donor = &mut outgoing[face.regions[direction]];
                donor.credit(flow.high)?;
                donor.credit(flow.low)?;
            }
        }
        Ok(TransferObservation {
            observation_version: "regional-surface-transfers-1".into(),
            cumulative_incoming_kilograms: incoming.into_iter().map(|v| v.high).collect(),
            cumulative_outgoing_kilograms: outgoing.into_iter().map(|v| v.high).collect(),
            cumulative_transferred_kilograms: history.total(),
        })
    }
    pub fn observe(&self, cp: &SeasonalCheckpoint) -> Result<Observation, String> {
        let depths: Vec<_> = cp
            .terminal_water_kilograms
            .iter()
            .zip(&self.areas)
            .map(|(&m, &a)| m / (RHO * a))
            .collect();
        let levels = depths
            .iter()
            .zip(&self.beds)
            .map(|(&d, &b)| (d > 0.).then_some(b + d))
            .collect();
        Ok(Observation {
            observation_version: "regional-surface-observation-1".into(),
            simulation_model_version: cp.model_version.clone(),
            elapsed_seconds: cp.elapsed_seconds,
            recipe: cp.recipe.clone(),
            regional_liquid_high_kilograms: cp.terminal_water_kilograms.clone(),
            regional_liquid_low_kilograms: cp.terminal_low_kilograms.as_ref().unwrap().clone(),
            depth_meters: depths,
            levels_meters: levels,
            explicit_stability_bound_seconds: self.stable_seconds,
        })
    }
    pub fn from_world(world: &World, settings: Settings) -> Result<Self, String> {
        Self::from_fields(
            &world.surface,
            world.recipe.radius_meters,
            world.terrain.elevation.clone(),
            world.water.body_ids.iter().map(|&id| id == 0).collect(),
            world.water.level_meters,
            settings,
        )
    }
    // Also used by isolated operator verification; never a generated-world recipe.
    pub fn from_fields(
        surface: &Surface,
        radius_meters: f64,
        beds: Vec<f64>,
        land: Vec<bool>,
        reference_level: f64,
        settings: Settings,
    ) -> Result<Self, String> {
        settings.validate()?;
        if beds.len() != surface.areas.len()
            || land.len() != beds.len()
            || beds.iter().any(|v| !v.is_finite())
            || !reference_level.is_finite()
        {
            return Err("Invalid surface-flow physical fields.".into());
        }
        let geometry = moisture_transport::Geometry::from_surface(surface, radius_meters)?;
        let mut sums = vec![0.; surface.areas.len()];
        let mut faces = Vec::new();
        for boundary in geometry.boundaries() {
            let [a, b] = boundary.regions;
            let slot = (surface.offsets[a] as usize..surface.offsets[a + 1] as usize)
                .find(|&j| surface.neighbors[j] as usize == b)
                .ok_or("Surface face lacks neighbor distance.")?;
            let distance = surface.distances[slot];
            let width = boundary
                .segments
                .iter()
                .map(|s| s.arc_length_meters)
                .sum::<f64>();
            let ratio = width / distance;
            if !ratio.is_finite() || ratio <= 0. {
                return Err("Invalid surface-flow face metric.".into());
            }
            sums[a] += ratio;
            sums[b] += ratio;
            faces.push(Face {
                regions: [a, b],
                width_over_distance: ratio,
                distance_meters: distance,
            });
        }
        let diffusivity = settings.maximum_diffusivity_square_meters_per_second;
        let stable_seconds = if diffusivity == 0. {
            f64::MAX
        } else {
            surface
                .areas
                .iter()
                .zip(sums)
                .map(|(&a, s)| COURANT * a / (diffusivity * s))
                .fold(f64::MAX, f64::min)
        };
        if !stable_seconds.is_finite() || stable_seconds <= 0. {
            return Err("Unrepresentable surface-flow stability bound.".into());
        }
        Ok(Self {
            areas: surface.areas.clone(),
            beds,
            land,
            reference_level,
            faces,
            settings,
            stable_seconds,
        })
    }
    pub fn initial_checkpoint(&self) -> Checkpoint {
        Checkpoint {
            model_version: MODEL_VERSION.into(),
            directed_transfers: Components::zero(2 * self.faces.len()),
        }
    }
    pub fn wet(&self, cp: &SeasonalCheckpoint) -> Vec<bool> {
        cp.terminal_water_kilograms
            .iter()
            .zip(&self.land)
            .map(|(&m, &land)| land && m > 0.)
            .collect()
    }
    pub fn incoming(&self, cp: &SeasonalCheckpoint) -> Result<Vec<CompensatedStock>, String> {
        let state = cp
            .regional_surface_flow
            .as_ref()
            .ok_or("Missing regional surface-flow history.")?;
        if state.model_version != MODEL_VERSION
            || state.directed_transfers.high_kilograms.len() != 2 * self.faces.len()
            || state.directed_transfers.low_kilograms.len() != 2 * self.faces.len()
        {
            return Err("Invalid surface-flow history pin or shape.".into());
        }
        let mut incoming = vec![CompensatedStock::new(0., 0., f64::MAX)?; self.areas.len()];
        for (f, face) in self.faces.iter().enumerate() {
            for direction in 0..2 {
                let source = face.regions[direction];
                let target = face.regions[1 - direction];
                let stock = state.directed_transfers.stock(2 * f + direction)?;
                if stock.high > 0.
                    && (!self.land[source]
                        || cp.elapsed_seconds == 0
                        || self.settings.maximum_diffusivity_square_meters_per_second == 0.)
                {
                    return Err("Surface-flow history has no enabled regional donor.".into());
                }
                incoming[target].credit(stock.high)?;
                incoming[target].credit(stock.low)?;
            }
        }
        Ok(incoming)
    }
    pub fn validate(&self, cp: &SeasonalCheckpoint) -> Result<(f64, usize), String> {
        let incoming = self.incoming(cp)?;
        let state = cp.regional_surface_flow.as_ref().unwrap();
        let mut outgoing = vec![CompensatedStock::new(0., 0., f64::MAX)?; self.areas.len()];
        for (f, face) in self.faces.iter().enumerate() {
            for direction in 0..2 {
                let flow = state.directed_transfers.stock(2 * f + direction)?;
                outgoing[face.regions[direction]].credit(flow.high)?;
                outgoing[face.regions[direction]].credit(flow.low)?;
            }
        }
        let h = cp
            .cumulative_lake_capture_kilograms
            .as_ref()
            .ok_or("Missing regional capture history.")?;
        let l = cp
            .cumulative_lake_capture_low_kilograms
            .as_ref()
            .ok_or("Missing regional capture tails.")?;
        if h.len() != self.areas.len() || l.len() != h.len() {
            return Err("Invalid regional capture shape.".into());
        }
        let mut maximum = (0., 0);
        for r in 0..self.areas.len() {
            let capture = CompensatedStock::new(h[r], l[r], f64::MAX)?;
            if !self.land[r] || cp.elapsed_seconds == 0 {
                if capture.high != 0. || cp.terminal_water_kilograms[r] != 0. {
                    return Err("Regional water/capture has a duplicate or initial owner.".into());
                }
                continue;
            }
            let route = cp.cumulative_runoff_transfers[r];
            let terms = [
                cp.terminal_water_kilograms[r],
                cp.terminal_low_kilograms.as_ref().unwrap()[r],
                -capture.high,
                -capture.low,
                -route.terminal_delivery,
                route.terminal_evaporation,
                -incoming[r].high,
                -incoming[r].low,
                outgoing[r].high,
                outgoing[r].low,
            ];
            let residual = moisture_transport::total_mass(&terms);
            let scale = terms.iter().map(|v| v.abs()).fold(1., f64::max);
            let relative = residual.abs() / scale;
            if !relative.is_finite() {
                return Err("Nonfinite regional surface-water identity.".into());
            }
            if relative > maximum.0 {
                maximum = (relative, r);
            }
        }
        Ok(maximum)
    }
    /// Provisional only. All rates use the old substep snapshot, never earlier arrivals.
    pub fn advance(
        &self,
        cp: &mut SeasonalCheckpoint,
        pool: &reference_pool::Layout,
        seconds: f64,
    ) -> Result<FlowBudget, String> {
        if !seconds.is_finite() || seconds <= 0. {
            return Err("Invalid surface-flow interval.".into());
        }
        let count = (seconds / self.stable_seconds).ceil().max(1.) as usize;
        if count > MAX_SUBSTEPS {
            return Err("Surface-flow interval exceeds the explicit work bound.".into());
        }
        let dt = seconds / count as f64;
        let mut budget = FlowBudget {
            substeps: count,
            ..Default::default()
        };
        let mut depth = vec![0.; self.areas.len()];
        for _ in 0..count {
            for (r, d) in depth.iter_mut().enumerate() {
                *d = if self.land[r] {
                    cp.terminal_water_kilograms[r] / (RHO * self.areas[r])
                } else {
                    (self.reference_level - self.beds[r]).max(0.)
                };
            }
            for (f, face) in self.faces.iter().enumerate() {
                let [a, b] = face.regions;
                // Relative differences preserve shallow water under a large datum shift.
                let difference = (self.beds[a] - self.beds[b]) + depth[a] - depth[b];
                let direction = usize::from(difference < 0.);
                let source = face.regions[direction];
                let target = face.regions[1 - direction];
                if !self.land[source] || difference == 0. {
                    continue;
                }
                let over_crest = depth[source] - (self.beds[target] - self.beds[source]).max(0.);
                let drive = difference.abs().min(over_crest);
                if drive <= 0. {
                    continue;
                }
                let beta = (over_crest.powf(5. / 3.)
                    / (self.settings.roughness * (drive / face.distance_meters).sqrt()))
                .min(self.settings.maximum_diffusivity_square_meters_per_second);
                let request = RHO * dt * beta * face.width_over_distance * drive;
                if !request.is_finite() || request < 0. {
                    return Err("Nonfinite surface-flow request.".into());
                }
                if request == 0. {
                    continue;
                }
                let mut donor = CompensatedStock::new(
                    cp.terminal_water_kilograms[source],
                    cp.terminal_low_kilograms.as_ref().unwrap()[source],
                    f64::MAX,
                )?;
                if request > donor.available() {
                    return Err("Surface-flow request violates positivity bound.".into());
                }
                let before = donor;
                if donor.withdraw(request) != request
                    || (donor.high, donor.low) == (before.high, before.low)
                {
                    return Err("Surface-flow debit is below donor resolution.".into());
                }
                let mut receiver = if let Some(body) = pool.by_region[target] {
                    CompensatedStock::new(
                        cp.reference_body_high_kilograms.as_ref().unwrap()[body],
                        cp.reference_body_low_kilograms.as_ref().unwrap()[body],
                        f64::MAX,
                    )?
                } else {
                    CompensatedStock::new(
                        cp.terminal_water_kilograms[target],
                        cp.terminal_low_kilograms.as_ref().unwrap()[target],
                        f64::MAX,
                    )?
                };
                let old = receiver;
                receiver.credit(request)?;
                let residual = moisture_transport::total_mass(&[
                    donor.high - before.high,
                    donor.low - before.low,
                    receiver.high - old.high,
                    receiver.low - old.low,
                ]);
                if !residual.is_finite() {
                    return Err("Nonfinite surface-flow pair residual.".into());
                }
                if (receiver.high, receiver.low) == (old.high, old.low)
                    || residual.abs() > 32. * f64::EPSILON * request.max(f64::MIN_POSITIVE)
                {
                    // Defined arithmetic-resolution limiter, not error swallowing.
                    // Neither provisional pair is published: all requested water
                    // remains at the regional donor until a representable flux.
                    budget.deferred_requests += 1;
                    budget.deferred_request_kilograms += request;
                    budget.maximum_deferred_request_kilograms =
                        budget.maximum_deferred_request_kilograms.max(request);
                    continue;
                }
                cp.terminal_water_kilograms[source] = donor.high;
                cp.terminal_low_kilograms.as_mut().unwrap()[source] = donor.low;
                if let Some(body) = pool.by_region[target] {
                    cp.reference_body_high_kilograms.as_mut().unwrap()[body] = receiver.high;
                    cp.reference_body_low_kilograms.as_mut().unwrap()[body] = receiver.low;
                } else {
                    cp.terminal_water_kilograms[target] = receiver.high;
                    cp.terminal_low_kilograms.as_mut().unwrap()[target] = receiver.low;
                }
                cp.regional_surface_flow
                    .as_mut()
                    .unwrap()
                    .directed_transfers
                    .credit(2 * f + direction, request)?;
                budget.transferred_kilograms += request;
            }
        }
        Ok(budget)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::seasonal_moisture::{
        ClosedLakeExchange, Model, ReferenceWaterPool, SoilNumerics, SurfaceNumerics,
        TerminalNumerics,
    };

    // Deliberately small operator geometry, not a generated-world restore fixture.
    fn fixture(
        beds: &[f64],
        edges: &[[usize; 2]],
        water: &[f64],
    ) -> (Layout, SeasonalCheckpoint, reference_pool::Layout) {
        let mut recipe: crate::Recipe = serde_json::from_str(include_str!(
            "../../../docs/scenarios/spill-connections.json"
        ))
        .unwrap();
        recipe.subdivision = 0;
        let world = World::generate(recipe).unwrap();
        let model = Model::from_world(
            &world,
            super::super::Settings {
                orography: Some(Default::default()),
                soil_numerics: Some(SoilNumerics::Compensated),
                surface_numerics: Some(SurfaceNumerics::Compensated),
                terminal_numerics: Some(TerminalNumerics::Compensated),
                reference_water_pool: Some(ReferenceWaterPool::FastConnectedBody),
                closed_lake_exchange: Some(ClosedLakeExchange::FrozenRegionalSurfaceFlow(
                    Settings {
                        roughness: 0.04,
                        maximum_diffusivity_square_meters_per_second: 1.,
                    },
                )),
                ..Default::default()
            },
            Default::default(),
            Default::default(),
        )
        .unwrap();
        let mut sums = vec![0.; beds.len()];
        for &[a, b] in edges {
            sums[a] += 1.;
            sums[b] += 1.;
        }
        let layout = Layout {
            areas: vec![1.; beds.len()],
            beds: beds.to_vec(),
            land: vec![true; beds.len()],
            reference_level: -10.,
            faces: edges
                .iter()
                .map(|&regions| Face {
                    regions,
                    width_over_distance: 1.,
                    distance_meters: 1.,
                })
                .collect(),
            settings: Settings {
                roughness: 0.04,
                maximum_diffusivity_square_meters_per_second: 1.,
            },
            stable_seconds: sums
                .into_iter()
                .map(|s| if s > 0. { COURANT / s } else { f64::MAX })
                .fold(f64::MAX, f64::min),
        };
        let mut cp = model.initial_state().checkpoint();
        cp.elapsed_seconds = 1;
        cp.terminal_water_kilograms = water.iter().map(|v| v * RHO).collect();
        cp.terminal_low_kilograms = Some(vec![0.; beds.len()]);
        cp.cumulative_lake_capture_kilograms = Some(cp.terminal_water_kilograms.clone());
        cp.cumulative_lake_capture_low_kilograms = Some(vec![0.; beds.len()]);
        cp.cumulative_runoff_transfers = vec![Default::default(); beds.len()];
        cp.regional_surface_flow = Some(layout.initial_checkpoint());
        (
            layout,
            cp,
            reference_pool::Layout::new(&vec![0; beds.len()]),
        )
    }
    fn mass(cp: &SeasonalCheckpoint) -> f64 {
        moisture_transport::total_mass(
            &cp.terminal_water_kilograms
                .iter()
                .chain(cp.terminal_low_kilograms.as_ref().unwrap())
                .copied()
                .collect::<Vec<_>>(),
        )
    }
    fn exact(values: impl Iterator<Item = f64>) -> i128 {
        values
            .map(|v| {
                let scaled = v * 2_f64.powi(50);
                let integer = scaled as i128;
                assert_eq!(scaled, integer as f64);
                integer
            })
            .sum()
    }
    #[test]
    fn flat_equilibrium_and_below_crest_do_not_leak_or_invent_water() {
        for (beds, water) in [
            (vec![0., 0.], vec![2., 2.]),
            (vec![0., 3.], vec![2., 0.]),
            (vec![0., 3.], vec![3., 0.]),
        ] {
            let (l, mut cp, p) = fixture(&beds, &[[0, 1]], &water);
            let before = cp.clone();
            l.advance(&mut cp, &p, 5.).unwrap();
            assert_eq!(cp, before);
        }
    }
    #[test]
    fn two_columns_match_independent_dyadic_update_and_converge_to_analytic_relaxation() {
        let (mut l, mut cp, p) = fixture(&[0., 0.], &[[0, 1]], &[4., 2.]);
        l.settings.maximum_diffusivity_square_meters_per_second = 0.5;
        l.stable_seconds = 0.9;
        l.advance(&mut cp, &p, 0.5).unwrap();
        assert_eq!(cp.terminal_water_kilograms, [3500., 2500.]);
        assert_eq!(
            exact(
                cp.terminal_water_kilograms
                    .iter()
                    .chain(cp.terminal_low_kilograms.as_ref().unwrap())
                    .copied()
            ),
            6000_i128 << 50
        );
        assert!(l.validate(&cp).unwrap().0 < 1e-12);
        let mut errors = Vec::new();
        for steps in [10, 20, 40] {
            let (_, mut cp, p) = fixture(&[0., 0.], &[[0, 1]], &[4., 2.]);
            for _ in 0..steps {
                l.advance(&mut cp, &p, 1. / steps as f64).unwrap();
            }
            let difference =
                (cp.terminal_water_kilograms[0] - cp.terminal_water_kilograms[1]) / RHO;
            errors.push((difference - 2. * (-1_f64).exp()).abs());
            assert!((mass(&cp) - 6000.).abs() < 1e-10);
        }
        assert!(errors[1] < errors[0] && errors[2] < errors[1]);
    }
    #[test]
    fn simultaneous_fork_is_symmetric_and_does_not_reuse_received_water() {
        let (l, mut cp, p) = fixture(&[0.; 4], &[[0, 1], [0, 2], [0, 3]], &[3., 0., 0., 0.]);
        l.advance(&mut cp, &p, 0.125).unwrap();
        assert_eq!(cp.terminal_water_kilograms, [1875., 375., 375., 375.]);
        assert_eq!(mass(&cp), 3000.);
        let (l, mut cp, p) = fixture(&[0.; 3], &[[0, 1], [1, 2]], &[3., 0., 0.]);
        l.advance(&mut cp, &p, 0.125).unwrap();
        assert_eq!(cp.terminal_water_kilograms, [2625., 375., 0.]);
        assert!(l.validate(&cp).unwrap().0 == 0.);
    }
    #[test]
    fn nested_bowls_connect_through_both_real_sills_with_no_parent_special_cases() {
        let (l, mut cp, p) = fixture(
            &[0., 2., 1., 5., -1.],
            &[[0, 1], [1, 2], [2, 3], [3, 4]],
            &[0., 0., 0., 0., 20.],
        );
        l.advance(&mut cp, &p, 100.).unwrap();
        assert!((mass(&cp) - 20000.).abs() < 1e-9);
        for r in 0..5 {
            assert!((l.beds[r] + cp.terminal_water_kilograms[r] / RHO - 5.4).abs() < 1e-6);
        }
        assert!(l.validate(&cp).unwrap().0 < 1e-12);
    }
    #[test]
    fn finite_reference_contact_receives_actual_mass_and_signed_tails_stay_owned() {
        let (mut l, mut cp, _) = fixture(&[0., -2.], &[[0, 1]], &[2., 0.]);
        l.land[1] = false;
        l.reference_level = 0.;
        cp.reference_body_high_kilograms = Some(vec![1e15]);
        cp.reference_body_low_kilograms = Some(vec![2_f64.powi(-50)]);
        let p = reference_pool::Layout::new(&[0, 7]);
        let before = exact(
            cp.terminal_water_kilograms
                .iter()
                .chain(cp.terminal_low_kilograms.as_ref().unwrap())
                .chain(cp.reference_body_high_kilograms.as_ref().unwrap())
                .chain(cp.reference_body_low_kilograms.as_ref().unwrap())
                .copied(),
        );
        l.advance(&mut cp, &p, 0.125).unwrap();
        assert_eq!(cp.terminal_water_kilograms[0], 1750.);
        assert_eq!(
            cp.reference_body_high_kilograms.as_ref().unwrap()[0],
            1e15 + 250.
        );
        assert_eq!(
            cp.reference_body_low_kilograms.as_ref().unwrap()[0],
            2_f64.powi(-50)
        );
        let after = exact(
            cp.terminal_water_kilograms
                .iter()
                .chain(cp.terminal_low_kilograms.as_ref().unwrap())
                .chain(cp.reference_body_high_kilograms.as_ref().unwrap())
                .chain(cp.reference_body_low_kilograms.as_ref().unwrap())
                .copied(),
        );
        assert_eq!(before, after);
    }
    #[test]
    fn datum_and_face_order_do_not_change_the_dyadic_parallel_update() {
        let (l, mut a, p) = fixture(&[0., 0., 0.], &[[0, 1], [1, 2]], &[4., 2., 0.]);
        let (mut reversed, mut b, pb) = fixture(&[0., 0., 0.], &[[1, 2], [0, 1]], &[4., 2., 0.]);
        reversed.beds.fill(1e9);
        l.advance(&mut a, &p, 0.125).unwrap();
        reversed.advance(&mut b, &pb, 0.125).unwrap();
        assert_eq!(a.terminal_water_kilograms, b.terminal_water_kilograms);
        assert_eq!(a.terminal_low_kilograms, b.terminal_low_kilograms);
    }
    #[test]
    fn unresolved_face_credit_keeps_both_owners_and_transfer_history_unchanged() {
        let (mut l, mut cp, _) = fixture(&[0., -2.], &[[0, 1]], &[2_f64.powi(-40), 0.]);
        l.land[1] = false;
        l.reference_level = 0.;
        cp.reference_body_high_kilograms = Some(vec![1e15]);
        cp.reference_body_low_kilograms = Some(vec![0.01]);
        let p = reference_pool::Layout::new(&[0, 7]);
        let before = cp.clone();
        let budget = l.advance(&mut cp, &p, 0.125).unwrap();
        assert_eq!(cp, before);
        assert_eq!(budget.transferred_kilograms, 0.);
        assert_eq!(budget.deferred_requests, 1);
        assert!(budget.deferred_request_kilograms > 0.);
        assert_eq!(
            budget.deferred_request_kilograms,
            budget.maximum_deferred_request_kilograms
        );
    }
    #[test]
    fn relabeling_and_geometric_area_scaling_preserve_capped_parallel_depths() {
        let (l, mut a, p) = fixture(&[0.; 3], &[[0, 1], [1, 2]], &[4., 2., 0.]);
        let (renamed, mut b, pb) = fixture(&[0.; 3], &[[2, 0], [0, 1]], &[2., 0., 4.]);
        l.advance(&mut a, &p, 0.125).unwrap();
        renamed.advance(&mut b, &pb, 0.125).unwrap();
        for (old, new) in [(0, 2), (1, 0), (2, 1)] {
            assert_eq!(
                a.terminal_water_kilograms[old],
                b.terminal_water_kilograms[new]
            );
            assert_eq!(
                a.terminal_low_kilograms.as_ref().unwrap()[old],
                b.terminal_low_kilograms.as_ref().unwrap()[new]
            );
        }
        let (mut scaled, mut c, pc) = fixture(&[0.; 3], &[[0, 1], [1, 2]], &[16., 8., 0.]);
        scaled.areas.fill(4.);
        for face in &mut scaled.faces {
            face.distance_meters *= 2.;
        }
        scaled.stable_seconds *= 4.;
        scaled.advance(&mut c, &pc, 0.5).unwrap();
        for r in 0..3 {
            assert_eq!(
                c.terminal_water_kilograms[r] / 4.,
                a.terminal_water_kilograms[r]
            );
        }
        assert_eq!(scaled.validate(&c).unwrap().0, 0.);
    }
    #[test]
    fn unresolved_scalar_vapor_credit_retains_liquid_but_resolvable_depletion_is_complete() {
        let mut recipe: crate::Recipe = serde_json::from_str(include_str!(
            "../../../docs/scenarios/spill-connections.json"
        ))
        .unwrap();
        recipe.subdivision = 0;
        let world = World::generate(recipe).unwrap();
        let mode = ClosedLakeExchange::FrozenRegionalSurfaceFlow(Default::default());
        let layout = super::super::lake_exchange::Layout::from_world(&world, mode).unwrap();
        let (_, mut cp, _) = fixture(&[0.; 12], &[], &[0.; 12]);
        let r = world.water.body_ids.iter().position(|&id| id == 0).unwrap();
        cp.terminal_water_kilograms[r] = 1e-10;
        cp.vapor_kilograms[r] = 1e15;
        let mut demand = vec![0.; 12];
        demand[r] = 1.;
        let before = cp.clone();
        let mut budget = FlowBudget::default();
        let (packets, residual) = layout
            .evaporate(&mut cp, &demand, Some(&mut budget))
            .unwrap();
        assert!(packets.is_empty());
        assert_eq!(residual, 0.);
        assert_eq!(cp, before);
        assert_eq!(budget.deferred_evaporation_requests, 1);
        assert_eq!(budget.deferred_evaporation_request_kilograms, 1e-10);
        cp.vapor_kilograms[r] = 0.;
        let (packets, residual) = layout
            .evaporate(&mut cp, &demand, Some(&mut budget))
            .unwrap();
        assert_eq!(packets, [(r, 1e-10)]);
        assert_eq!(residual, 0.);
        assert_eq!(cp.terminal_water_kilograms[r], 0.);
        assert_eq!(cp.terminal_low_kilograms.as_ref().unwrap()[r], 0.);
        assert_eq!(budget.deferred_evaporation_requests, 1);
    }
}
