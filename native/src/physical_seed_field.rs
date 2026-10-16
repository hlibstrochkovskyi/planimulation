//! Bounded continuous physical initial perturbations, not display displacement
//! or a claim to simulate erosion. Frozen four-band value-noise construction.
use crate::{geography, geography::Noise};
use serde::Serialize;
pub const MODEL_VERSION: &str = "physical-seed-spectrum-1";
/// Correlation lattice spacings, not sinusoidal wavelengths or grid spacing.
pub const BANDS: [(f64, f64); 4] = [(8000., 0.125), (4000., 0.25), (2000., 0.25), (1000., 0.375)];
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    pub radius_meters: f64,
    pub amplitude_meters: f64,
    pub length_scale: f64,
}
impl Settings {
    pub fn validate(self) -> Result<(), String> {
        if !self.radius_meters.is_finite()
            || !(100000. ..=20000000.).contains(&self.radius_meters)
            || !self.amplitude_meters.is_finite()
            || !(0. ..=1000.).contains(&self.amplitude_meters)
            || !self.length_scale.is_finite()
            || !(0.01..=1000.).contains(&self.length_scale)
        {
            return Err("Invalid physical seed-field settings.".into());
        }
        Ok(())
    }
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BandRecord {
    pub correlation_lattice_spacing_meters: f64,
    pub weight: f64,
    pub random_stream: String,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Identity {
    pub model_version: &'static str,
    pub random_version: &'static str,
    pub noise_dependency_model_version: &'static str,
    pub seed: String,
    pub settings: Settings,
    pub bands: Vec<BandRecord>,
}
pub struct Field {
    identity: Identity,
    bands: Vec<Noise>,
}
impl Field {
    pub fn new(seed: &str, settings: Settings) -> Result<Self, String> {
        settings.validate()?;
        if seed.trim().is_empty() || seed.encode_utf16().count() > 128 {
            return Err(
                "Physical seed field requires a nonblank seed of at most 128 UTF-16 units.".into(),
            );
        }
        let records: Vec<_> = BANDS
            .iter()
            .enumerate()
            .map(|(i, &(length, weight))| BandRecord {
                correlation_lattice_spacing_meters: length * settings.length_scale,
                weight,
                random_stream: format!("{MODEL_VERSION}.band-{i}"),
            })
            .collect();
        let bands = records
            .iter()
            .map(|b| Noise::new(seed, &b.random_stream))
            .collect();
        Ok(Self {
            identity: Identity {
                model_version: MODEL_VERSION,
                random_version: "fnv1a-utf8-mulberry32-1",
                noise_dependency_model_version: geography::MODEL_VERSION,
                seed: seed.into(),
                settings,
                bands: records,
            },
            bands,
        })
    }
    pub fn identity(&self) -> &Identity {
        &self.identity
    }
    pub fn sample(&self, p: [f64; 3]) -> Result<f64, String> {
        if p.iter().any(|v| !v.is_finite())
            || (p.iter().map(|v| v * v).sum::<f64>() - 1.).abs() > 1e-12
        {
            return Err("Physical seed field requires a finite unit direction.".into());
        }
        let settings = self.identity.settings;
        if settings.amplitude_meters == 0. {
            return Ok(0.);
        }
        let value = self
            .bands
            .iter()
            .zip(&self.identity.bands)
            .map(|(noise, b)| {
                b.weight
                    * noise.sample(
                        p,
                        settings.radius_meters / b.correlation_lattice_spacing_meters,
                    )
            })
            .sum::<f64>()
            * settings.amplitude_meters;
        // Convex interpolation and normalized positive band weights give the
        // analytic bound; observe roundoff rather than clipping physical data.
        if !value.is_finite() || value.abs() > settings.amplitude_meters * (1. + 1e-12) {
            return Err("Physical seed field left its finite amplitude envelope.".into());
        }
        Ok(value)
    }
}
