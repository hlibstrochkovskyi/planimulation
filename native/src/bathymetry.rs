//! `bathymetry-1`: ocean-floor contribution for `multiscale-geography-5`.
//! Placeholder hook: returns zero until the bathymetry model lands.
use crate::{Recipe, Surface, crust::Crust, tectonics::Tectonics};

pub const MODEL_VERSION: &str = "bathymetry-1";

/// Per-region ocean-floor contribution in meters, added to the bed before
/// preparation and the initial water fit.
pub fn seafloor(
    surface: &Surface,
    _crust: &Crust,
    _tectonics: &Tectonics,
    _recipe: &Recipe,
    elevation: &[f64],
) -> Result<Vec<f64>, String> {
    if elevation.len() != surface.centers.len() {
        return Err("Invalid bathymetry input.".into());
    }
    Ok(vec![0.; elevation.len()])
}
