use super::*;

fn chain(heights: &[f64]) -> (Layout, SeasonalCheckpoint) {
    let (layout, cp, _) = chain_geometry(heights);
    (layout, cp)
}

fn chain_geometry(heights: &[f64]) -> (Layout, SeasonalCheckpoint, closed_lake::Layout) {
    // Synthetic reciprocal topology is for directed operator tests, not recipe restore.
    let recipe = serde_json::from_str(include_str!(
        "../../../../docs/scenarios/seasonal-temperature.json"
    ))
    .unwrap();
    let mut world = World::generate(recipe).unwrap();
    let model = super::super::Model::from_world(
        &world,
        Default::default(),
        Default::default(),
        Default::default(),
    )
    .unwrap();
    let mut cp = model.initial_state().checkpoint();
    let n = heights.len();
    let mut neighbors = Vec::new();
    let mut offsets = vec![0];
    for r in 0..n {
        if r > 0 {
            neighbors.push((r - 1) as u32);
        }
        if r + 1 < n {
            neighbors.push((r + 1) as u32);
        }
        offsets.push(neighbors.len() as u32);
    }
    world.surface = crate::Surface {
        centers: vec![],
        faces: vec![],
        distances: vec![1.; neighbors.len()],
        offsets,
        neighbors,
        areas: vec![1.; n],
        boundary_offsets: vec![],
        boundaries: vec![],
    };
    world.terrain.elevation = heights.to_vec();
    world.water.body_ids = vec![0; n];
    world.basins = crate::basins::Basins::build(&world.surface, heights).unwrap();
    world.drainage =
        crate::drainage::Drainage::build(&world.surface, heights, &world.water.body_ids);
    let geometry = closed_lake::Layout::from_world(&world).unwrap();
    let layout = Layout::from_world(&world, &geometry).unwrap();
    cp.elapsed_seconds = 1;
    cp.terminal_water_kilograms = vec![0.; n];
    cp.terminal_low_kilograms = Some(vec![0.; n]);
    cp.leaf_spill_state = Some(leaf_spill::Checkpoint::zero(n));
    cp.merged_lake_state = Some(Checkpoint::empty());
    cp.cumulative_lake_capture_kilograms = Some(vec![0.; n]);
    cp.cumulative_lake_capture_low_kilograms = Some(vec![0.; n]);
    cp.cumulative_runoff_transfers = vec![Default::default(); n];
    (layout, cp, geometry)
}

#[test]
fn three_way_birth_and_incremental_geometry_have_exact_integer_ownership() {
    let (layout, mut cp) = chain(&[0., 2., 1., 2., 0.]);
    assert_eq!(layout.groups.len(), 1);
    let group = &layout.groups[0];
    for (&r, &cap) in group
        .description
        .child_terminals
        .iter()
        .zip(&group.description.child_capacities_kilograms)
    {
        cp.terminal_water_kilograms[r] = cap;
        cp.cumulative_lake_capture_kilograms.as_mut().unwrap()[r] = cap;
    }
    let r = group.description.child_terminals[0];
    cp.terminal_low_kilograms.as_mut().unwrap()[r] = -2_f64.powi(-54);
    layout.settle(&mut cp).unwrap();
    assert!(cp.merged_lake_state.as_ref().unwrap().parents.is_empty());
    cp.terminal_low_kilograms.as_mut().unwrap()[r] = 0.;
    layout.settle(&mut cp).unwrap();
    let p = &cp.merged_lake_state.as_ref().unwrap().parents[0];
    assert_eq!(p.birth_high_kilograms, 5000.);
    assert_eq!(p.birth_low_kilograms, 0.);
    assert_eq!(layout.surfaces(&cp).unwrap()[0].exposed_regions, [0, 2, 4]);
    assert!(
        group
            .description
            .child_terminals
            .iter()
            .all(|&r| cp.terminal_water_kilograms[r] == 0.)
    );
    cp.leaf_spill_state
        .as_mut()
        .unwrap()
        .pending_input
        .high_kilograms[r] = 6000.;
    cp.cumulative_lake_capture_kilograms.as_mut().unwrap()[r] += 6000.;
    layout.settle(&mut cp).unwrap();
    assert_eq!(
        layout.surfaces(&cp).unwrap()[0].exposed_regions,
        [0, 1, 2, 3, 4]
    );
    assert_eq!(
        layout.surfaces(&cp).unwrap()[0].relative_height_above_birth_meters,
        1.2
    );
    let mut grants = vec![0.; 5];
    layout
        .evaporate(&mut cp, &[100., 200., 300., 400., 500.], &mut grants)
        .unwrap();
    assert_eq!(grants, [100., 200., 300., 400., 500.]);
    for (r, &grant) in grants.iter().enumerate() {
        cp.cumulative_runoff_transfers[r].terminal_evaporation += grant;
    }
    let parent = &cp.merged_lake_state.as_ref().unwrap().parents[0];
    assert_eq!(parent.surplus_high_kilograms, 4500.);
    assert_eq!(parent.surplus_low_kilograms, 0.);
    let integer_components: i128 = parent
        .components()
        .iter()
        .map(|&v| {
            assert_eq!(v as i128 as f64, v);
            v as i128
        })
        .sum();
    assert_eq!(integer_components + 1500, 11000);
    assert_eq!(layout.validate(&cp).unwrap().0, 0.);
    assert_eq!(
        layout.surfaces(&cp).unwrap()[0].relative_height_above_birth_meters,
        0.9
    );
    let before = cp.clone();
    let mut provisional = cp.clone();
    assert!(
        layout
            .evaporate(&mut provisional, &[1000.; 5], &mut grants)
            .unwrap_err()
            .contains("drying/splitting")
    );
    assert_eq!(cp, before);
}

