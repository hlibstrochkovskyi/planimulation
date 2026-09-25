use planimulation_core::{
    Random,
    reservoir::Column,
    reservoir_pair::{Checkpoint, Geometry, ReservoirPair, Storage, Volumes},
};

fn volumes(left: f64, right: f64) -> Volumes {
    Volumes { left, right }
}
fn column(bed_meters: f64, area_square_meters: f64) -> Column {
    Column {
        bed_meters,
        area_square_meters,
    }
}
fn geometry() -> Geometry {
    Geometry {
        left: vec![column(0., 2.), column(1., 3.)],
        right: vec![column(-1., 4.)],
        connection: vec![column(3., 1.), column(5., 2.)],
        sill_meters: 3.,
    }
}
fn pair() -> ReservoirPair {
    ReservoirPair::new(geometry(), volumes(0., 0.)).unwrap()
}
fn near(a: f64, b: f64) {
    assert!((a - b).abs() <= 1e-9_f64.max(b.abs() * 1e-12), "{a} != {b}");
}

fn direct(columns: &[Column], level: Option<f64>) -> f64 {
    level.map_or(0., |h| {
        columns
            .iter()
            .map(|c| c.area_square_meters * (h - c.bed_meters).max(0.))
            .sum()
    })
}
fn audit(p: &ReservoirPair) {
    let c = p.checkpoint();
    let s = p.snapshot().unwrap();
    let mut sum = direct(&c.geometry.left, s.levels_meters[0])
        + direct(&c.geometry.right, s.levels_meters[1]);
    if s.phase != "separate" {
        sum += direct(&c.geometry.connection, s.levels_meters[0]);
    }
    assert!((sum - s.total_stored_cubic_meters).abs() <= 1e-9_f64.max(sum * 1e-10));
    near(
        s.total_stored_cubic_meters,
        c.inventory.initial.left
            + c.inventory.initial.right
            + c.inventory.input.left
            + c.inventory.input.right,
    );
}

#[test]
fn fill_spill_threshold_and_merge_include_connection_storage() {
    let mut p = pair();
    assert_eq!(p.capacities_cubic_meters(), [12., 16.]);
    assert_eq!(p.snapshot().unwrap().levels_meters, [None, None]);
    for (input, phase, levels, transfer, shared, stored) in [
        (12., "separate", [Some(3.), None], 0., 0., 12.),
        (4., "separate", [Some(3.), Some(0.)], 4., 0., 16.),
        (12., "atSill", [Some(3.), Some(3.)], 12., 0., 28.),
        (10., "merged", [Some(4.), Some(4.)], 0., 10., 38.),
        (22., "merged", [Some(6.), Some(6.)], 0., 22., 60.),
        (0., "merged", [Some(6.), Some(6.)], 0., 0., 60.),
    ] {
        let step = p.add_input(volumes(input, 0.)).unwrap();
        assert_eq!(step.snapshot.phase, phase);
        assert_eq!(step.snapshot.levels_meters, levels);
        assert_eq!(step.left_to_right_cubic_meters, transfer);
        assert_eq!(step.right_to_left_cubic_meters, 0.);
        assert_eq!(step.input_to_common_storage_cubic_meters, shared);
        assert_eq!(step.snapshot.total_stored_cubic_meters, stored);
        audit(&p);
    }
}

#[test]
fn unequal_initial_stocks_and_reversed_spill_do_not_equalize_early() {
    let mut p = ReservoirPair::new(geometry(), volumes(7., 4.)).unwrap();
    assert_eq!(p.snapshot().unwrap().levels_meters, [Some(2.), Some(0.)]);
    let step = p.add_input(volumes(9., 0.)).unwrap();
    assert_eq!(step.left_to_right_cubic_meters, 4.);
    assert_eq!(step.snapshot.levels_meters, [Some(3.), Some(1.)]);
    audit(&p);
    let mut p = pair();
    let step = p.add_input(volumes(0., 18.)).unwrap();
    assert_eq!(step.right_to_left_cubic_meters, 2.);
    assert_eq!(step.snapshot.levels_meters, [Some(1.), Some(3.)]);
    audit(&p);
    assert_eq!(
        ReservoirPair::new(geometry(), volumes(12., 16.))
            .unwrap()
            .snapshot()
            .unwrap()
            .phase,
        "atSill"
    );
}

#[test]
fn simultaneous_input_and_one_large_pulse_have_conservative_event_boundaries() {
    let mut p = pair();
    let step = p.add_input(volumes(20., 18.)).unwrap();
    assert_eq!(step.left_to_right_cubic_meters, 0.);
    assert_eq!(step.right_to_left_cubic_meters, 0.);
    assert_eq!(step.input_to_common_storage_cubic_meters, 10.);
    assert_eq!(step.snapshot.levels_meters, [Some(4.); 2]);
    audit(&p);
    let mut other = pair();
    let step = other.add_input(volumes(38., 0.)).unwrap();
    assert_eq!(step.left_to_right_cubic_meters, 16.);
    assert_eq!(step.input_to_common_storage_cubic_meters, 10.);
    assert_eq!(step.snapshot, p.snapshot().unwrap());
    let mut exact = pair();
    assert_eq!(
        exact.add_input(volumes(12., 16.)).unwrap().snapshot.phase,
        "atSill"
    );
}

