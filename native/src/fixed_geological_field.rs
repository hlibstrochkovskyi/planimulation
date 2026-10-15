//! Target-independent initial bed sampling, not a supported World family.
//! V2 continental columns/detail with independently pinned geological origins.
use crate::{
    Recipe, Surface, continental_uplands, crust,
    geodesic_envelope::{self, Field as Envelope, Settings, Source},
    geography, structural_sources, terrain,
};
use serde::Serialize;
pub const MODEL_VERSION: &str = "fixed-geological-field-1";
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Sample {
    pub continentality: f64,
    pub baseline_meters: f64,
    pub convergence_meters: f64,
    pub divergence_meters: f64,
    pub detail_meters: f64,
    pub elevation_meters: f64,
}
pub struct Field {
    reference: Recipe,
    source_identity: structural_sources::Identity,
    envelope: Envelope,
    geography: geography::Field,
    threshold: f64,
}
impl Field {
    pub fn new(reference: Recipe) -> Result<Self, String> {
        let sources = structural_sources::build(&reference)?;
        // Threshold fitting already uses the frozen global reference quadrature;
        // the tiny output mesh below cannot reroll it for a local patch.
        let columns = continental_uplands::build_crust(
            &Surface::build(0, reference.radius_meters),
            &reference.seed,
            reference.continental_fraction,
            reference.continental_scale,
        )?;
        let envelope = Envelope::new(
            sources.sources,
            Settings {
                radius_meters: reference.radius_meters,
                width_meters: reference.boundary_width_km * 1000.,
            },
        )?;
        let geography = geography::Field::new(&reference.seed, reference.continental_scale)?;
        Ok(Self {
            reference,
            source_identity: sources.identity,
            envelope,
            geography,
            threshold: columns.threshold,
        })
    }
    pub fn reference_recipe(&self) -> &Recipe {
        &self.reference
    }
    pub fn source_identity(&self) -> &structural_sources::Identity {
        &self.source_identity
    }
    pub fn sources(&self) -> &[Source] {
        self.envelope.sources()
    }
    pub fn sample(&self, direction: [f64; 3]) -> Result<Sample, String> {
        let potential = self.geography.continental_potential(direction)?;
        let c = crust::continentality(potential, self.threshold);
        let thickness = continental_uplands::thickness(c, potential, self.threshold)?;
        let baseline = terrain::crust_baseline(thickness, 3000. - 200. * c);
        let modulation = self.geography.relief(direction, c)?;
        let envelope = self.envelope.sample(direction)?;
        let convergence =
            envelope.values_meters[0] * self.reference.relief_scale * modulation.convergence_factor;
        let divergence = (envelope.values_meters[1] - envelope.values_meters[2])
            * self.reference.relief_scale
            * modulation.divergence_factor;
        let detail = modulation.detail * self.reference.detail_amplitude_meters;
        Ok(Sample {
            continentality: c,
            baseline_meters: baseline,
            convergence_meters: convergence,
            divergence_meters: divergence,
            detail_meters: detail,
            elevation_meters: baseline + convergence + divergence + detail,
        })
    }
    pub fn envelope_model_version(&self) -> &'static str {
        geodesic_envelope::MODEL_VERSION
    }
}
