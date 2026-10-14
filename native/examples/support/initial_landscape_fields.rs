//! Direct initial-field composition for diagnostic sampling, not a dense World.
use planimulation_core::{
    Recipe, Surface, continental_uplands, crust::Crust, geography, tectonics::Tectonics,
    terrain::Terrain,
};

pub fn build(
    surface: &Surface,
    parameters: &Recipe,
) -> Result<(Crust, Tectonics, Terrain), String> {
    parameters.validate()?;
    if !matches!(
        parameters.model_version.as_str(),
        "continuous-plates-1" | continental_uplands::MODEL_VERSION
    ) || parameters.terrain_preparation_passes != Some(0)
    {
        return Err("Diagnostic composition requires a supported structural family and zero old preparation.".into());
    }
    let crust = if parameters.model_version == continental_uplands::MODEL_VERSION {
        continental_uplands::build_crust(
            surface,
            &parameters.seed,
            parameters.continental_fraction,
            parameters.continental_scale,
        )?
    } else {
        Crust::build(
            surface,
            &parameters.seed,
            parameters.continental_fraction,
            parameters.continental_scale,
        )
    };
    let tectonics = Tectonics::build_with_continuous_roots(
        surface,
        &parameters.seed,
        parameters.plate_count,
        parameters.max_plate_speed_cm_per_year,
        parameters.radius_meters,
    )?;
    let mut terrain = Terrain::build(surface, &crust, &tectonics, parameters);
    if parameters.model_version == continental_uplands::MODEL_VERSION {
        let field = geography::Field::new(&parameters.seed, parameters.continental_scale)?;
        for (i, &direction) in surface.centers.iter().enumerate() {
            let sample = field.relief(direction, crust.continentality[i])?;
            terrain.convergence[i] *= sample.convergence_factor;
            terrain.divergence[i] *= sample.divergence_factor;
            terrain.detail[i] = sample.detail * parameters.detail_amplitude_meters;
            terrain.elevation[i] = terrain.baseline[i]
                + terrain.convergence[i]
                + terrain.divergence[i]
                + terrain.detail[i];
        }
    }
    Ok((crust, tectonics, terrain))
}