#[test]
fn swapping_sides_and_partitioning_pulses_preserve_results() {
    let mut a = pair();
    let mut g = geometry();
    std::mem::swap(&mut g.left, &mut g.right);
    let mut b = ReservoirPair::new(g, volumes(0., 0.)).unwrap();
    for (l, r) in [(1., 2.), (15., 0.), (0., 3.), (10., 9.)] {
        let x = a.add_input(volumes(l, r)).unwrap();
        let y = b.add_input(volumes(r, l)).unwrap();
        assert_eq!(
            x.snapshot.total_stored_cubic_meters,
            y.snapshot.total_stored_cubic_meters
        );
        assert_eq!(
            x.snapshot.levels_meters,
            [y.snapshot.levels_meters[1], y.snapshot.levels_meters[0]]
        );
        assert_eq!(x.left_to_right_cubic_meters, y.right_to_left_cubic_meters);
        audit(&a);
        audit(&b);
    }
    let mut one = pair();
    one.add_input(volumes(40., 20.)).unwrap();
    let mut many = pair();
    for _ in 0..80 {
        many.add_input(volumes(0.5, 0.25)).unwrap();
        audit(&many);
    }
    assert_eq!(one.snapshot().unwrap(), many.snapshot().unwrap());
}

#[test]
fn checkpoints_resume_exactly_before_at_and_after_connection() {
    let mut p = pair();
    for input in [
        volumes(7., 0.),
        volumes(9., 0.),
        volumes(12., 0.),
        volumes(10., 0.),
    ] {
        p.add_input(input).unwrap();
        let bytes = serde_json::to_vec(&p.checkpoint()).unwrap();
        let mut restored = ReservoirPair::restore(serde_json::from_slice(&bytes).unwrap()).unwrap();
        assert_eq!(p, restored);
        let mut uninterrupted = p.clone();
        for input in [volumes(0.13, 0.27), volumes(1.71, 0.99), volumes(100., 0.)] {
            assert_eq!(
                uninterrupted.add_input(input.clone()).unwrap(),
                restored.add_input(input).unwrap()
            );
        }
        assert_eq!(uninterrupted, restored);
    }
}

#[test]
fn malformed_geometry_inventory_versions_and_states_are_rejected() {
    for initial in [
        volumes(-1., 0.),
        volumes(f64::NAN, 0.),
        volumes(13., 0.),
        volumes(0., 17.),
    ] {
        assert!(ReservoirPair::new(geometry(), initial).is_err());
    }
    for kind in 0..6 {
        let mut g = geometry();
        match kind {
            0 => g.left.clear(),
            1 => g.connection.clear(),
            2 => g.connection[0].bed_meters = 2.,
            3 => g.left[0].area_square_meters = 0.,
            4 => g.sill_meters = f64::NAN,
            _ => g.right[0].bed_meters = 3.,
        }
        assert!(ReservoirPair::new(g, volumes(0., 0.)).is_err());
    }
    let good = pair().checkpoint();
    let mut bad = good.clone();
    bad.experiment_version = "future".into();
    assert!(ReservoirPair::restore(bad).is_err());
    let mut bad = good.clone();
    bad.inventory.storage = Storage::Connected { volume: 1. };
    assert!(ReservoirPair::restore(bad).is_err());
    let mut bad = good.clone();
    bad.inventory.input.left = 1.;
    assert!(ReservoirPair::restore(bad).is_err());
    let mut bad = good.clone();
    bad.inventory.pulse_count = 1;
    bad.inventory.storage = Storage::Separate(volumes(1., 0.));
    assert!(ReservoirPair::restore(bad).is_err());
    let mut p = pair();
    p.add_input(volumes(12., 16.)).unwrap();
    let mut bad = p.checkpoint();
    bad.inventory.storage = Storage::Separate(volumes(12., 16.));
    assert!(ReservoirPair::restore(bad).is_err());
    let mut json = serde_json::to_value(good).unwrap();
    json["unexpected"] = true.into();
    assert!(serde_json::from_slice::<Checkpoint>(&serde_json::to_vec(&json).unwrap()).is_err());
}

#[test]
fn failures_leave_all_state_unchanged_including_precision_and_counter_errors() {
    let mut p = pair();
    p.add_input(volumes(38., 0.)).unwrap();
    for input in [
        volumes(-1., 0.),
        volumes(0., f64::INFINITY),
        volumes(f64::NAN, 0.),
        volumes(f64::MAX, f64::MAX),
        volumes(1e-100, 0.),
    ] {
        let before = p.clone();
        assert!(p.add_input(input).is_err());
        assert_eq!(p, before);
    }
    let mut c = p.checkpoint();
    c.inventory.pulse_count = u64::MAX;
    let mut p = ReservoirPair::restore(c).unwrap();
    let before = p.clone();
    assert!(p.add_input(volumes(0., 0.)).is_err());
    assert_eq!(p, before);
    let mut g = geometry();
    for c in g
        .left
        .iter_mut()
        .chain(&mut g.right)
        .chain(&mut g.connection)
    {
        c.bed_meters += 1e12;
    }
    g.sill_meters += 1e12;
    let mut p = ReservoirPair::new(g, volumes(0., 0.)).unwrap();
    let before = p.clone();
    assert!(p.add_input(volumes(0.000001, 0.)).is_err());
    assert_eq!(p, before);
}

