use planimulation_core::{
    Surface,
    drainage::Drainage,
    runoff_transport::{Network, Settings, Transfers},
};

fn close(a: f64, b: f64) {
    assert!((a - b).abs() < 1e-11, "{a} != {b}");
}
fn network(receivers: &[u32]) -> Network {
    let lengths: Vec<_> = receivers
        .iter()
        .enumerate()
        .map(|(i, &r)| if r as usize == i { 0. } else { 3600. })
        .collect();
    Network::from_receivers(receivers, &lengths, Settings::default()).unwrap()
}

#[test]
fn a_departure_only_reaches_its_neighbor_and_never_reuses_same_step_arrivals() {
    let net = network(&[1, 2, 2]);
    let step = net.advance(&[100., 0., 0.], 600.).unwrap();
    let released = 100. * (1. - (-1_f64 / 6.).exp());
    close(step.sent_kilograms[0], released);
    assert_eq!(step.sent_kilograms[1], 0.);
    assert_eq!(step.transit_kilograms[1], step.sent_kilograms[0]);
    assert_eq!(step.terminal_delivery_kilograms, vec![0.; 3]);
    let second = net.advance(&step.transit_kilograms, 600.).unwrap();
    assert!(second.terminal_delivery_kilograms[2] > 0.);
    close(
        second.transit_kilograms.iter().sum::<f64>()
            + second.terminal_delivery_kilograms.iter().sum::<f64>(),
        100.,
    );
}

#[test]
fn one_reach_has_exact_exponential_decay_and_terminal_input_is_retained_not_discarded() {
    let net = network(&[1, 1]);
    let mut stock = vec![100., 0.];
    let mut delivered = 0.;
    for _ in 0..12 {
        let step = net.advance(&stock, 600.).unwrap();
        stock = step.transit_kilograms;
        delivered += step.terminal_delivery_kilograms[1];
    }
    close(stock[0], 100. * (-2_f64).exp());
    close(stock[0] + delivered, 100.);
    let local = net.advance(&[0., 7.], 600.).unwrap();
    assert_eq!(local.terminal_delivery_kilograms, [0., 7.]);
    assert_eq!(local.transit_kilograms, [0., 0.]);
}

#[test]
fn branched_inputs_have_one_physical_recipient_and_permutation_preserves_ownership() {
    let net = network(&[2, 2, 3, 3]);
    let step = net.advance(&[20., 30., 10., 5.], 600.).unwrap();
    assert_eq!(
        step.received_transit_kilograms[2],
        step.sent_kilograms[0] + step.sent_kilograms[1]
    );
    assert_eq!(
        step.terminal_delivery_kilograms[3],
        step.sent_kilograms[2] + 5.
    );
    close(
        step.transit_kilograms.iter().sum::<f64>()
            + step.terminal_delivery_kilograms.iter().sum::<f64>(),
        65.,
    );
    // Physical old 0,1,2,3 become new 3,2,0,1.
    let remapped = network(&[1, 1, 0, 0])
        .advance(&[10., 5., 30., 20.], 600.)
        .unwrap();
    for (old, new) in [3, 2, 0, 1].into_iter().enumerate() {
        close(step.transit_kilograms[old], remapped.transit_kilograms[new]);
        close(
            step.terminal_delivery_kilograms[old],
            remapped.terminal_delivery_kilograms[new],
        );
    }
}

#[test]
fn time_refinement_converges_to_the_independent_two_reach_cascade_solution() {
    let net = network(&[1, 2, 2]);
    let t = 2_f64;
    let exact_middle = 100. * t * (-t).exp();
    let exact_terminal = 100. * (1. - (1. + t) * (-t).exp());
    let mut errors = Vec::new();
    for dt in [600, 300, 150, 75] {
        let mut stock = vec![100., 0., 0.];
        let mut terminal = 0.;
        for _ in 0..7200 / dt {
            let step = net.advance(&stock, dt as f64).unwrap();
            stock = step.transit_kilograms;
            terminal += step.terminal_delivery_kilograms[2];
        }
        close(stock[0], 100. * (-t).exp());
        errors.push((stock[1] - exact_middle).abs() + (terminal - exact_terminal).abs());
        close(stock.iter().sum::<f64>() + terminal, 100.);
    }
    assert!(errors.windows(2).all(|p| p[1] < p[0]), "{errors:?}");
}

#[test]
fn physical_distance_and_speed_define_response_without_label_dependent_lengths() {
    let settings = Settings::default();
    let net = Network::from_receivers(&[1, 1], &[7200., 0.], settings).unwrap();
    assert_eq!(net.response_seconds(), [7200., 0.]);
    let slow = net.advance(&[100., 0.], 600.).unwrap();
    let twice = Network::from_receivers(
        &[1, 1],
        &[7200., 0.],
        Settings {
            effective_speed_meters_per_second: 2.,
            ..settings
        },
    )
    .unwrap()
    .advance(&[100., 0.], 600.)
    .unwrap();
    assert!(twice.terminal_delivery_kilograms[1] > slow.terminal_delivery_kilograms[1]);
    let tiny = Network::from_receivers(&[1, 1], &[1., 0.], settings).unwrap();
    assert_eq!(tiny.response_seconds(), [3600., 0.]);
    assert_eq!(tiny.maximum_coupled_step_seconds(), 1200);
}