#[test]
fn birth_only_owner_has_no_fictitious_evaporation_and_tiny_debits_refuse() {
    let (layout, mut cp) = chain(&[0., 2., 1.]);
    let group = &layout.groups[0];
    for (&r, &cap) in group
        .description
        .child_terminals
        .iter()
        .zip(&group.description.child_capacities_kilograms)
    {
        cp.terminal_water_kilograms[r] = cap;
    }
    layout.settle(&mut cp).unwrap();
    let mut regional = vec![0.; 3];
    assert!(
        layout
            .evaporate(&mut cp, &[1., 0., 0.], &mut regional)
            .unwrap_err()
            .contains("drying/splitting")
    );
    layout.evaporate(&mut cp, &[0.; 3], &mut regional).unwrap();
    assert_eq!(regional, [0.; 3]);
    let parent = &mut cp.merged_lake_state.as_mut().unwrap().parents[0];
    parent.surplus_high_kilograms = 5e14;
    parent.surplus_low_kilograms = 1e-300;
    assert!(
        layout
            .evaporate(&mut cp, &[1e-320, 0., 0.], &mut regional)
            .unwrap_err()
            .contains("resolution")
    );
}

#[test]
fn reversible_three_way_frontier_retains_provenance_and_independent_children() {
    let (mut layout, mut cp, geometry) = chain_geometry(&[0., 2., 1., 2., 0.]);
    layout.split_enabled = true;
    cp.merged_lake_state = Some(layout.frontier_checkpoint());
    let description = layout.groups[0].description.clone();
    for (&r, &cap) in description
        .child_terminals
        .iter()
        .zip(&description.child_capacities_kilograms)
    {
        cp.terminal_water_kilograms[r] = cap;
        cp.cumulative_lake_capture_kilograms.as_mut().unwrap()[r] = cap;
    }
    // At exactly zero depth on the sill, children remain separate, avoiding chatter.
    layout.settle(&mut cp).unwrap();
    assert!(cp.merged_lake_state.as_ref().unwrap().parents.is_empty());
    let r = description.child_terminals[0];
    cp.leaf_spill_state
        .as_mut()
        .unwrap()
        .pending_input
        .high_kilograms[r] = 1500.;
    cp.cumulative_lake_capture_kilograms.as_mut().unwrap()[r] += 1500.;
    layout.settle(&mut cp).unwrap();
    assert_eq!(layout.validate(&cp).unwrap().0, 0.);
    // A second parent-only capture must remain attributable after this parent dries.
    layout.record_capture(&mut cp, 1, 500., 0.).unwrap();
    cp.cumulative_lake_capture_kilograms.as_mut().unwrap()[1] = 500.;
    cp.leaf_spill_state
        .as_mut()
        .unwrap()
        .pending_input
        .high_kilograms[r] = 500.;
    layout.settle(&mut cp).unwrap();
    // Dyadic proportional shares: common layer is 2000 / 4000 = half the demand.
    let (packets, residual) = layout
        .evaporate_frontier(&mut cp, &[256., 512., 768., 1024., 1440.], &geometry)
        .unwrap();
    assert_eq!(residual, 0.);
    for (i, grant) in &packets {
        cp.cumulative_runoff_transfers[*i].terminal_evaporation += grant;
    }
    assert!(cp.merged_lake_state.as_ref().unwrap().parents.is_empty());
    assert_eq!(cp.terminal_water_kilograms, [1872., 0., 616., 0., 1280.]);
    assert_eq!(cp.terminal_low_kilograms.as_ref().unwrap(), &[0.; 5]);
    assert_eq!(layout.validate(&cp).unwrap().0, 0.);
    let owned: i128 = cp.terminal_water_kilograms.iter().map(|v| *v as i128).sum();
    let evaporated: i128 = packets.iter().map(|(_, v)| *v as i128).sum();
    assert_eq!(owned + evaporated, 7000);
    let history = cp
        .merged_lake_state
        .as_ref()
        .unwrap()
        .frontier
        .as_ref()
        .unwrap();
    assert_eq!(history.merge_counts, [1]);
    assert_eq!(history.split_counts, [1]);
    // Refill only the actual deficits; historical parent-only flows remain untouched.
    for (&r, &cap) in description
        .child_terminals
        .iter()
        .zip(&description.child_capacities_kilograms)
    {
        let refill = cap - cp.terminal_water_kilograms[r];
        cp.terminal_water_kilograms[r] = cap;
        cp.cumulative_lake_capture_kilograms.as_mut().unwrap()[r] += refill;
    }
    cp.leaf_spill_state
        .as_mut()
        .unwrap()
        .pending_input
        .high_kilograms[r] = 512.;
    cp.cumulative_lake_capture_kilograms.as_mut().unwrap()[r] += 512.;
    cp.elapsed_seconds = 2;
    layout.settle(&mut cp).unwrap();
    assert_eq!(layout.validate(&cp).unwrap().0, 0.);
    let history = cp
        .merged_lake_state
        .as_ref()
        .unwrap()
        .frontier
        .as_ref()
        .unwrap();
    assert_eq!(history.merge_counts, [2]);
    assert_eq!(history.split_counts, [1]);
}
