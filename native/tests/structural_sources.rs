use planimulation_core::{
    Recipe, Surface, continental_uplands,
    geodesic_envelope::{Field, Settings, Source},
    structural_sources::{self, REFERENCE_REGION_COUNT, REFERENCE_SUBDIVISION, Sources},
    tectonics::Tectonics,
    terrain::boundary_amplitudes,
    water::WaterSettings,
};
use serde_json::Value;

fn recipe() -> Recipe {
    serde_json::from_value(serde_json::json!({
        "schemaVersion":1,"modelVersion":"multiscale-geography-2",
        "randomVersion":"fnv1a-utf8-mulberry32-1","seed":"first-light",
        "subdivision":3,"radiusMeters":6371000.,"plateCount":12,
        "maxPlateSpeedCmPerYear":8.,"continentalFraction":0.38,
        "continentalScale":1.,"reliefScale":1.,"boundaryWidthKm":300.,
        "detailAmplitudeMeters":300.,"terrainPreparationPasses":0,
        "water":{"mode":"coverage","fraction":0.71}
    }))
    .unwrap()
}
fn settings(r: &Recipe) -> Settings {
    Settings {
        radius_meters: r.radius_meters,
        width_meters: r.boundary_width_km * 1000.,
    }
}
fn geometric_record(sources: &Sources) -> Value {
    let mut value = serde_json::to_value(&sources.sources).unwrap();
    for source in value.as_array_mut().unwrap() {
        source.as_object_mut().unwrap().remove("amplitudesMeters");
    }
    value
}

#[test]
fn target_resolution_detail_relief_and_water_do_not_change_sources_or_identity() {
    let original = recipe();
    let sources = structural_sources::build(&original).unwrap();
    let bytes = serde_json::to_vec(&sources).unwrap();
    let field = Field::new(sources.sources.clone(), settings(&original)).unwrap();
    let shared = Surface::build(0, original.radius_meters);
    for (level, detail, relief, water) in [
        (0, 0., 0., WaterSettings::Coverage { fraction: 0. }),
        (2, 1000., 2., WaterSettings::Coverage { fraction: 1. }),
        (
            6,
            73.,
            0.3,
            WaterSettings::Volume {
                volume_cubic_meters: 1e18,
            },
        ),
    ] {
        let mut r = original.clone();
        r.subdivision = level;
        r.detail_amplitude_meters = detail;
        r.relief_scale = relief;
        r.water = water;
        let other = structural_sources::build(&r).unwrap();
        assert_eq!(sources.identity, other.identity);
        assert_eq!(bytes, serde_json::to_vec(&other).unwrap());
        let other = Field::new(other.sources, settings(&r)).unwrap();
        for &p in &shared.centers {
            assert_eq!(field.sample(p).unwrap(), other.sample(p).unwrap());
        }
    }
    let identity = serde_json::to_value(&sources.identity).unwrap();
    for excluded in [
        "subdivision",
        "detailAmplitudeMeters",
        "reliefScale",
        "water",
        "boundaryWidthKm",
    ] {
        assert!(identity.get(excluded).is_none());
    }
}

#[test]
fn reference_construction_retains_weighted_partition_crust_and_raw_amplitude_laws() {
    let r = recipe();
    let actual = structural_sources::build(&r).unwrap();
    assert_eq!(actual.model_version, structural_sources::MODEL_VERSION);
    assert_eq!(actual.reference_subdivision, 5);
    assert_eq!(actual.reference_region_count, 10242);
    assert_eq!(actual.reference_region_count, REFERENCE_REGION_COUNT);
    let s = Surface::build(REFERENCE_SUBDIVISION, r.radius_meters);
    let crust =
        continental_uplands::build_crust(&s, &r.seed, r.continental_fraction, r.continental_scale)
            .unwrap();
    let plates = Tectonics::build_with_continuous_roots(
        &s,
        &r.seed,
        r.plate_count,
        r.max_plate_speed_cm_per_year,
        r.radius_meters,
    )
    .unwrap();
    let expected: Vec<_> = (0..plates.boundary_types.len())
        .map(|segment| {
            let a = plates.boundary_cells[segment * 2] as usize;
            let b = plates.boundary_cells[segment * 2 + 1] as usize;
            Source::new(
                plates.boundary_directions[segment * 2],
                plates.boundary_directions[segment * 2 + 1],
                boundary_amplitudes(
                    plates.boundary_motion[segment * 2],
                    (crust.continentality[a] + crust.continentality[b]) * 0.5,
                ),
            )
            .unwrap()
        })
        .collect();
    assert_eq!(
        serde_json::to_vec(&actual.sources).unwrap(),
        serde_json::to_vec(&expected).unwrap()
    );
}

