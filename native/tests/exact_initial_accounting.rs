use planimulation_core::{
    Recipe, World,
    exact_initial_accounting::{ExactInitialAccounting, FRACTION_BITS, exact_units},
    initial_water_inventory::InitialWaterInventory,
};

fn recipe() -> Recipe {
    serde_json::from_str(include_str!("../../docs/scenarios/spill-connections.json")).unwrap()
}

#[test]
fn generated_initial_water_has_one_exact_owned_ledger_and_round_trips() {
    let world = World::generate(recipe()).unwrap();
    let accounting = ExactInitialAccounting::from_world(&world).unwrap();
    assert_eq!(accounting.unit_fraction_bits, FRACTION_BITS);
    assert_eq!(accounting.stocks.len(), 5);
    let owned: i128 = accounting
        .stocks
        .iter()
        .map(|stock| stock.volume_units.parse::<i128>().unwrap())
        .sum();
    let source = accounting.source_total_units.parse::<i128>().unwrap();
    let total = accounting.exact_total_units.parse::<i128>().unwrap();
    assert_eq!(owned, total);
    assert_eq!(
        total - source,
        accounting.reconciliation_units.parse::<i128>().unwrap()
    );
    assert_ne!(total, source);
    assert!(accounting.exact_total_cubic_meters().unwrap().is_finite());
    let json = serde_json::to_string(&accounting).unwrap();
    let decoded: ExactInitialAccounting = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded.clone().restore().unwrap(), accounting);
    assert_eq!(serde_json::to_string(&decoded).unwrap(), json);
}

#[test]
fn generated_seed_ensemble_keeps_local_ownership_and_explicit_reconciliation() {
    let base = recipe();
    let mut reconciled = 0;
    for seed in 0..20 {
        let mut input = base.clone();
        input.seed = format!("accounting-import-{seed}");
        let world = World::generate(input).unwrap();
        let accounting = ExactInitialAccounting::from_world(&world).unwrap();
        reconciled += usize::from(accounting.reconciliation_units != "0");
        assert_eq!(accounting.clone().restore().unwrap(), accounting);
    }
    assert!(reconciled > 0);
    for water in [
        planimulation_core::water::WaterSettings::Coverage { fraction: 0. },
        planimulation_core::water::WaterSettings::Coverage { fraction: 1. },
        planimulation_core::water::WaterSettings::Volume {
            volume_cubic_meters: 1e15,
        },
    ] {
        let mut input = base.clone();
        input.water = water;
        let world = World::generate(input).unwrap();
        let accounting = ExactInitialAccounting::from_world(&world).unwrap();
        assert_eq!(accounting.clone().restore().unwrap(), accounting);
    }
    let mut detailed = base;
    detailed.subdivision = 5;
    let world = World::generate(detailed).unwrap();
    let accounting = ExactInitialAccounting::from_world(&world).unwrap();
    assert_eq!(accounting.clone().restore().unwrap(), accounting);
}

#[test]
fn invalid_imports_and_corrupted_ledgers_reject_without_reinterpreting_versions() {
    let world = World::generate(recipe()).unwrap();
    let baseline = ExactInitialAccounting::from_world(&world).unwrap();
    let mut variants = Vec::new();
    let mut altered = baseline.clone();
    altered.accounting_version = "future".into();
    variants.push(altered);
    let mut altered = baseline.clone();
    altered.unit_fraction_bits += 1;
    variants.push(altered);
    let mut altered = baseline.clone();
    altered.exact_total_units = "01".into();
    variants.push(altered);
    let mut altered = baseline.clone();
    altered.reconciliation_units = "0".into();
    variants.push(altered);
    let mut altered = baseline.clone();
    altered.stocks[0].volume_units = "0".into();
    variants.push(altered);
    let mut altered = baseline.clone();
    altered.stocks[0].legacy_stock_units = "0".into();
    variants.push(altered);
    let mut altered = baseline.clone();
    altered.stocks[1].branch = altered.stocks[0].branch;
    variants.push(altered);
    let mut altered = baseline.clone();
    altered.origin_recipe.seed = "different".into();
    variants.push(altered);
    for candidate in variants {
        assert!(candidate.restore().is_err());
    }
    assert_eq!(baseline.clone().restore().unwrap(), baseline);
    assert!(exact_units(1e-18).is_err());
    assert!(exact_units(f64::MAX).is_err());
    assert!(exact_units(f64::NAN).is_err());
    assert!(exact_units(-0.).is_err());
}

#[test]
fn analytical_bound_rejects_a_legacy_tolerated_unowned_total_change() {
    let mut world = World::generate(recipe()).unwrap();
    world.water.resolved_volume_cubic_meters += 100_000.;
    assert!(
        InitialWaterInventory::from_water(
            &world.surface,
            &world.terrain.elevation,
            &world.water,
            &world.basins,
        )
        .is_ok()
    );
    assert!(
        ExactInitialAccounting::from_world(&world)
            .unwrap_err()
            .contains("summation bound")
    );
}
