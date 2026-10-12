//! Standalone moving wet/dry verification; no seasonal or rendering selection.
mod element;
mod face_controls;
mod faces;
mod moment_controls;
mod moments;
mod native;
mod paired_evolution;
mod reconstruction;
mod reference;
use serde_json::{Value, json};

const RADIUS: f64 = 1000.;
const CAP: f64 = 1000.;
const END_SECONDS: f64 = 300.;
const AXES: [[f64; 3]; 2] = [[1., 0., 0.], [1., 2., 3.]];
const THRESHOLDS: [f64; 3] = [0.001, 0.01, 0.05];

fn reference_front(depth: &[f64], threshold: f64) -> Result<f64, String> {
    if depth.windows(2).any(|h| h[0] > h[1] + 1e-12) {
        return Err("Dry reference contour requires a monotone profile.".into());
    }
    let at = depth
        .windows(2)
        .position(|h| h[0] < threshold && h[1] >= threshold)
        .ok_or("Missing interior dry reference contour.")?;
    let width = 2. / depth.len() as f64;
    Ok(-1. + (at as f64 + 0.5 + (threshold - depth[at]) / (depth[at + 1] - depth[at])) * width)
}

/// Reference controls only; not yet a native front accuracy qualification.
pub fn reference_report() -> Result<Value, String> {
    let mut planar = Vec::new();
    for (bands, steps) in [
        (64, 1600),
        (128, 1600),
        (256, 1600),
        (512, 1600),
        (1024, 400),
        (1024, 800),
        (1024, 1600),
        (1024, 3200),
    ] {
        let r = reference::planar(bands, steps)?;
        let width = 200. / bands as f64;
        let exact: Vec<_> = (0..bands)
            .map(|i| {
                let a = -100. + i as f64 * width;
                reference::wave_average(a, a + width, 100.)
            })
            .collect();
        let rms = (r
            .depth
            .iter()
            .zip(&exact)
            .map(|(a, b)| (a - b).powi(2))
            .sum::<f64>()
            / bands as f64)
            .sqrt();
        planar.push(json!({"bands":bands,"steps":steps,"stepSeconds":100./steps as f64,"endSeconds":100.,"rmsBandAverageErrorMeters":rms,
            "finalBandDepthMeters":r.depth,"exactBandDepthMeters":exact,"diagnostics":r.diagnostics}));
    }
    let mut spherical = Vec::new();
    for (bands, steps) in [
        (256, 600),
        (512, 300),
        (512, 600),
        (512, 1200),
        (1024, 1200),
        (2048, 1200),
        (4096, 1200),
        (4096, 2400),
    ] {
        let r = reference::spherical(bands, steps)?;
        spherical.push(json!({"bands":bands,"steps":steps,"stepSeconds":END_SECONDS/steps as f64,"finalBandDepthMeters":r.depth,"diagnostics":r.diagnostics}));
        let contours = THRESHOLDS
            .map(|threshold| {
                Ok(json!({"thresholdMeters":threshold,
            "p":reference_front(&r.depth, threshold)?}))
            })
            .into_iter()
            .collect::<Result<Vec<_>, String>>()?;
        spherical.last_mut().unwrap()["contours"] = json!(contours);
    }
    Ok(
        json!({"reportVersion":"surface-dry-reference-report-1","radiusMeters":RADIUS,
        "capSquareMetersPerSecond":CAP,"endSeconds":END_SECONDS,"roughness":reference::ROUGHNESS,
        "planarCertificateParameters":{"domainMeters":[-100.,100.],"speedMetersPerSecond":0.1,
            "capSquareMetersPerSecond":100.,"endSeconds":100.,"exactFrontMeters":10.,
            "exactDepth":"[(7/3)*(c*n)^2*(c*t-x)_+]^(3/7)",
            "boundary":"Exact time-integrated left inflow, zero right flux; no volume forcing or state fitting"},
        "sphericalInitialization":"Exact band averages of p_+^(3/7); native initial point stocks are not mass rescaled",
        "referenceMethod":"Centered-face depth, backward Euler, damped Picard, rebuilt nonlinear residual, no dry film or clipping",
        "selectedSphericalControl":7,"nativeReferenceSpatialControls":[5,6],"nativeReferenceTemporalControls":[6,7],
        "scope":"Reference-only planar exact traveling-wave certificate and independent spherical dry-cap controls; not a native accuracy pass",
        "planar":planar,"spherical":spherical}),
    )
}

/// Fixed dry-cap problem; no existing simulation or persistence selector.
pub fn report() -> Result<Value, String> {
    native::report(reference_report()?)
}

/// Flat-only interface-depth ablation with the unchanged independent reference.
pub fn centered_report() -> Result<Value, String> {
    native::centered_report(reference_report()?)
}

/// Gradient-side assembly comparison; the edge depth and reference are retained.
pub fn incident_report() -> Result<Value, String> {
    native::incident_report(reference_report()?)
}

/// Fixed dry witness with bounded coefficient quadrature refinement.
/// This report does not select or qualify a seasonal model.
pub fn element_report() -> Result<Value, String> {
    element::report(reference_report()?)
}

/// Frozen interval reconstruction controls; no time evolution or model selection.
pub fn reconstruction_report() -> Result<Value, String> {
    reconstruction::report()
}

/// Frozen all-cell moment reconstruction and face diagnostics, not evolution.
pub fn moment_reconstruction_report() -> Result<Value, String> {
    moment_controls::report()
}

/// Frozen shared-face derivatives and single-step accounting, not evolution.
pub fn shared_face_report() -> Result<Value, String> {
    face_controls::report()
}

/// Bounded consecutive planar transfers; no native/seasonal model selection.
pub fn paired_evolution_report() -> Result<Value, String> {
    paired_evolution::report()
}
