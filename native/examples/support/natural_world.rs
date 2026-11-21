use planimulation_core::{Recipe, water::WaterSettings};

pub fn recipe(seed: &str, subdivision: u32) -> Recipe {
    Recipe {
        schema_version: 1,
        model_version: "multiscale-geography-4".into(),
        random_version: "fnv1a-utf8-mulberry32-1".into(),
        seed: seed.into(),
        subdivision,
        radius_meters: 6_371_000.,
        plate_count: 12,
        max_plate_speed_cm_per_year: 8.,
        continental_fraction: 0.38,
        continental_scale: 1.,
        relief_scale: 1.,
        boundary_width_km: 300.,
        detail_amplitude_meters: 300.,
        terrain_preparation_passes: Some(4),
        water: WaterSettings::Coverage { fraction: 0.71 },
    }
}
