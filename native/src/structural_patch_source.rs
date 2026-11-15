//! Procedural initial bed from geological potential and a drainage scaffold.
//! Not erosion-time integration, funded lakes or mutation of an existing World.
use crate::{
    drainage_landforms,
    irregular_patch::{self, Geometry},
};
use serde::Serialize;
use std::{cmp::Ordering, collections::BinaryHeap};

pub const MODEL_VERSION: &str = "drainage-scaffold-initial-bed-1";
#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    /// For the fixed m=.5,n=1 profile expression; units 1/year.
    pub erodibility_per_year: f64,
    /// Dimensionless cap on the profile contribution along scaffold links.
    /// Geological potential floors can require a larger actual difference.
    pub maximum_profile_slope: f64,
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Source {
    pub model_version: &'static str,
    pub settings: Settings,
    pub potential_levels_meters: Vec<f64>,
    /// A topological generation scaffold, NOT the resulting actual dry route.
    pub scaffold_receivers: Vec<u32>,
    pub scaffold_contributing_area_square_meters: Vec<f64>,
    pub initial_bed_meters: Vec<f64>,
    pub slope_capped_nodes: usize,
    pub terminal_area_relative_residual: f64,
}
#[derive(Clone, Copy)]
struct Entry(f64, usize);
impl PartialEq for Entry {
    fn eq(&self, o: &Self) -> bool {
        self.0.total_cmp(&o.0) == Ordering::Equal && self.1 == o.1
    }
}
impl Eq for Entry {}
impl PartialOrd for Entry {
    fn partial_cmp(&self, o: &Self) -> Option<Ordering> {
        Some(self.cmp(o))
    }
}
impl Ord for Entry {
    fn cmp(&self, o: &Self) -> Ordering {
        o.0.total_cmp(&self.0).then_with(|| o.1.cmp(&self.1))
    }
}

pub fn build(
    g: &Geometry,
    potential: &[f64],
    uplift_meters_per_year: &[f64],
    settings: Settings,
) -> Result<Source, String> {
    let n = potential.len();
    if g.model_version != irregular_patch::MODEL_VERSION
        || g.chart_points_meters.len() != n
        || uplift_meters_per_year.len() != n
        || !settings.erodibility_per_year.is_finite()
        || settings.erodibility_per_year <= 0.
        || !settings.maximum_profile_slope.is_finite()
        || !(0. ..=2.).contains(&settings.maximum_profile_slope)
        || settings.maximum_profile_slope == 0.
        || uplift_meters_per_year.iter().enumerate().any(|(i, u)| {
            !u.is_finite()
                || *u < 0.
                || g.fixed_external_boundary.get(i).copied().unwrap_or(true) && *u != 0.
        })
    {
        return Err("Invalid structural initial-bed settings or fields.".into());
    }
    // Reuse graph shape/topology/finite-field validation. This returned dry
    // graph is deliberately NOT the minimax scaffold used below.
    drainage_landforms::dry_receivers(
        potential,
        &g.reference_areas_square_meters,
        &g.fixed_external_boundary,
        &g.edges,
    )?;
    if !g.fixed_external_boundary.iter().any(|v| *v) {
        return Err("Structural patch needs explicit external terminals.".into());
    }
    let mut adjacent = vec![Vec::new(); n];
    for e in &g.edges {
        adjacent[e.a as usize].push((e.b as usize, e.distance_meters));
        adjacent[e.b as usize].push((e.a as usize, e.distance_meters));
    }
    for a in &mut adjacent {
        a.sort_unstable_by_key(|v| v.0);
    }
    let mut level = vec![f64::INFINITY; n];
    let mut parent: Vec<_> = (0..n as u32).collect();
    let mut distance = vec![0.; n];
    let mut settled = vec![false; n];
    let mut order = Vec::with_capacity(n);
    let mut heap = BinaryHeap::new();
    for i in 0..n {
        if g.fixed_external_boundary[i] {
            level[i] = potential[i];
            heap.push(Entry(level[i], i));
        }
    }
    while let Some(Entry(value, i)) = heap.pop() {
        if settled[i] {
            continue;
        }
        settled[i] = true;
        order.push(i);
        for &(j, length) in &adjacent[i] {
            let candidate = value.max(potential[j]);
            if !settled[j] && candidate < level[j] {
                level[j] = candidate;
                parent[j] = i as u32;
                distance[j] = length;
                heap.push(Entry(candidate, j));
            }
        }
    }
    if order.len() != n {
        return Err("Structural initial-bed scaffold has unreachable points.".into());
    }
    let mut area: Vec<_> = g
        .reference_areas_square_meters
        .iter()
        .zip(&g.fixed_external_boundary)
        .map(|(&a, &fixed)| if fixed { 0. } else { a })
        .collect();
    let active: f64 = area.iter().sum();
    let mut corrections = vec![0.; n];
    for &i in order.iter().rev() {
        let r = parent[i] as usize;
        if r == i {
            continue;
        }
        let amount = area[i] - corrections[r];
        let next = area[r] + amount;
        corrections[r] = (next - area[r]) - amount;
        area[r] = next;
    }
    let terminal: f64 = (0..n)
        .filter(|&i| parent[i] as usize == i)
        .map(|i| area[i])
        .sum();
    let residual = if active == 0. {
        if terminal == 0. { 0. } else { f64::INFINITY }
    } else {
        (terminal - active).abs() / active
    };
    if !residual.is_finite() || residual > 1e-12 || area.iter().any(|a| !a.is_finite() || *a < 0.) {
        return Err("Structural initial-bed source areas do not reconcile.".into());
    }
    let mut initial = level.clone();
    let mut capped = 0;
    for &i in &order {
        let r = parent[i] as usize;
        if r == i {
            continue;
        }
        let denominator = settings.erodibility_per_year * area[i].sqrt();
        let grade = uplift_meters_per_year[i] / denominator;
        if !denominator.is_finite() || denominator <= 0. || !grade.is_finite() {
            return Err("Unrepresentable structural profile grade.".into());
        }
        let slope = if grade > settings.maximum_profile_slope {
            capped += 1;
            settings.maximum_profile_slope
        } else {
            grade
        };
        let profile = initial[r] + slope * distance[i];
        initial[i] = level[i].max(profile);
        if !initial[i].is_finite() || (slope > 0. && initial[i] <= initial[r]) {
            return Err("Unrepresentable structural initial height.".into());
        }
    }
    Ok(Source {
        model_version: MODEL_VERSION,
        settings,
        potential_levels_meters: level,
        scaffold_receivers: parent,
        scaffold_contributing_area_square_meters: area,
        initial_bed_meters: initial,
        slope_capped_nodes: capped,
        terminal_area_relative_residual: residual,
    })
}