#[test]
fn a_water_body_keeps_first_contact_locations_instead_of_teleporting_to_its_canonical_id() {
    let s = Surface {
        centers: vec![[1., 0., 0.]; 5],
        faces: vec![],
        offsets: vec![0, 1, 3, 5, 7, 8],
        neighbors: vec![1, 0, 2, 1, 3, 2, 4, 3],
        distances: vec![3600.; 8],
        areas: vec![1.; 5],
        boundary_offsets: vec![],
        boundaries: vec![],
    };
    let d = Drainage::build(&s, &[3., 2., -1., -2., 4.], &[0, 0, 1, 1, 0]);
    let net = Network::from_surface(&s, &d, Settings::default()).unwrap();
    assert_eq!(d.outlets, [2; 5]);
    let step = net.advance(&[0., 0., 0., 0., 100.], 600.).unwrap();
    assert!(step.terminal_delivery_kilograms[3] > 0.);
    assert_eq!(step.terminal_delivery_kilograms[2], 0.);
}

#[test]
fn malformed_graphs_bad_units_intervals_and_overflow_reject_atomically() {
    for (r, l) in [
        (vec![], vec![]),
        (vec![1, 0], vec![1., 1.]),
        (vec![2, 1], vec![1., 0.]),
        (vec![0], vec![1.]),
        (vec![1, 1], vec![0., 0.]),
        (vec![1, 1], vec![f64::NAN, 0.]),
    ] {
        assert!(Network::from_receivers(&r, &l, Settings::default()).is_err());
    }
    let net = network(&[1, 1]);
    let original = vec![1., 0.];
    for dt in [0., 601., f64::NAN] {
        assert!(net.advance(&original, dt).is_err());
    }
    for stock in [vec![-1., 0.], vec![1.], vec![f64::MAX, f64::MAX]] {
        assert!(net.advance(&stock, 600.).is_err());
    }
    assert_eq!(original, [1., 0.]);
    assert!(
        Network::from_receivers(
            &[1, 1],
            &[1., 0.],
            Settings {
                effective_speed_meters_per_second: 0.,
                ..Default::default()
            }
        )
        .is_err()
    );
}

#[test]
fn cumulative_routing_records_keep_small_additions_and_replay_with_signed_roundoff() {
    let mut ledger = Transfers::from_values([2_f64.powi(53); 4]);
    let mut correction = [0.; 4];
    ledger.accumulate(Transfers::from_values([1.; 4]), &mut correction);
    let saved = serde_json::to_vec(&(ledger, correction)).unwrap();
    let (mut restored, mut rc): (Transfers, [f64; 4]) = serde_json::from_slice(&saved).unwrap();
    for _ in 0..999 {
        ledger.accumulate(Transfers::from_values([1.; 4]), &mut correction);
        restored.accumulate(Transfers::from_values([1.; 4]), &mut rc);
    }
    assert_eq!(ledger.values(), [2_f64.powi(53) + 1000.; 4]);
    assert_eq!(ledger, restored);
    assert_eq!(correction, rc);
}

#[test]
fn physical_spherical_neighbors_cross_the_atlas_seam_and_malformed_adjacency_rejects() {
    let mut surface = Surface::build(2, 1_000_000.);
    let (source, edge) = (0..surface.areas.len())
        .find_map(|i| {
            let p = surface.centers[i];
            (surface.offsets[i] as usize..surface.offsets[i + 1] as usize).find_map(|k| {
                let q = surface.centers[surface.neighbors[k] as usize];
                (p[1].abs() < 0.9
                    && q[1].abs() < 0.9
                    && (p[2].atan2(p[0]) - q[2].atan2(q[0])).abs() > std::f64::consts::PI)
                    .then_some((i, k))
            })
        })
        .unwrap();
    let target = surface.neighbors[edge] as usize;
    let n = surface.areas.len();
    let mut receivers: Vec<_> = (0..n as u32).collect();
    receivers[source] = target as u32;
    let mut outlets: Vec<_> = (0..n as u32).collect();
    outlets[source] = target as u32;
    let drainage = Drainage {
        receivers,
        outlets,
        flat_steps: vec![0; n],
        contributing_area: vec![0.; n],
    };
    let net = Network::from_surface(&surface, &drainage, Settings::default()).unwrap();
    let mut stock = vec![0.; n];
    stock[source] = 10.;
    let step = net.advance(&stock, 600.).unwrap();
    assert!(step.terminal_delivery_kilograms[target] > 0.);
    close(
        step.transit_kilograms[source] + step.terminal_delivery_kilograms[target],
        10.,
    );
    let old_distance = surface.distances[edge];
    surface.distances[edge] = -1.;
    assert!(Network::from_surface(&surface, &drainage, Settings::default()).is_err());
    surface.distances[edge] = old_distance;
    let mut bad = drainage.clone();
    let far = (0..n)
        .find(|&i| {
            i != source
                && !surface.neighbors
                    [surface.offsets[source] as usize..surface.offsets[source + 1] as usize]
                    .contains(&(i as u32))
        })
        .unwrap();
    bad.receivers[source] = far as u32;
    bad.outlets[source] = far as u32;
    assert!(Network::from_surface(&surface, &bad, Settings::default()).is_err());
    surface.offsets[0] = 1;
    assert!(Network::from_surface(&surface, &drainage, Settings::default()).is_err());
}