#[test]
fn seeded_pulses_and_geometry_scaling_match_independent_prism_sums() {
    for seed in 0..30 {
        let mut rng = Random::stream(&format!("pair-{seed}"), "pulses");
        let mut p = pair();
        let mut g = geometry();
        for c in g
            .left
            .iter_mut()
            .chain(&mut g.right)
            .chain(&mut g.connection)
        {
            c.area_square_meters *= 4.;
            c.bed_meters += 100.;
        }
        g.sill_meters += 100.;
        let mut scaled = ReservoirPair::new(g, volumes(0., 0.)).unwrap();
        for _ in 0..100 {
            let l = f64::from(rng.next_u32() % 20) * 0.25;
            let r = f64::from(rng.next_u32() % 20) * 0.25;
            let a = p.add_input(volumes(l, r)).unwrap();
            let b = scaled.add_input(volumes(l * 4., r * 4.)).unwrap();
            near(
                a.snapshot.total_stored_cubic_meters * 4.,
                b.snapshot.total_stored_cubic_meters,
            );
            for (x, y) in a
                .snapshot
                .levels_meters
                .into_iter()
                .zip(b.snapshot.levels_meters)
            {
                match (x, y) {
                    (Some(x), Some(y)) => near(x + 100., y),
                    (None, None) => {}
                    _ => panic!("Different wet masks"),
                }
            }
            audit(&p);
            audit(&scaled);
        }
    }
}

#[test]
fn physical_chain_matches_basin_analysis_and_counts_the_dry_sill_only_after_merge() {
    use planimulation_core::{Surface, basins::Basins};
    // 0 -- 1 -- sill -- right minimum -- upper bank.
    let surface = Surface {
        centers: vec![[1., 0., 0.]; 5],
        faces: vec![],
        offsets: vec![0, 1, 3, 5, 7, 8],
        neighbors: vec![1, 0, 2, 1, 3, 2, 4, 3],
        distances: vec![1.; 8],
        areas: vec![2., 3., 1., 4., 2.],
        boundary_offsets: vec![],
        boundaries: vec![],
    };
    let heights = [0., 1., 3., -1., 5.];
    let basins = Basins::build(&surface, &heights).unwrap();
    let before = basins.clone();
    let mut p = pair();
    for (side, region) in [0, 3].into_iter().enumerate() {
        near(
            p.capacities_cubic_meters()[side],
            basins.nodes()[basins.region_nodes()[region]]
                .capacity_cubic_meters
                .unwrap(),
        );
    }
    let at = p.add_input(volumes(28., 0.)).unwrap().snapshot;
    assert_eq!(at.phase, "atSill");
    assert_eq!(direct(&geometry().connection, at.levels_meters[0]), 0.);
    near(
        at.total_stored_cubic_meters,
        basins.volume_at_level(basins.root(), 3.).unwrap(),
    );
    let merged = p.add_input(volumes(32., 0.)).unwrap().snapshot;
    assert_eq!(direct(&geometry().connection, merged.levels_meters[0]), 5.);
    near(
        merged.total_stored_cubic_meters,
        basins.volume_at_level(basins.root(), 6.).unwrap(),
    );
    assert_eq!(basins, before);
}

#[test]
fn varied_weighted_bowls_plateaus_and_initial_stocks_conserve_water() {
    for seed in 0..60 {
        let mut rng = Random::stream(&format!("pair-geometry-{seed}"), "geometry");
        let mut area = || 1. + f64::from(rng.next_u32() % 20);
        let sill = 3. + f64::from(seed % 5);
        let g = Geometry {
            left: vec![column(-2., area()), column(-2., area()), column(1., area())],
            right: vec![column(-3., area()), column(0., area())],
            connection: vec![
                column(sill, area()),
                column(sill, area()),
                column(sill + 2., area()),
            ],
            sill_meters: sill,
        };
        let empty = ReservoirPair::new(g.clone(), volumes(0., 0.)).unwrap();
        let [l, r] = empty.capacities_cubic_meters();
        let mut p = ReservoirPair::new(g, volumes(l * 0.25, r * 0.5)).unwrap();
        for _ in 0..100 {
            let input = volumes(
                f64::from(rng.next_u32() % 40) * 0.25,
                f64::from(rng.next_u32() % 40) * 0.25,
            );
            let result = p.add_input(input).unwrap();
            assert!(
                result.left_to_right_cubic_meters >= 0. && result.right_to_left_cubic_meters >= 0.
            );
            audit(&p);
        }
        assert_eq!(p.snapshot().unwrap().phase, "merged");
        let json = serde_json::to_vec(&p.checkpoint()).unwrap();
        assert_eq!(
            p,
            ReservoirPair::restore(serde_json::from_slice(&json).unwrap()).unwrap()
        );
    }
}
