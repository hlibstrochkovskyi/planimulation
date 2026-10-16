//! Unit-runoff transport: the volume-weighted negative transpose of a frozen
//! facet directional derivative. No water inventory or erosion is advanced.
use crate::{
    terrain_patch_facets::{Descent, Router},
    terrain_patch_geometry,
};
use serde::Serialize;

pub const MODEL_VERSION: &str = "facet-specific-transport-1";

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Routing {
    pub model_version: &'static str,
    /// Derivative-normalized flux shares, NOT the old angular area shares.
    pub descents: Vec<Descent>,
    pub contributing_area_square_meters: Vec<f64>,
    /// Terminals have no outgoing width or specific-area pressure (JSON null).
    pub outgoing_width_meters: Vec<Option<f64>>,
    pub specific_area_meters: Vec<Option<f64>>,
    /// Unit-runoff geometry flux; not cubic meters or measured discharge.
    pub edge_flux_square_meters: Vec<[f64; 2]>,
    pub upstream_to_downstream: Vec<u32>,
    pub closed_sink_count: usize,
    pub closed_fraction_of_active_area: f64,
    pub terminal_area_relative_residual: f64,
    pub maximum_local_flux_relative_residual: f64,
}

#[derive(Default)]
struct Sum {
    value: f64,
    correction: f64,
}
impl Sum {
    fn add(&mut self, value: f64) -> Result<(), String> {
        let delta = value - self.correction;
        let next = self.value + delta;
        self.correction = (next - self.value) - delta;
        self.value = next;
        if !value.is_finite() || value < 0. || !next.is_finite() || !self.correction.is_finite() {
            return Err("Specific-area accumulation overflowed.".into());
        }
        Ok(())
    }
}

pub fn route(router: &Router, h: &[f64]) -> Result<Routing, String> {
    let old = router.route(h)?;
    let g = router.geometry();
    accumulate(
        &g.reference_areas_square_meters,
        &g.fixed_external_boundary,
        &old.descents,
    )
}

/// Bounded static graph primitive. Callers must derive coefficients and heights
/// from the same geometry; this checks numbers/topology, not recipe provenance.
pub fn accumulate(areas: &[f64], fixed: &[bool], descents: &[Descent]) -> Result<Routing, String> {
    let n = areas.len();
    if !(1..=terrain_patch_geometry::MAX_CELLS).contains(&n)
        || fixed.len() != n
        || descents.len() != n
        || areas.iter().any(|a| !a.is_finite() || *a <= 0.)
    {
        return Err("Invalid specific-area fields.".into());
    }
    let mut ds = descents.to_vec();
    let mut incoming = vec![0usize; n];
    let mut widths = vec![None; n];
    for (i, d) in ds.iter_mut().enumerate() {
        if !d.slope.is_finite()
            || d.slope < 0.
            || d.unit_tangent_direction.iter().any(|v| !v.is_finite())
            || d.receivers.iter().any(|&j| j as usize >= n)
            || d.area_weights
                .iter()
                .any(|w| !w.is_finite() || !(0. ..=1.).contains(w))
            || d.derivative_coefficients_per_meter
                .iter()
                .any(|a| !a.is_finite() || *a < 0.)
        {
            return Err("Invalid specific-area descent.".into());
        }
        let mut count = 0;
        for k in 0..2 {
            let w = d.area_weights[k];
            let a = d.derivative_coefficients_per_meter[k];
            if w == 0. {
                if a != 0. {
                    return Err("Inactive specific-area derivative is nonzero.".into());
                }
            } else {
                let j = d.receivers[k] as usize;
                if a <= 0. || j == i || fixed[i] {
                    return Err("Invalid active specific-area receiver.".into());
                }
                count += 1;
                incoming[j] += 1;
            }
        }
        if count == 0 {
            continue;
        }
        if (d.area_weights.iter().sum::<f64>() - 1.).abs() > 1e-12
            || count == 2 && d.receivers[0] == d.receivers[1]
        {
            return Err("Invalid specific-area partition.".into());
        }
        let alpha: f64 = d.derivative_coefficients_per_meter.iter().sum();
        let width = areas[i] * alpha;
        if !alpha.is_finite() || !width.is_finite() || width <= 0. {
            return Err("Unrepresentable outgoing transport width.".into());
        }
        for k in 0..2 {
            d.area_weights[k] = d.derivative_coefficients_per_meter[k] / alpha;
            if d.derivative_coefficients_per_meter[k] > 0. && d.area_weights[k] == 0. {
                return Err("Unrepresentable positive transport share.".into());
            }
        }
        widths[i] = Some(width);
    }
    let mut collected: Vec<_> = areas
        .iter()
        .zip(fixed)
        .map(|(&a, &b)| Sum {
            value: if b { 0. } else { a },
            correction: 0.,
        })
        .collect();
    let mut order: Vec<_> = (0..n)
        .filter(|&i| incoming[i] == 0)
        .map(|i| i as u32)
        .collect();
    let mut specific = vec![None; n];
    let mut flux = vec![[0.; 2]; n];
    let mut max_residual = 0_f64;
    let mut cursor = 0;
    while cursor < order.len() {
        let i = order[cursor] as usize;
        cursor += 1;
        if let Some(width) = widths[i] {
            let a = collected[i].value / width;
            if !a.is_finite() || a <= 0. {
                return Err("Unrepresentable specific area.".into());
            }
            specific[i] = Some(a);
            for (k, edge_flux) in flux[i].iter_mut().enumerate() {
                if ds[i].area_weights[k] == 0. {
                    continue;
                }
                let j = ds[i].receivers[k] as usize;
                // Exact discrete transport definition; shares are alpha/sum.
                let f = collected[i].value * ds[i].area_weights[k];
                if !f.is_finite() || f <= 0. {
                    return Err("Unrepresentable positive transport flux.".into());
                }
                *edge_flux = f;
                collected[j].add(f)?;
                incoming[j] -= 1;
                if incoming[j] == 0 {
                    order.push(j as u32);
                }
            }
            let residual =
                (flux[i].iter().sum::<f64>() - collected[i].value).abs() / collected[i].value;
            if !residual.is_finite() || residual > 1e-12 {
                return Err("Specific-area local flux budget failed.".into());
            }
            max_residual = max_residual.max(residual);
        }
    }
    if order.len() != n {
        return Err("Specific-area graph must be acyclic.".into());
    }
    let (mut source, mut terminal, mut closed) = (Sum::default(), Sum::default(), Sum::default());
    let mut sinks = 0;
    for i in 0..n {
        if !fixed[i] {
            source.add(areas[i])?;
        }
        if widths[i].is_none() {
            terminal.add(collected[i].value)?;
            if !fixed[i] {
                closed.add(collected[i].value)?;
                sinks += 1;
            }
        }
    }
    if source.value <= 0. {
        return Err("Specific-area graph requires an active source.".into());
    }
    let residual = (terminal.value - source.value).abs() / source.value;
    if !residual.is_finite() || residual > 1e-12 {
        return Err("Specific-area terminal budget failed.".into());
    }
    Ok(Routing {
        model_version: MODEL_VERSION,
        descents: ds,
        contributing_area_square_meters: collected.into_iter().map(|s| s.value).collect(),
        outgoing_width_meters: widths,
        specific_area_meters: specific,
        edge_flux_square_meters: flux,
        upstream_to_downstream: order,
        closed_sink_count: sinks,
        closed_fraction_of_active_area: closed.value / source.value,
        terminal_area_relative_residual: residual,
        maximum_local_flux_relative_residual: max_residual,
    })
}
