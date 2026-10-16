//! Bounded dry patch experiment, independent of global water and rendering.
//! Prescribed uplift -> actual downhill incision -> stable hillslope substeps.
use crate::{
    facet_incision, hillslope_transport, landscape, terrain_patch_facets,
    terrain_patch_geometry::{self, Geometry},
};
use serde::Serialize;

pub const MODEL_VERSION: &str = "fine-patch-landscape-1";
pub const FACET_MODEL_VERSION: &str = "fine-patch-facet-landscape-1";
const MAX_WORK: usize = 128_000_000;
const MAX_DIFFUSION_SUBSTEPS: usize = 64;

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    pub erodibility_per_year: f64,
    pub diffusivity_square_meters_per_year: f64,
    pub step_years: f64,
    pub steps: usize,
}
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Routing {
    pub receivers: Vec<u32>,
    pub receiver_distances_meters: Vec<f64>,
    pub outlets: Vec<u32>,
    /// Active reference area only; not rain volume or discharge.
    pub contributing_area_square_meters: Vec<f64>,
    pub closed_sink_count: usize,
    pub closed_fraction_of_active_area: f64,
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Step {
    pub step: usize,
    pub accepted_uplift_prism_cubic_meters: f64,
    pub detached_material_cubic_meters: f64,
    pub signed_hillslope_boundary_exchange_cubic_meters: f64,
    pub gross_requested_hillslope_transfer_cubic_meters: f64,
    pub unresolved_uplift_nodes: usize,
    pub unresolved_incision_nodes: usize,
    pub unresolved_hillslope_node_updates: usize,
    pub relative_prism_ledger_residual: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub receiver_constraint: Option<ConstraintSummary>,
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConstraintSummary {
    pub limited_nodes: usize,
    pub prevented_reference_prism_cubic_meters: f64,
    pub maximum_prevented_incision_meters: f64,
    pub limited_updates: Vec<LimitedUpdate>,
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LimitedUpdate {
    pub node: u32,
    pub prevented_incision_meters: f64,
    pub accepted_incision_meters: f64,
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CumulativeConstraint {
    pub incision_model_version: &'static str,
    pub cumulative_prevented_incision_meters: Vec<f64>,
    pub limited_node_updates: usize,
    pub prevented_reference_prism_cubic_meters: f64,
    pub maximum_prevented_incision_meters: f64,
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Preparation<R = Routing> {
    pub model_version: &'static str,
    pub settings: Settings,
    pub diffusion_substeps_per_step: usize,
    pub elevation_meters: Vec<f64>,
    pub accepted_uplift_displacement_meters: Vec<f64>,
    pub removed_incision_thickness_meters: Vec<f64>,
    pub signed_hillslope_height_change_meters: Vec<f64>,
    pub accepted_uplift_prism_cubic_meters: f64,
    /// Detachment-limited bookkeeping reservoir, NOT material shown to leave
    /// this patch through an outlet and NOT a deposition/soil inventory.
    pub detached_material_reservoir_cubic_meters: f64,
    pub signed_hillslope_boundary_exchange_cubic_meters: f64,
    pub gross_requested_hillslope_transfer_cubic_meters: f64,
    pub relative_prism_ledger_residual: f64,
    pub final_routing: R,
    pub history: Vec<Step>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub receiver_constraint: Option<CumulativeConstraint>,
}
#[derive(Default)]
struct Sum {
    value: f64,
    correction: f64,
}
impl Sum {
    fn add(&mut self, v: f64) -> Result<(), String> {
        let adjusted = v - self.correction;
        let next = self.value + adjusted;
        self.correction = (next - self.value) - adjusted;
        self.value = next;
        if !v.is_finite() || !next.is_finite() || !self.correction.is_finite() {
            return Err("Patch accounting overflowed.".into());
        }
        Ok(())
    }
}
fn ledger(residual: f64, scale: f64) -> Result<f64, String> {
    let relative = if scale == 0. {
        if residual == 0. { 0. } else { f64::INFINITY }
    } else {
        residual.abs() / scale
    };
    if !relative.is_finite() || relative > 1e-10 {
        return Err("Patch displacement/incision/hillslope ledger failed.".into());
    }
    Ok(relative)
}
/// Numerical/topological validation, not authentication of arbitrary mutated
/// coordinates against a recipe. Callers supply geometry from its pinned builder.
fn validate(g: &Geometry, h: &[f64]) -> Result<(), String> {
    g.settings.validate()?;
    let nx = g.settings.cells_x as usize;
    let ny = g.settings.cells_y as usize;
    let n = nx * ny;
    if g.model_version != terrain_patch_geometry::MODEL_VERSION
        || h.len() != n
        || h.iter().any(|v| !v.is_finite())
        || g.cell_directions.len() != n
        || g.reference_areas_square_meters.len() != n
        || g.fixed_external_boundary.len() != n
        || g.offsets.len() != n + 1
        || g.offsets[0] != 0
        || g.neighbors.len() != g.distances_meters.len()
        || g.offsets[n] as usize != g.neighbors.len()
        || g.reference_areas_square_meters
            .iter()
            .any(|v| !v.is_finite() || *v <= 0.)
        || g.distances_meters
            .iter()
            .any(|v| !v.is_finite() || *v <= 0.)
        || g.side_faces.len() != ny * (nx - 1) + nx * (ny - 1)
    {
        return Err("Invalid patch fields or geometry dimensions.".into());
    }
    let mut face_cursor = 0;
    for i in 0..n {
        let row = i / nx;
        let col = i % nx;
        if g.fixed_external_boundary[i] != (row == 0 || col == 0 || row + 1 == ny || col + 1 == nx)
        {
            return Err("Patch requires its recorded fixed external perimeter.".into());
        }
        let start = g.offsets[i] as usize;
        let end = g.offsets[i + 1] as usize;
        if start > end || end > g.neighbors.len() {
            return Err("Invalid patch routing offsets.".into());
        }
        let expected: Vec<_> = (row.saturating_sub(1)..=(row + 1).min(ny - 1))
            .flat_map(|r| (col.saturating_sub(1)..=(col + 1).min(nx - 1)).map(move |c| r * nx + c))
            .filter(|&j| j != i)
            .map(|j| j as u32)
            .collect();
        if g.neighbors[start..end] != expected {
            return Err("Invalid patch routing neighbors.".into());
        }
        for j in [
            (col + 1 < nx).then_some(i + 1),
            (row + 1 < ny).then_some(i + nx),
        ]
        .into_iter()
        .flatten()
        {
            let edge = &g.side_faces[face_cursor];
            face_cursor += 1;
            let k = g.neighbors[start..end].binary_search(&(j as u32)).unwrap() + start;
            if edge.a as usize != i
                || edge.b as usize != j
                || !edge.shared_face_length_meters.is_finite()
                || edge.shared_face_length_meters <= 0.
                || edge.center_distance_meters != g.distances_meters[k]
            {
                return Err("Invalid patch shared-face geometry.".into());
            }
        }
    }
    Ok(())
}

/// Actual adjacent downhill potential. Equal-height plateaus can reach a real
/// fixed-height boundary; enclosed flats retain their canonical dry sink.
pub fn route(g: &Geometry, heights: &[f64]) -> Result<Routing, String> {
    validate(g, heights)?;
    route_validated(g, heights)
}
fn route_validated(g: &Geometry, h: &[f64]) -> Result<Routing, String> {
    let n = h.len();
    let mut receivers: Vec<u32> = (0..n as u32).collect();
    let mut flat_steps = vec![u32::MAX; n];
    for i in 0..n {
        if g.fixed_external_boundary[i] {
            flat_steps[i] = 0;
            continue;
        }
        let mut steepest = 0.;
        let mut equal_boundary = None;
        for k in g.offsets[i] as usize..g.offsets[i + 1] as usize {
            let j = g.neighbors[k] as usize;
            let slope = (h[i] - h[j]) / g.distances_meters[k];
            if !slope.is_finite() {
                return Err("Patch routing slope overflowed.".into());
            }
            if slope > 0. && (slope > steepest || slope == steepest && j < receivers[i] as usize) {
                steepest = slope;
                receivers[i] = j as u32;
            }
            if g.fixed_external_boundary[j] && h[j] == h[i] {
                equal_boundary = Some(equal_boundary.map_or(j, |old: usize| old.min(j)));
            }
        }
        if receivers[i] == i as u32
            && let Some(j) = equal_boundary
        {
            receivers[i] = j as u32;
        }
        if receivers[i] != i as u32 {
            flat_steps[i] = 0;
        }
    }
    let mut visited = vec![false; n];
    let mut component = Vec::new();
    let mut queue = Vec::new();
    for start in 0..n {
        if visited[start] || g.fixed_external_boundary[start] {
            continue;
        }
        component.clear();
        component.push(start);
        visited[start] = true;
        let mut cursor = 0;
        while cursor < component.len() {
            let i = component[cursor];
            cursor += 1;
            for k in g.offsets[i] as usize..g.offsets[i + 1] as usize {
                let j = g.neighbors[k] as usize;
                if !visited[j] && !g.fixed_external_boundary[j] && h[j] == h[i] {
                    visited[j] = true;
                    component.push(j);
                }
            }
        }
        queue.clear();
        queue.extend(component.iter().copied().filter(|&i| flat_steps[i] == 0));
        queue.sort_unstable();
        if queue.is_empty() {
            flat_steps[start] = 0;
            queue.push(start);
        }
        cursor = 0;
        while cursor < queue.len() {
            let i = queue[cursor];
            cursor += 1;
            for k in g.offsets[i] as usize..g.offsets[i + 1] as usize {
                let j = g.neighbors[k] as usize;
                if !g.fixed_external_boundary[j] && h[j] == h[i] && flat_steps[j] == u32::MAX {
                    receivers[j] = i as u32;
                    flat_steps[j] = flat_steps[i] + 1;
                    queue.push(j);
                }
            }
        }
    }
    let mut incoming = vec![0usize; n];
    let mut links = vec![0.; n];
    for (i, &r) in receivers.iter().enumerate() {
        let r = r as usize;
        if r != i {
            incoming[r] += 1;
            let start = g.offsets[i] as usize;
            let k = g.neighbors[start..g.offsets[i + 1] as usize]
                .binary_search(&(r as u32))
                .unwrap()
                + start;
            links[i] = g.distances_meters[k];
        }
    }
    queue.clear();
    queue.extend((0..n).filter(|&i| incoming[i] == 0));
    let mut areas: Vec<_> = g
        .reference_areas_square_meters
        .iter()
        .enumerate()
        .map(|(i, &a)| if g.fixed_external_boundary[i] { 0. } else { a })
        .collect();
    let mut corrections = vec![0.; n];
    let mut cursor = 0;
    while cursor < queue.len() {
        let i = queue[cursor];
        cursor += 1;
        let r = receivers[i] as usize;
        if r != i {
            let adjusted = areas[i] - corrections[r];
            let next = areas[r] + adjusted;
            corrections[r] = (next - areas[r]) - adjusted;
            areas[r] = next;
            if !next.is_finite() {
                return Err("Patch drainage area overflowed.".into());
            }
            incoming[r] -= 1;
            if incoming[r] == 0 {
                queue.push(r);
            }
        }
    }
    if queue.len() != n {
        return Err("Patch drainage must be acyclic.".into());
    }
    let mut outlets = vec![0u32; n];
    for &i in queue.iter().rev() {
        outlets[i] = if receivers[i] as usize == i {
            i as u32
        } else {
            outlets[receivers[i] as usize]
        };
    }
    let mut closed = Sum::default();
    let mut active = Sum::default();
    let mut sinks = 0;
    for i in 0..n {
        if !g.fixed_external_boundary[i] {
            active.add(g.reference_areas_square_meters[i])?;
            if !g.fixed_external_boundary[outlets[i] as usize] {
                closed.add(g.reference_areas_square_meters[i])?;
            }
            if receivers[i] as usize == i {
                sinks += 1;
            }
        }
    }
    Ok(Routing {
        receivers,
        receiver_distances_meters: links,
        outlets,
        contributing_area_square_meters: areas,
        closed_sink_count: sinks,
        closed_fraction_of_active_area: closed.value / active.value,
    })
}

/// Trusted builder geometry, caller-owned initial bed and prescribed rates.
/// No global water or active/pending sediment stock is advanced. The detached
/// reservoir records removed bed without pretending it was redeposited.
pub fn prepare(
    g: &Geometry,
    initial: &[f64],
    uplift: &[f64],
    settings: Settings,
) -> Result<Preparation, String> {
    prepare_impl(g, initial, uplift, settings, SingleReceiver)
}

/// Opt-in physical candidate. Its receiver limiter is an explicit numerical
/// approximation; prevented incision is diagnostic pressure, not an inventory.
pub fn prepare_facets(
    g: &Geometry,
    initial: &[f64],
    uplift: &[f64],
    settings: Settings,
) -> Result<Preparation<terrain_patch_facets::Routing>, String> {
    prepare_impl(
        g,
        initial,
        uplift,
        settings,
        FacetReceivers { router: None },
    )
}

trait PatchProcess {
    type Flow;
    const MODEL: &'static str;
    const STAGE_WORK: usize;
    const CONSTRAINED: bool;
    fn route(&mut self, g: &Geometry, h: &[f64]) -> Result<Self::Flow, String>;
    fn incise(
        &self,
        g: &Geometry,
        h: &[f64],
        flow: &Self::Flow,
        settings: Settings,
    ) -> Result<(landscape::Incision, Option<facet_incision::Constraint>), String>;
}
struct SingleReceiver;
impl PatchProcess for SingleReceiver {
    type Flow = Routing;
    const MODEL: &'static str = MODEL_VERSION;
    const STAGE_WORK: usize = 3;
    const CONSTRAINED: bool = false;
    fn route(&mut self, g: &Geometry, h: &[f64]) -> Result<Self::Flow, String> {
        route_validated(g, h)
    }
    fn incise(
        &self,
        g: &Geometry,
        h: &[f64],
        flow: &Self::Flow,
        settings: Settings,
    ) -> Result<(landscape::Incision, Option<facet_incision::Constraint>), String> {
        let incision = landscape::incise(
            h,
            &g.reference_areas_square_meters,
            &flow.receivers,
            &flow.receiver_distances_meters,
            landscape::Settings {
                erodibility_per_year: settings.erodibility_per_year,
                step_years: settings.step_years,
                base_level_meters: None,
            },
        )?;
        Ok((incision, None))
    }
}
struct FacetReceivers {
    router: Option<terrain_patch_facets::Router>,
}
impl PatchProcess for FacetReceivers {
    type Flow = terrain_patch_facets::Routing;
    const MODEL: &'static str = FACET_MODEL_VERSION;
    const STAGE_WORK: usize = 6;
    const CONSTRAINED: bool = true;
    fn route(&mut self, g: &Geometry, h: &[f64]) -> Result<Self::Flow, String> {
        if self.router.is_none() {
            self.router = Some(terrain_patch_facets::Router::new(g)?);
        }
        self.router.as_ref().unwrap().route(h)
    }
    fn incise(
        &self,
        g: &Geometry,
        h: &[f64],
        flow: &Self::Flow,
        settings: Settings,
    ) -> Result<(landscape::Incision, Option<facet_incision::Constraint>), String> {
        let result = facet_incision::incise(
            h,
            &g.reference_areas_square_meters,
            &flow.contributing_area_square_meters,
            &flow.descents,
            facet_incision::Settings {
                erodibility_per_year: settings.erodibility_per_year,
                step_years: settings.step_years,
            },
        )?;
        Ok((result.incision, Some(result.receiver_constraint)))
    }
}
fn prepare_impl<P: PatchProcess>(
    g: &Geometry,
    initial: &[f64],
    uplift: &[f64],
    settings: Settings,
    mut process: P,
) -> Result<Preparation<P::Flow>, String> {
    validate(g, initial)?;
    let n = initial.len();
    if uplift.len() != n
        || uplift
            .iter()
            .enumerate()
            .any(|(i, v)| !v.is_finite() || *v < 0. || g.fixed_external_boundary[i] && *v != 0.)
        || !(1..=256).contains(&settings.steps)
        || !settings.step_years.is_finite()
        || settings.step_years <= 0.
        || !(settings.step_years * settings.steps as f64).is_finite()
        || !settings.erodibility_per_year.is_finite()
        || settings.erodibility_per_year < 0.
        || !settings.diffusivity_square_meters_per_year.is_finite()
        || settings.diffusivity_square_meters_per_year < 0.
    {
        return Err("Invalid patch preparation fields/settings.".into());
    }
    let mut conductance = vec![0.; n];
    if settings.diffusivity_square_meters_per_year > 0. {
        for e in &g.side_faces {
            let weight = settings.diffusivity_square_meters_per_year * e.shared_face_length_meters
                / e.center_distance_meters;
            if !weight.is_finite() || weight <= 0. {
                return Err("Unrepresentable patch diffusion rate.".into());
            }
            for i in [e.a as usize, e.b as usize] {
                if !g.fixed_external_boundary[i] {
                    conductance[i] += weight / g.reference_areas_square_meters[i];
                }
            }
        }
    }
    let cfl = conductance.into_iter().fold(0_f64, f64::max) * settings.step_years;
    if !cfl.is_finite() || cfl / 0.8 > MAX_DIFFUSION_SUBSTEPS as f64 {
        return Err("Excessive patch diffusion substeps.".into());
    }
    let substeps = if settings.diffusivity_square_meters_per_year == 0. {
        0
    } else {
        (cfl / 0.8).ceil().max(1.) as usize
    };
    if n.checked_mul(settings.steps)
        .and_then(|v| v.checked_mul(P::STAGE_WORK + substeps))
        .is_none_or(|v| v > MAX_WORK)
    {
        return Err("Excessive patch preparation workload.".into());
    }
    let mut bed = initial.to_vec();
    let mut displacement = vec![0.; n];
    let mut erosion = vec![0.; n];
    let mut lateral = vec![0.; n];
    let mut accepted_u = Sum::default();
    let mut detached = Sum::default();
    let mut boundary = Sum::default();
    let mut gross = Sum::default();
    let mut history = Vec::with_capacity(settings.steps);
    let mut receiver_constraint = P::CONSTRAINED.then(|| CumulativeConstraint {
        incision_model_version: facet_incision::MODEL_VERSION,
        cumulative_prevented_incision_meters: vec![0.; n],
        limited_node_updates: 0,
        prevented_reference_prism_cubic_meters: 0.,
        maximum_prevented_incision_meters: 0.,
    });
    let mut prevented = Sum::default();
    for step in 0..settings.steps {
        let before = bed.clone();
        let mut u = Sum::default();
        let mut unresolved_u = 0;
        for i in 0..n {
            if uplift[i] == 0. {
                continue;
            }
            let requested = uplift[i] * settings.step_years;
            let updated = bed[i] + requested;
            let accepted = updated - bed[i];
            if !requested.is_finite() || requested <= 0. || !updated.is_finite() || accepted < 0. {
                return Err("Unrepresentable patch uplift.".into());
            }
            if accepted == 0. {
                unresolved_u += 1;
            }
            let prism = accepted * g.reference_areas_square_meters[i];
            if accepted > 0. && prism == 0. {
                return Err("Unrepresentable patch uplift prism.".into());
            }
            u.add(prism)?;
            displacement[i] += accepted;
            bed[i] = updated;
        }
        let routing = process.route(g, &bed)?;
        let (incision, constraint) = process.incise(g, &bed, &routing, settings)?;
        let constraint_summary = if let Some(c) = constraint {
            let cumulative = receiver_constraint.as_mut().unwrap();
            for (sum, &value) in cumulative
                .cumulative_prevented_incision_meters
                .iter_mut()
                .zip(&c.prevented_incision_meters)
            {
                *sum += value;
                if !sum.is_finite() {
                    return Err("Cumulative facet constraint overflowed.".into());
                }
            }
            cumulative.limited_node_updates += c.limited_nodes;
            prevented.add(c.prevented_reference_prism_cubic_meters)?;
            cumulative.prevented_reference_prism_cubic_meters = prevented.value;
            cumulative.maximum_prevented_incision_meters = cumulative
                .maximum_prevented_incision_meters
                .max(c.maximum_prevented_incision_meters);
            Some(ConstraintSummary {
                limited_nodes: c.limited_nodes,
                prevented_reference_prism_cubic_meters: c.prevented_reference_prism_cubic_meters,
                maximum_prevented_incision_meters: c.maximum_prevented_incision_meters,
                limited_updates: c
                    .prevented_incision_meters
                    .iter()
                    .enumerate()
                    .filter(|(_, v)| **v > 0.)
                    .map(|(i, &value)| LimitedUpdate {
                        node: i as u32,
                        prevented_incision_meters: value,
                        accepted_incision_meters: incision.removed_thickness_meters[i],
                    })
                    .collect(),
            })
        } else {
            None
        };
        for (sum, &v) in erosion.iter_mut().zip(&incision.removed_thickness_meters) {
            *sum += v;
        }
        bed = incision.elevation_meters;
        let mut b = Sum::default();
        let mut q = Sum::default();
        let mut unresolved_d = 0;
        for _ in 0..substeps {
            let t = hillslope_transport::transport(
                &bed,
                &g.reference_areas_square_meters,
                &g.fixed_external_boundary,
                &g.side_faces,
                hillslope_transport::Settings {
                    diffusivity_square_meters_per_year: settings.diffusivity_square_meters_per_year,
                    step_years: settings.step_years / substeps as f64,
                },
            )?;
            for (sum, &v) in lateral.iter_mut().zip(&t.accepted_height_change_meters) {
                *sum += v;
            }
            b.add(t.requested_boundary_exchange_cubic_meters)?;
            q.add(t.gross_requested_transfer_cubic_meters)?;
            unresolved_d += t.unresolved_requested_change_nodes;
            bed = t.elevation_meters;
        }
        let mut net = Sum::default();
        for i in 0..n {
            net.add((bed[i] - before[i]) * g.reference_areas_square_meters[i])?;
        }
        let e = incision.removed_material_cubic_meters;
        let residual = ledger(
            net.value - (u.value - e + b.value),
            u.value.max(e).max(q.value).max(b.value.abs()),
        )?;
        accepted_u.add(u.value)?;
        detached.add(e)?;
        boundary.add(b.value)?;
        gross.add(q.value)?;
        history.push(Step {
            step: step + 1,
            accepted_uplift_prism_cubic_meters: u.value,
            detached_material_cubic_meters: e,
            signed_hillslope_boundary_exchange_cubic_meters: b.value,
            gross_requested_hillslope_transfer_cubic_meters: q.value,
            unresolved_uplift_nodes: unresolved_u,
            unresolved_incision_nodes: incision.unresolved_positive_incision_nodes,
            unresolved_hillslope_node_updates: unresolved_d,
            relative_prism_ledger_residual: residual,
            receiver_constraint: constraint_summary,
        });
    }
    let mut net = Sum::default();
    for i in 0..n {
        net.add((bed[i] - initial[i]) * g.reference_areas_square_meters[i])?;
    }
    let residual = ledger(
        net.value - (accepted_u.value - detached.value + boundary.value),
        accepted_u
            .value
            .max(detached.value)
            .max(gross.value)
            .max(boundary.value.abs()),
    )?;
    let final_routing = process.route(g, &bed)?;
    Ok(Preparation {
        model_version: P::MODEL,
        settings,
        diffusion_substeps_per_step: substeps,
        elevation_meters: bed,
        accepted_uplift_displacement_meters: displacement,
        removed_incision_thickness_meters: erosion,
        signed_hillslope_height_change_meters: lateral,
        accepted_uplift_prism_cubic_meters: accepted_u.value,
        detached_material_reservoir_cubic_meters: detached.value,
        signed_hillslope_boundary_exchange_cubic_meters: boundary.value,
        gross_requested_hillslope_transfer_cubic_meters: gross.value,
        relative_prism_ledger_residual: residual,
        final_routing,
        history,
        receiver_constraint,
    })
}