#[test]
fn speed_changes_weights_not_arc_locations_and_stopped_sources_are_zero() {
    let mut r = recipe();
    let moving = structural_sources::build(&r).unwrap();
    r.max_plate_speed_cm_per_year = 16.;
    let faster = structural_sources::build(&r).unwrap();
    assert_eq!(geometric_record(&moving), geometric_record(&faster));
    assert_ne!(moving.identity, faster.identity);
    assert_ne!(
        serde_json::to_vec(&moving.sources).unwrap(),
        serde_json::to_vec(&faster.sources).unwrap()
    );
    r.max_plate_speed_cm_per_year = 0.;
    let stopped = structural_sources::build(&r).unwrap();
    assert_eq!(geometric_record(&moving), geometric_record(&stopped));
    let field = Field::new(stopped.sources, settings(&r)).unwrap();
    for &p in &Surface::build(1, r.radius_meters).centers {
        let sample = field.sample(p).unwrap();
        assert_eq!(sample.values_meters, [0.; 3]);
        assert_eq!(sample.winner_sources, [None; 3]);
        assert_eq!(sample.evaluated_arcs, 0);
    }
}

#[test]
fn seed_and_material_inputs_are_recorded_while_width_is_composition_only() {
    let original = recipe();
    let sources = structural_sources::build(&original).unwrap();
    let mut r = original.clone();
    r.boundary_width_km = 50.;
    let narrow = structural_sources::build(&r).unwrap();
    assert_eq!(sources.identity, narrow.identity);
    assert_eq!(
        serde_json::to_vec(&sources.sources).unwrap(),
        serde_json::to_vec(&narrow.sources).unwrap()
    );
    r = original.clone();
    r.continental_fraction = 0.;
    let oceanic = structural_sources::build(&r).unwrap();
    assert_ne!(sources.identity, oceanic.identity);
    assert_eq!(geometric_record(&sources), geometric_record(&oceanic));
    assert_ne!(
        serde_json::to_vec(&sources.sources).unwrap(),
        serde_json::to_vec(&oceanic.sources).unwrap()
    );
    r = original;
    r.seed = "independent-structure".into();
    let other = structural_sources::build(&r).unwrap();
    assert_ne!(sources.identity, other.identity);
    assert_ne!(geometric_record(&sources), geometric_record(&other));
}

#[test]
fn maximum_supported_plates_have_unit_arcs_and_exact_seam_pole_queries() {
    let mut r = recipe();
    r.plate_count = 32;
    let sources = structural_sources::build(&r).unwrap();
    assert!(!sources.sources.is_empty());
    assert!(sources.sources.len() <= planimulation_core::geodesic_envelope::MAX_SOURCES);
    for source in &sources.sources {
        let record = serde_json::to_value(source).unwrap();
        for key in ["start", "end", "midpoint"] {
            let norm: f64 = record[key]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_f64().unwrap().powi(2))
                .sum();
            assert!((norm - 1.).abs() <= 1e-12);
        }
        assert!(record["length"].as_f64().unwrap() > 0.);
    }
    let field = Field::new(sources.sources, settings(&r)).unwrap();
    for p in [
        [0., 1., 0.],
        [0., -1., 0.],
        [-1., 0., 0.],
        [-1., 0., 1e-12],
        [-1., 0., -1e-12],
    ] {
        let fast = field.sample(p).unwrap();
        let oracle = field.sample_brute_force(p).unwrap();
        assert_eq!(fast.values_meters, oracle.values_meters);
        assert_eq!(fast.winner_sources, oracle.winner_sources);
    }
}

#[test]
fn invalid_or_prepared_reference_recipes_refuse_without_widening_production_limits() {
    let original = recipe();
    for invalid in 0..6 {
        let mut r = original.clone();
        match invalid {
            0 => r.model_version = "multiscale-geography-1".into(),
            1 => r.terrain_preparation_passes = Some(1),
            2 => r.subdivision = 7,
            3 => r.plate_count = 33,
            4 => r.max_plate_speed_cm_per_year = f64::NAN,
            5 => r.water = WaterSettings::Coverage { fraction: -0.1 },
            _ => unreachable!(),
        }
        assert!(structural_sources::build(&r).is_err());
    }
    assert_eq!(original, recipe());
}
