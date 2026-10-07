use super::*;
use crate::moisture_transport::total_mass;

fn add_signed(terms: &mut Vec<f64>, mass: Mass, sign: f64) {
    terms.extend([sign * mass.high, sign * mass.low]);
}
fn check(terms: &[f64], maximum: &mut f64) -> Result<(), String> {
    let scale = terms.iter().map(|v| v.abs()).fold(1., f64::max);
    let residual = total_mass(terms).abs() / scale;
    if !residual.is_finite() || residual > 128. * f64::EPSILON {
        return Err(format!(
            "Regional seasonal local ledger exceeds tolerance: {residual}."
        ));
    }
    *maximum = maximum.max(residual);
    Ok(())
}
impl Model {
    pub(super) fn validate(&self, cp: &Checkpoint) -> Result<Budget, String> {
        fn pins(c: &Checkpoint) -> [&str; 9] {
            [
                c.model_version.as_str(),
                c.soil_model_version.as_str(),
                c.transport_model_version.as_str(),
                c.surface_flow_model_version.as_str(),
                c.runoff_model_version.as_str(),
                c.temperature_model_version.as_str(),
                c.wind_model_version.as_str(),
                c.orographic_model_version.as_str(),
                c.reference_body_model_version.as_str(),
            ]
        }
        if cp.schema_version != self.origin.schema_version
            || pins(cp) != pins(&self.origin)
            || cp.recipe != self.origin.recipe
            || cp.settings != self.origin.settings
            || cp.temperature_settings != self.origin.temperature_settings
            || cp.wind_settings != self.origin.wind_settings
            || cp.elapsed_seconds > super::super::MAX_ELAPSED_SECONDS
        {
            return Err("Regional seasonal metadata or model versions do not match.".into());
        }
        let n = self.forcing.areas.len();
        if cp.resolution.is_some() != self.origin.resolution.is_some() {
            return Err(
                "Regional numerical policy requires matching resolution diagnostics.".into(),
            );
        }
        if let Some(resolution) = cp.resolution {
            resolution.validate()?;
        }
        let contacts = self.forcing.flows[0].directed_contacts();
        let pool = self.forcing.reference_pool.as_ref().unwrap();
        if [
            cp.liquid.len(),
            cp.soil.len(),
            cp.snow.len(),
            cp.vapor.len(),
            cp.drainage.len(),
            cp.local_transfers.len(),
            cp.drainage_sent.len(),
        ]
        .iter()
        .any(|&len| len != n)
            || cp.atmospheric_transfers.len() != contacts.len()
            || cp.surface_transfers.len() != contacts.len()
            || cp.reference_bodies.len() != pool.ids.len()
        {
            return Err("Regional seasonal stock or graph-history shape does not match.".into());
        }
        if cp.elapsed_seconds == 0 && cp != &self.origin {
            return Err(
                "Regional seasonal day-zero state is not its exact initial partition.".into(),
            );
        }
        for masses in [
            &cp.liquid,
            &cp.soil,
            &cp.snow,
            &cp.vapor,
            &cp.drainage,
            &cp.reference_bodies,
            &cp.atmospheric_transfers,
            &cp.surface_transfers,
            &cp.drainage_sent,
        ] {
            for mass in masses {
                mass.stock(f64::MAX)?;
            }
        }
        let mut atmospheric = vec![Vec::<f64>::new(); n];
        let mut surface = vec![Vec::<f64>::new(); n];
        for (edge, &[source, target]) in contacts.iter().enumerate() {
            let atm = cp.atmospheric_transfers[edge];
            add_signed(&mut atmospheric[source], atm, 1.);
            add_signed(&mut atmospheric[target], atm, -1.);
            let water = cp.surface_transfers[edge];
            if !self.forcing.is_land[source] && water.high != 0. {
                return Err("Reference body cannot supply regional face outflow.".into());
            }
            add_signed(&mut surface[source], water, 1.);
            add_signed(&mut surface[target], water, -1.);
        }
        let mut route_in = vec![Vec::<f64>::new(); n];
        for (r, mass) in cp.drainage_sent.iter().enumerate() {
            let target = self.forcing.routing.receivers()[r] as usize;
            add_signed(&mut route_in[target], *mass, -1.);
            if !cp.settings.routing_enabled && mass.high != 0. {
                return Err("Drainage transfers while routing is disabled.".into());
            }
        }
        let mut body_terms: Vec<Vec<f64>> = cp
            .reference_bodies
            .iter()
            .zip(&self.origin.reference_bodies)
            .map(|(&a, &b)| vec![a.high, a.low, -b.high, -b.low])
            .collect();
        let mut maximum = 0.;
        for r in 0..n {
            let f = cp.local_transfers[r];
            for mass in f.values() {
                mass.stock(f64::MAX)?;
            }
            if (!cp.settings.precipitation_enabled && (f.rain.high != 0. || f.snowfall.high != 0.))
                || (!cp.settings.evaporation_enabled
                    && (f.liquid_evaporation.high != 0. || f.soil_evaporation.high != 0.))
            {
                return Err("Regional seasonal transfers contradict disabled forcing.".into());
            }
            cp.soil[r].stock(
                self.forcing.areas[r] * cp.settings.soil.soil_capacity_kilograms_per_square_meter,
            )?;
            let mut soil = vec![cp.soil[r].high, cp.soil[r].low];
            add_signed(&mut soil, f.infiltration, -1.);
            add_signed(&mut soil, f.soil_evaporation, 1.);
            add_signed(&mut soil, f.soil_drainage, 1.);
            check(&soil, &mut maximum)?;
            let mut snow = vec![cp.snow[r].high, cp.snow[r].low];
            add_signed(&mut snow, f.snowfall, -1.);
            add_signed(&mut snow, f.melt, 1.);
            check(&snow, &mut maximum)?;
            let mut vapor = vec![cp.vapor[r].high, cp.vapor[r].low];
            for mass in [f.liquid_evaporation, f.soil_evaporation] {
                add_signed(&mut vapor, mass, -1.);
            }
            for mass in [f.rain, f.snowfall] {
                add_signed(&mut vapor, mass, 1.);
            }
            vapor.extend(&atmospheric[r]);
            check(&vapor, &mut maximum)?;
            let mut drainage = vec![cp.drainage[r].high, cp.drainage[r].low];
            add_signed(&mut drainage, f.soil_drainage, -1.);
            add_signed(&mut drainage, cp.drainage_sent[r], 1.);
            if !self.forcing.routing.is_terminal(r) {
                drainage.extend(&route_in[r]);
            }
            check(&drainage, &mut maximum)?;
            if let Some(body) = pool.by_region[r] {
                if [
                    cp.liquid[r],
                    cp.soil[r],
                    cp.drainage[r],
                    f.infiltration,
                    f.soil_evaporation,
                    f.soil_drainage,
                    cp.drainage_sent[r],
                ]
                .iter()
                .any(|v| v.high != 0.)
                {
                    return Err("Reference region has duplicated terrestrial ownership.".into());
                }
                let terms = &mut body_terms[body];
                add_signed(terms, f.rain, -1.);
                add_signed(terms, f.melt, -1.);
                add_signed(terms, f.liquid_evaporation, 1.);
                terms.extend(&surface[r]);
                terms.extend(&route_in[r]);
            } else {
                let mut liquid = vec![cp.liquid[r].high, cp.liquid[r].low];
                add_signed(&mut liquid, f.rain, -1.);
                add_signed(&mut liquid, f.melt, -1.);
                add_signed(&mut liquid, f.liquid_evaporation, 1.);
                add_signed(&mut liquid, f.infiltration, 1.);
                liquid.extend(&surface[r]);
                if self.forcing.routing.is_terminal(r) {
                    liquid.extend(&route_in[r]);
                }
                check(&liquid, &mut maximum)?;
            }
        }
        for terms in &body_terms {
            check(terms, &mut maximum)?;
        }
        let initial = total_mass(
            &self
                .origin
                .reference_bodies
                .iter()
                .flat_map(|v| [v.high, v.low])
                .collect::<Vec<_>>(),
        );
        let stored = total_mass(
            &[
                &cp.liquid,
                &cp.soil,
                &cp.snow,
                &cp.vapor,
                &cp.drainage,
                &cp.reference_bodies,
            ]
            .iter()
            .flat_map(|v| v.iter().flat_map(|m| [m.high, m.low]))
            .collect::<Vec<_>>(),
        );
        let global = (stored - initial).abs() / initial.max(1.);
        if !initial.is_finite() || !stored.is_finite() || !global.is_finite() || global > 1e-12 {
            return Err("Regional seasonal global water identity exceeds tolerance.".into());
        }
        Ok(Budget {
            initial_kilograms: initial,
            stored_kilograms: stored,
            relative_global_residual: global,
            maximum_relative_local_residual: maximum,
        })
    }
}
