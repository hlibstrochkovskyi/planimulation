//! Experimental initial-condition detail in physical-length bands, not erosion
//! or reconstructed folding. No camera/renderer inputs affect this field.
use crate::{V, dot, geography::Noise};
use serde::Serialize;

pub const MODEL_VERSION: &str = "physical-relief-spectrum-1";
/// Rotated value-noise lattice correlation lengths, NOT exact wavelengths or a
/// spectral band limit. Fixed positive weights sum to one before suppression.
pub const BANDS: [(f64, f64); 5] = [
    (800_000., 0.25),
    (400_000., 0.30),
    (200_000., 0.25),
    (100_000., 0.15),
    (50_000., 0.05),
];

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    pub radius_meters: f64,
    /// Owned physical-grid footprint proxy; never camera zoom or display LOD.
    pub sampling_length_meters: f64,
    pub amplitude_meters: f64,
}
#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BandRecord {
    pub correlation_length_meters: f64,
    pub base_weight: f64,
    pub footprint_retention: f64,
    pub angular_frequency: f64,
}
pub struct Field {
    settings: Settings,
    noise: Vec<Noise>,
    records: Vec<BandRecord>,
}
#[derive(Clone, Copy, Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Sample {
    pub mountain_strength: f64,
    pub signed_detail_meters: f64,
}

/// Smoothly suppress correlation lengths below 2–4 physical footprint widths.
/// This is an explicit heuristic footprint filter, not perfect anti-aliasing of
/// a nonlinear ridge transform or an exact finite-volume average.
pub fn footprint_retention(length: f64, footprint: f64) -> Result<f64, String> {
    if !length.is_finite() || length <= 0. || !footprint.is_finite() || footprint <= 0. {
        return Err("Invalid physical detail correlation length/footprint.".into());
    }
    let ratio = length / footprint;
    // Finite positive operands can give an infinite ratio; it denotes a fully
    // resolved band and does not enter lattice coordinates.
    let x = ((ratio - 2.) * 0.5).clamp(0., 1.);
    Ok(x * x * (3. - 2. * x))
}
impl Field {
    pub fn new(seed: &str, settings: Settings) -> Result<Self, String> {
        if seed.trim().is_empty()
            || seed.encode_utf16().count() > 128
            || !settings.radius_meters.is_finite()
            || !(100_000. ..=20_000_000.).contains(&settings.radius_meters)
            || !settings.sampling_length_meters.is_finite()
            || settings.sampling_length_meters <= 0.
            || !settings.amplitude_meters.is_finite()
            || !(0. ..=1000.).contains(&settings.amplitude_meters)
        {
            return Err("Invalid physical relief field settings.".into());
        }
        let records = BANDS
            .iter()
            .map(|&(length, weight)| {
                Ok(BandRecord {
                    correlation_length_meters: length,
                    base_weight: weight,
                    footprint_retention: footprint_retention(
                        length,
                        settings.sampling_length_meters,
                    )?,
                    angular_frequency: settings.radius_meters / length,
                })
            })
            .collect::<Result<Vec<_>, String>>()?;
        Ok(Self {
            settings,
            noise: (0..BANDS.len())
                .map(|i| Noise::new(seed, &format!("{MODEL_VERSION}.detail.{i}")))
                .collect(),
            records,
        })
    }
    pub fn bands(&self) -> &[BandRecord] {
        &self.records
    }
    pub fn sample(
        &self,
        p: V,
        continentality: f64,
        convergence_meters: f64,
    ) -> Result<Sample, String> {
        if p.iter().any(|v| !v.is_finite())
            || (dot(p, p) - 1.).abs() > 1e-12
            || !continentality.is_finite()
            || !(0. ..=1.).contains(&continentality)
            || !convergence_meters.is_finite()
            || convergence_meters < 0.
        {
            return Err("Invalid physical relief direction or structural inputs.".into());
        }
        let mountain_strength =
            continentality * (convergence_meters / (1000. + convergence_meters));
        let mut detail = 0.;
        for (noise, band) in self.noise.iter().zip(&self.records) {
            if band.footprint_retention == 0. {
                continue;
            }
            let n = noise.sample(p, band.angular_frequency);
            let ridge = 2. * (1. - n.abs()).powi(2) - 1.;
            // Signed coarse structure persists; ridged shape grows only with
            // supplied continental convergence. No plate strength is invented.
            let shaped = (1. - 0.5 * mountain_strength) * n + 0.5 * mountain_strength * ridge;
            detail += band.base_weight * band.footprint_retention * shaped;
        }
        // Suppressed weights are NOT renormalized into larger coarse relief.
        let signed_detail_meters =
            self.settings.amplitude_meters * (0.3 + 0.7 * continentality) * detail;
        if !signed_detail_meters.is_finite()
            || signed_detail_meters.abs() > self.settings.amplitude_meters * (1. + 1e-12)
        {
            return Err("Physical detail exceeded its declared meter bound.".into());
        }
        Ok(Sample {
            mountain_strength,
            signed_detail_meters,
        })
    }
}
