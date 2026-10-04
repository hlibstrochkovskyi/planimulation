//! Coupled exclusive leaf owners with exposure frozen for a half-local stage.
//! Reaching a first connection refuses; no implicit spill collector or merge.
use super::{Checkpoint, closed_lake, reference_pool};
use crate::{World, moisture_transport::total_mass, surface_water::CompensatedStock};

pub(super) const MODEL_VERSION: &str = "closed-leaf-exchange-1";

pub(super) struct Layout {
    pub geometry: closed_lake::Layout,
    pub by_region: Vec<Option<usize>>,
}

impl Layout {
    pub fn from_world(world: &World) -> Result<Self, String> {
        let geometry = closed_lake::Layout::from_world(world)?;
        if crate::basins::Basins::build(&world.surface, &world.terrain.elevation)? != world.basins {
            return Err("Coupled lake geometry has a stale basin hierarchy.".into());
        }
        if world
            .water
            .body_ids
            .iter()
            .any(|&b| b as usize > world.surface.areas.len())
        {
            return Err("Coupled lake reference-body ID is outside generated bounds.".into());
        }
        if crate::drainage::Drainage::build(
            &world.surface,
            &world.terrain.elevation,
            &world.water.body_ids,
        ) != world.drainage
        {
            return Err("Coupled lake geometry has stale drainage.".into());
        }
        let mut by_region = vec![None; world.surface.areas.len()];
        for (b, lake) in geometry.lakes().iter().enumerate() {
            for &r in lake.regions() {
                if by_region[r].replace(b).is_some() {
                    return Err("Overlapping coupled lake owners.".into());
                }
            }
        }
        Ok(Self {
            geometry,
            by_region,
        })
    }

    fn liquid(cp: &Checkpoint, r: usize) -> closed_lake::Liquid {
        closed_lake::Liquid {
            high_kilograms: cp.terminal_water_kilograms[r],
            low_kilograms: cp.terminal_low_kilograms.as_ref().unwrap()[r],
        }
    }

    pub fn exposure(&self, cp: &Checkpoint) -> Result<Vec<bool>, String> {
        let mut wet = vec![false; self.by_region.len()];
        for lake in self.geometry.lakes() {
            let r = lake.terminal_region();
            let surface = lake
                .surface(Self::liquid(cp, r))
                .map_err(|e| format!("Coupled lake at terminal {r}: {e}"))?;
            if surface.at_spill_threshold {
                return Err(format!(
                    "Coupled lake at terminal {r} reached its unsupported first connection."
                ));
            }
            for i in surface.exposed_regions {
                wet[i] = true;
            }
        }
        Ok(wet)
    }

    /// Capture a complete local liquid pair once; the caller clears its owner.
    pub fn capture(
        &self,
        cp: &mut Checkpoint,
        region: usize,
        high: f64,
        low: f64,
    ) -> Result<(), String> {
        CompensatedStock::new(high, low, f64::MAX)?;
        let b = self.by_region[region].ok_or("Capture outside a lake leaf.")?;
        let r = self.geometry.lakes()[b].terminal_region();
        let old = Self::liquid(cp, r);
        let mut donor = CompensatedStock::new(old.high_kilograms, old.low_kilograms, f64::MAX)?;
        donor.credit(high)?;
        donor.credit(low)?;
        let residual = total_mass(&[
            donor.high - old.high_kilograms,
            donor.low - old.low_kilograms,
            -high,
            -low,
        ]);
        if !residual.is_finite()
            || residual.abs() > 32. * f64::EPSILON * old.high_kilograms.max(high).max(1.)
        {
            return Err("Lake capture exceeds its local arithmetic tolerance.".into());
        }
        let high_totals = cp.cumulative_lake_capture_kilograms.as_mut().unwrap();
        let low_totals = cp.cumulative_lake_capture_low_kilograms.as_mut().unwrap();
        let mut total = CompensatedStock::new(high_totals[region], low_totals[region], f64::MAX)?;
        total.credit(high)?;
        total.credit(low)?;
        cp.terminal_water_kilograms[r] = donor.high;
        cp.terminal_low_kilograms.as_mut().unwrap()[r] = donor.low;
        high_totals[region] = total.high;
        low_totals[region] = total.low;
        Ok(())
    }

    /// Newly captured rain/melt can fund grants on the pre-stage footprint.
    /// Growth/contraction changes the next stage, not this stage's requests.
    pub fn evaporate(
        &self,
        cp: &mut Checkpoint,
        demand: &[f64],
    ) -> Result<(Vec<f64>, f64), String> {
        // Do not hide an unsupported crossing by evaporating back below it.
        self.exposure(cp)?;
        let mut regional = vec![0.; self.by_region.len()];
        let mut maximum: f64 = 0.;
        for lake in self.geometry.lakes() {
            let r = lake.terminal_region();
            let before = Self::liquid(cp, r);
            let donor =
                CompensatedStock::new(before.high_kilograms, before.low_kilograms, f64::MAX)?;
            let requests: Vec<_> = lake.regions().iter().map(|&i| demand[i]).collect();
            let (after, grants, residual) = reference_pool::allocate(donor, &requests)?;
            cp.terminal_water_kilograms[r] = after.high;
            cp.terminal_low_kilograms.as_mut().unwrap()[r] = after.low;
            maximum = maximum.max(residual.abs());
            for (&i, grant) in lake.regions().iter().zip(grants) {
                regional[i] = grant;
            }
        }
        self.exposure(cp)?;
        Ok((regional, maximum))
    }

    /// One lake identity replaces endpoint-only identities in this version.
    pub fn validate(&self, cp: &Checkpoint) -> Result<(f64, usize), String> {
        let high = cp
            .cumulative_lake_capture_kilograms
            .as_ref()
            .ok_or("Missing lake capture ledger.")?;
        let low = cp
            .cumulative_lake_capture_low_kilograms
            .as_ref()
            .ok_or("Missing lake capture low ledger.")?;
        if high.len() != self.by_region.len() || low.len() != high.len() {
            return Err("Invalid lake capture ledger shape.".into());
        }
        for (i, (&h, &l)) in high.iter().zip(low).enumerate() {
            CompensatedStock::new(h, l, f64::MAX)?;
            if (self.by_region[i].is_none() || cp.elapsed_seconds == 0) && (h != 0. || l != 0.) {
                return Err("Lake capture ledger has the wrong owner or initial state.".into());
            }
        }
        self.exposure(cp)?;
        let mut maximum = (0., 0);
        for lake in self.geometry.lakes() {
            let r = lake.terminal_region();
            let liquid = Self::liquid(cp, r);
            let delivered = cp.cumulative_runoff_transfers[r].terminal_delivery;
            let mut terms = vec![liquid.high_kilograms, liquid.low_kilograms, -delivered];
            let mut flows = vec![delivered];
            for &i in lake.regions() {
                let e = cp.cumulative_runoff_transfers[i].terminal_evaporation;
                terms.extend([-high[i], -low[i], e]);
                flows.extend([high[i], low[i], e]);
            }
            let residual = total_mass(&terms);
            let scale = liquid.high_kilograms.max(total_mass(&flows)).max(1.);
            if !residual.is_finite() || !scale.is_finite() {
                return Err("Nonfinite coupled lake identity.".into());
            }
            if residual.abs() / scale > maximum.0 {
                maximum = (residual.abs() / scale, r);
            }
        }
        Ok(maximum)
    }
}
