//! Controlled initial-world ablations. Target-grid sampled origins remain
//! mesh-dependent; fixed origins use a separately pinned geological mesh.
use crate::{
    Recipe, Surface,
    crust::Crust,
    geodesic_envelope::{Field, Settings, Source},
    geography, structural_sources,
    tectonics::Tectonics,
    terrain::{Terrain, boundary_amplitudes},
};
use serde::Serialize;
#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Origin {
    RegionSeeds,
    BoundaryArcs,
    FixedBoundaryArcs,
}
impl Origin {
    pub fn model_version(self) -> &'static str {
        match self {
            Self::RegionSeeds => "geodesic-region-seeds-1",
            Self::BoundaryArcs => "geodesic-sampled-arcs-1",
            Self::FixedBoundaryArcs => structural_sources::MODEL_VERSION,
        }
    }
}
#[derive(Debug)]
pub struct Initial {
    pub terrain: Terrain,
    pub sources: Vec<Source>,
    pub origin: Origin,
    pub mean_evaluated_sources: f64,
    pub maximum_evaluated_sources: usize,
    pub source_identity: Option<structural_sources::Identity>,
}
fn angle(a: [f64; 3], b: [f64; 3]) -> f64 {
    let cross = [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ];
    cross
        .iter()
        .map(|v| v * v)
        .sum::<f64>()
        .sqrt()
        .atan2(a.into_iter().zip(b).map(|(a, b)| a * b).sum())
}
/// V2 crust and detail are retained exactly. No smoothing/history/water is
/// performed. Caller owns inputs; errors never mutate them or old models.
pub fn build(
    s: &Surface,
    crust: &Crust,
    plates: &Tectonics,
    reference: &Recipe,
    old: &Terrain,
    origin: Origin,
) -> Result<Initial, String> {
    reference.validate()?;
    let n = s.centers.len();
    let m = plates.boundary_types.len();
    if reference.model_version != "multiscale-geography-2"
        || reference.terrain_preparation_passes != Some(0)
        || !(1..=1_000_000).contains(&n)
        || crust.continentality.len() != n
        || old.elevation.len() != n
        || old.baseline.len() != n
        || old.detail.len() != n
        || old.convergence.len() != n
        || old.divergence.len() != n
        || old.applied_passes != 0
        || old.transported_cubic_meters != 0.
        || old.preparation.len() != n
        || old.preparation.iter().any(|&v| v != 0.)
        || m > crate::geodesic_envelope::MAX_SOURCES
        || plates.boundary_cells.len() != m * 2
        || plates.boundary_directions.len() != m * 2
        || plates.boundary_motion.len() != m * 2
        || crust
            .continentality
            .iter()
            .any(|v| !v.is_finite() || !(0. ..=1.).contains(v))
        || old
            .baseline
            .iter()
            .chain(&old.detail)
            .chain(&old.convergence)
            .chain(&old.divergence)
            .chain(&old.elevation)
            .any(|v| !v.is_finite())
        || plates.boundary_motion.iter().any(|v| !v.is_finite())
    {
        return Err("Invalid sampled geodesic-belt composition inputs.".into());
    }
    if (0..n).any(|i| {
        old.elevation[i] != old.baseline[i] + old.convergence[i] + old.divergence[i] + old.detail[i]
    }) {
        return Err("Noninitial or inconsistent source terrain provenance.".into());
    }
    let width = reference.boundary_width_km * 1000.;
    let mut sources = Vec::new();
    let source_identity;
    if matches!(origin, Origin::FixedBoundaryArcs) {
        let fixed = structural_sources::build(reference)?;
        sources = fixed.sources;
        source_identity = Some(fixed.identity);
    } else {
        source_identity = None;
        let mut seeded = if matches!(origin, Origin::RegionSeeds) {
            Some(vec![[0_f64; 3]; n])
        } else {
            None
        };
        for segment in 0..m {
            let a = plates.boundary_cells[segment * 2] as usize;
            let b = plates.boundary_cells[segment * 2 + 1] as usize;
            if a >= n || b >= n || a == b {
                return Err("Invalid sampled boundary cells.".into());
            }
            let amplitudes = boundary_amplitudes(
                plates.boundary_motion[segment * 2],
                (crust.continentality[a] + crust.continentality[b]) * 0.5,
            );
            let start = plates.boundary_directions[segment * 2];
            let end = plates.boundary_directions[segment * 2 + 1];
            let arc = Source::new(start, end, amplitudes)?;
            if let Some(seeded) = seeded.as_mut() {
                let sum = std::array::from_fn::<_, 3, _>(|i| start[i] + end[i]);
                let norm = sum.iter().map(|v| v * v).sum::<f64>().sqrt();
                let p = sum.map(|v| v / norm);
                for cell in [a, b] {
                    let decay =
                        (-angle(s.centers[cell], p) * reference.radius_meters / width).exp();
                    for j in 0..3 {
                        seeded[cell][j] = seeded[cell][j].max(amplitudes[j] * decay);
                    }
                }
            } else {
                sources.push(arc);
            }
        }
        if let Some(seeded) = seeded {
            for (i, amplitudes) in seeded.into_iter().enumerate() {
                if amplitudes.iter().any(|&v| v > 0.) {
                    sources.push(Source::new(s.centers[i], s.centers[i], amplitudes)?);
                }
            }
        }
    }
    let field = Field::new(
        sources,
        Settings {
            radius_meters: reference.radius_meters,
            width_meters: width,
        },
    )?;
    let modulation = geography::Field::new(&reference.seed, reference.continental_scale)?;
    let mut terrain = old.clone();
    let mut count = 0usize;
    let mut maximum = 0;
    terrain.convergence = vec![0.; n];
    terrain.divergence = vec![0.; n];
    for (i, &p) in s.centers.iter().enumerate() {
        let sample = field.sample(p)?;
        count = count
            .checked_add(sample.evaluated_arcs)
            .ok_or("Geodesic evaluation count overflowed.")?;
        maximum = maximum.max(sample.evaluated_arcs);
        let roughness = modulation.relief(p, crust.continentality[i])?;
        terrain.convergence[i] =
            sample.values_meters[0] * reference.relief_scale * roughness.convergence_factor;
        terrain.divergence[i] = (sample.values_meters[1] - sample.values_meters[2])
            * reference.relief_scale
            * roughness.divergence_factor;
        terrain.elevation[i] = terrain.baseline[i]
            + terrain.convergence[i]
            + terrain.divergence[i]
            + terrain.detail[i];
        if !terrain.elevation[i].is_finite() {
            return Err("Geodesic initial terrain composition overflowed.".into());
        }
    }
    Ok(Initial {
        terrain,
        sources: field.sources().to_vec(),
        origin,
        mean_evaluated_sources: count as f64 / n as f64,
        maximum_evaluated_sources: maximum,
        source_identity,
    })
}
