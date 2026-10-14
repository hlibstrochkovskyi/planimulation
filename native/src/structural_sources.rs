//! Initial geological origins on a pinned canonical reference mesh, independent
//! of target terrain sampling. Fixed discretization is an approximation, not
//! continuous tectonic evolution or an unweighted Voronoi partition.
use crate::{
    Recipe, Surface, continental_uplands,
    geodesic_envelope::{self, Source},
    plate_roots,
    tectonics::Tectonics,
    terrain::boundary_amplitudes,
};
use serde::Serialize;

pub const MODEL_VERSION: &str = "fixed-geological-sources-1";
pub const REFERENCE_SUBDIVISION: u32 = 5;
pub const REFERENCE_REGION_COUNT: usize = 10 * 4_usize.pow(REFERENCE_SUBDIVISION) + 2;

/// Construction identity, not a hash of arbitrary caller-supplied arrays. Only
/// inputs affecting raw origins/amplitudes appear here. Width, relief scaling,
/// target resolution, local detail and water belong to subsequent composition.
/// Private fields and no deserializer prevent unchecked provenance construction.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Identity {
    source_model_version: &'static str,
    envelope_model_version: &'static str,
    plate_root_version: &'static str,
    crust_model_version: &'static str,
    random_version: String,
    reference_subdivision: u32,
    seed: String,
    radius_meters: f64,
    plate_count: u32,
    max_plate_speed_cm_per_year: f64,
    continental_fraction: f64,
    continental_scale: f64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Sources {
    pub model_version: &'static str,
    pub reference_subdivision: u32,
    pub reference_region_count: usize,
    pub identity: Identity,
    /// All canonical sampled half-dual arcs, including stopped/zero amplitudes.
    /// Order is independent of the target grid; IDs are local to this identity.
    pub sources: Vec<Source>,
}

/// Build trusted components together; accept no target grid or unverified crust,
/// plate or terrain snapshots. Reuse the existing continuous root, resistance,
/// motion and crust streams without consuming any terrain/detail/water stream.
pub fn build(reference: &Recipe) -> Result<Sources, String> {
    reference.validate()?;
    if reference.model_version != continental_uplands::MODEL_VERSION
        || reference.terrain_preparation_passes != Some(0)
        || reference.plate_count as usize > REFERENCE_REGION_COUNT
    {
        return Err("Fixed geological sources require an unprepared v2 reference recipe.".into());
    }
    let surface = Surface::build(REFERENCE_SUBDIVISION, reference.radius_meters);
    let crust = continental_uplands::build_crust(
        &surface,
        &reference.seed,
        reference.continental_fraction,
        reference.continental_scale,
    )?;
    let plates = Tectonics::build_with_continuous_roots(
        &surface,
        &reference.seed,
        reference.plate_count,
        reference.max_plate_speed_cm_per_year,
        reference.radius_meters,
    )?;
    let count = plates.boundary_types.len();
    if count > geodesic_envelope::MAX_SOURCES {
        return Err("Canonical geological source count exceeds its bound.".into());
    }
    let mut sources = Vec::with_capacity(count);
    for segment in 0..count {
        let a = plates.boundary_cells[segment * 2] as usize;
        let b = plates.boundary_cells[segment * 2 + 1] as usize;
        let amplitudes = boundary_amplitudes(
            plates.boundary_motion[segment * 2],
            (crust.continentality[a] + crust.continentality[b]) * 0.5,
        );
        sources.push(Source::new(
            plates.boundary_directions[segment * 2],
            plates.boundary_directions[segment * 2 + 1],
            amplitudes,
        )?);
    }
    Ok(Sources {
        model_version: MODEL_VERSION,
        reference_subdivision: REFERENCE_SUBDIVISION,
        reference_region_count: surface.centers.len(),
        identity: Identity {
            source_model_version: MODEL_VERSION,
            envelope_model_version: geodesic_envelope::MODEL_VERSION,
            plate_root_version: plate_roots::PROPOSAL_VERSION,
            crust_model_version: continental_uplands::MODEL_VERSION,
            random_version: reference.random_version.clone(),
            reference_subdivision: REFERENCE_SUBDIVISION,
            seed: reference.seed.clone(),
            radius_meters: reference.radius_meters,
            plate_count: reference.plate_count,
            max_plate_speed_cm_per_year: reference.max_plate_speed_cm_per_year,
            continental_fraction: reference.continental_fraction,
            continental_scale: reference.continental_scale,
        },
        sources,
    })
}
