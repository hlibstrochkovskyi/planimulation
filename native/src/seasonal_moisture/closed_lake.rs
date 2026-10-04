//! Finite, isolated leaf-lake exposure and evaporation. Not coupled seasonal
//! evolution: callers own the donor and must credit actual atmospheric grants.
use super::{
    CLOSED_LAKE_MODEL_VERSION, Model, REFERENCE_POOL_MODEL_VERSION, State,
    WATER_DENSITY_KILOGRAMS_PER_CUBIC_METER,
};
use crate::{
    Recipe, World,
    moisture_transport::total_mass,
    reservoir::{Boundary, Column, Reservoir},
    surface_water::CompensatedStock,
};
use serde::Serialize;

pub mod spill;

pub const COMPONENT_VERSION: &str = "closed-leaf-lake-1";

#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Liquid {
    pub high_kilograms: f64,
    pub low_kilograms: f64,
}
impl Liquid {
    fn stock(self) -> Result<CompensatedStock, String> {
        CompensatedStock::new(self.high_kilograms, self.low_kilograms, f64::MAX)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Demand {
    pub temperature_celsius: f64,
    /// Already integrated over the caller's interval, before the temperature gate.
    pub potential_kilograms_per_square_meter: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Surface {
    pub volume_cubic_meters: f64,
    /// None exactly when dry. Relative height drives exposure, not rounded datum.
    pub height_above_minimum_meters: Option<f64>,
    /// Optional display coordinate; absent when its datum loses relative precision.
    pub absolute_level_meters: Option<f64>,
    pub exposed_area_square_meters: f64,
    pub exposed_regions: Vec<usize>,
    pub reconstruction_residual_cubic_meters: f64,
    pub at_spill_threshold: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Evaporation {
    pub remaining_liquid: Liquid,
    /// Same order as Lake::regions(); caller credits these actual grants once.
    pub regional_grants_kilograms: Vec<f64>,
    pub actual_evaporation_kilograms: f64,
    pub before_surface: Surface,
    pub after_surface: Surface,
    pub allocation_residual_kilograms: f64,
}

pub struct Lake {
    terminal_region: usize,
    basin_node: usize,
    regions: Vec<usize>,
    minimum_bed_meters: f64,
    columns: Vec<Column>,
    curve: Reservoir,
}
impl Lake {
    fn new(
        terminal_region: usize,
        basin_node: usize,
        regions: Vec<usize>,
        columns: Vec<Column>,
        spill: Option<f64>,
    ) -> Result<Self, String> {
        if regions.is_empty()
            || regions.len() != columns.len()
            || !regions.contains(&terminal_region)
        {
            return Err("Invalid closed-lake footprint.".into());
        }
        let minimum = columns
            .iter()
            .map(|c| c.bed_meters)
            .fold(f64::INFINITY, f64::min);
        let relative: Vec<_> = columns
            .iter()
            .map(|c| Column {
                bed_meters: c.bed_meters - minimum,
                area_square_meters: c.area_square_meters,
            })
            .collect();
        let boundary = spill.map_or(Boundary::Closed, |h| Boundary::ExternalCollector {
            spill_level_meters: h - minimum,
        });
        // The old isolated-reservoir curve is reused as a read-only index only.
        // Its external boundary marks the first connection, not a physical sink.
        let curve = Reservoir::new(relative.clone(), boundary, 0.)?;
        if curve.capacity_cubic_meters().is_some_and(|c| {
            let mass = c * WATER_DENSITY_KILOGRAMS_PER_CUBIC_METER;
            !mass.is_finite() || mass <= 0.
        }) {
            return Err("Closed-lake first-connection mass capacity overflow or underflow.".into());
        }
        Ok(Self {
            terminal_region,
            basin_node,
            regions,
            minimum_bed_meters: minimum,
            columns: relative,
            curve,
        })
    }
    pub fn terminal_region(&self) -> usize {
        self.terminal_region
    }
    pub fn basin_node(&self) -> usize {
        self.basin_node
    }
    pub fn regions(&self) -> &[usize] {
        &self.regions
    }
    pub fn capacity_cubic_meters(&self) -> Option<f64> {
        self.curve.capacity_cubic_meters()
    }

    /// Geometry is derived from a rounded volume; the input pair stays authoritative.
    /// A stock above the first connection rejects instead of inventing a merger.
    pub fn surface(&self, liquid: Liquid) -> Result<Surface, String> {
        liquid.stock()?;
        if let Some(capacity) = self.capacity_cubic_meters() {
            let mass_capacity = capacity * WATER_DENSITY_KILOGRAMS_PER_CUBIC_METER;
            // Validate represented components before the rounded kg -> m³
            // conversion can hide a positive tail above the first connection.
            CompensatedStock::new(liquid.high_kilograms, liquid.low_kilograms, mass_capacity)
                .map_err(|_| {
                    "Closed lake exceeds its represented first-connection mass capacity."
                        .to_string()
                })?;
        }
        let volume = total_mass(&[liquid.high_kilograms, liquid.low_kilograms])
            / WATER_DENSITY_KILOGRAMS_PER_CUBIC_METER;
        if !volume.is_finite() || (liquid.high_kilograms > 0. && volume == 0.) {
            return Err("Closed-lake volume overflow or underflow.".into());
        }
        if self.capacity_cubic_meters().is_some_and(|c| volume > c) {
            return Err("Closed lake exceeds its first connection; explicit spill/merge ownership is required.".into());
        }
        let height = self.curve.level_for_volume(volume)?;
        let exposed: Vec<_> = height.map_or_else(Vec::new, |h| {
            self.columns
                .iter()
                .enumerate()
                .filter(|(_, c)| c.bed_meters < h)
                .map(|(i, _)| i)
                .collect()
        });
        let area = total_mass(
            &exposed
                .iter()
                .map(|&i| self.columns[i].area_square_meters)
                .collect::<Vec<_>>(),
        );
        if !area.is_finite() || (volume > 0. && area <= 0.) {
            return Err("Invalid closed-lake exposed area.".into());
        }
        let represented = height.map_or(Ok(0.), |h| self.curve.volume_at_level(h))?;
        let absolute = height.and_then(|h| {
            let level = self.minimum_bed_meters + h;
            let reconstructed = level - self.minimum_bed_meters;
            (level.is_finite() && reconstructed > 0. && (reconstructed - h).abs() <= h * 1e-10)
                .then_some(level)
        });
        Ok(Surface {
            volume_cubic_meters: volume,
            height_above_minimum_meters: height,
            absolute_level_meters: absolute,
            exposed_area_square_meters: area,
            exposed_regions: exposed.iter().map(|&i| self.regions[i]).collect(),
            reconstruction_residual_cubic_meters: represented - volume,
            at_spill_threshold: self.capacity_cubic_meters().is_some_and(|c| {
                liquid.high_kilograms == c * WATER_DENSITY_KILOGRAMS_PER_CUBIC_METER
                    && liquid.low_kilograms == 0.
            }),
        })
    }

    /// One explicit frozen-footprint interval. Drying can expose a lower shelf
    /// afterwards; splitting/merging and moving seasonal masks are not simulated.
    pub fn evaporate(
        &self,
        liquid: Liquid,
        demand: &[Demand],
        enabled: bool,
    ) -> Result<Evaporation, String> {
        if demand.len() != self.regions.len()
            || demand.iter().any(|d| {
                !d.temperature_celsius.is_finite()
                    || !d.potential_kilograms_per_square_meter.is_finite()
                    || d.potential_kilograms_per_square_meter < 0.
            })
        {
            return Err("Invalid closed-lake evaporation forcing.".into());
        }
        let before = self.surface(liquid)?;
        let requests: Vec<_> = self
            .columns
            .iter()
            .zip(demand)
            .map(|(c, d)| {
                if enabled
                    && d.temperature_celsius > 0.
                    && before
                        .height_above_minimum_meters
                        .is_some_and(|h| c.bed_meters < h)
                {
                    c.area_square_meters * d.potential_kilograms_per_square_meter
                } else {
                    0.
                }
            })
            .collect();
        let (after, grants, residual) =
            super::reference_pool::allocate(liquid.stock()?, &requests)?;
        let remaining = Liquid {
            high_kilograms: after.high,
            low_kilograms: after.low,
        };
        let after_surface = self.surface(remaining)?;
        let actual = total_mass(&grants);
        Ok(Evaporation {
            remaining_liquid: remaining,
            regional_grants_kilograms: grants,
            actual_evaporation_kilograms: actual,
            before_surface: before,
            after_surface,
            allocation_residual_kilograms: residual,
        })
    }
}

/// Exclusive leaf footprints: no copies of ancestor subtrees and no ocean donors.
pub struct Layout {
    recipe: Recipe,
    lakes: Vec<Lake>,
}
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Observation {
    pub terminal_region: usize,
    pub basin_node: usize,
    pub footprint_regions: usize,
    pub liquid: Liquid,
    pub capacity_cubic_meters: Option<f64>,
    pub surface: Option<Surface>,
    /// Geometric refusal is retained; stock is never capped or moved by observation.
    pub surface_failure: Option<String>,
}
impl Layout {
    pub fn from_world(world: &World) -> Result<Self, String> {
        let n = world.surface.areas.len();
        if world.terrain.elevation.len() != n
            || world.water.body_ids.len() != n
            || world.drainage.receivers.len() != n
            || world.basins.region_nodes().len() != n
        {
            return Err("Closed-lake geometry fields differ.".into());
        }
        let k = world.basins.nodes().len();
        let mut owned = vec![Vec::new(); k];
        for (i, &b) in world.basins.region_nodes().iter().enumerate() {
            owned
                .get_mut(b)
                .ok_or("Invalid closed-lake branch owner.")?
                .push(i);
        }
        let mut used = vec![false; k];
        let mut lakes = Vec::new();
        for i in 0..n {
            if world.water.body_ids[i] != 0 || world.drainage.receivers[i] as usize != i {
                continue;
            }
            let b = world.basins.region_nodes()[i];
            let node = &world.basins.nodes()[b];
            if used[b]
                || !node.children.is_empty()
                || node.birth_level_meters != world.terrain.elevation[i]
                || owned[b].iter().any(|&r| world.water.body_ids[r] != 0)
            {
                return Err(
                    "Closed terminal does not have an exclusive initially dry minimum leaf.".into(),
                );
            }
            used[b] = true;
            let regions = std::mem::take(&mut owned[b]);
            let columns = regions
                .iter()
                .map(|&r| Column {
                    bed_meters: world.terrain.elevation[r],
                    area_square_meters: world.surface.areas[r],
                })
                .collect();
            lakes.push(Lake::new(i, b, regions, columns, node.spill_level_meters)?);
        }
        Ok(Self {
            recipe: world.recipe.clone(),
            lakes,
        })
    }
    pub fn lakes(&self) -> &[Lake] {
        &self.lakes
    }
    fn validate_observation(&self, model: &Model, state: &State) -> Result<(), String> {
        if ![REFERENCE_POOL_MODEL_VERSION, CLOSED_LAKE_MODEL_VERSION]
            .contains(&model.model_version())
            || model.origin.recipe != self.recipe
        {
            return Err("Closed-lake observations require a matching model-8/9 recipe.".into());
        }
        model.budget(state)?;
        Ok(())
    }
    /// Independent frozen endpoint probe, not the next physical interval.
    /// Thermal/capacity fields remain the existing fixed-geography monthly ones.
    pub fn probe(
        &self,
        model: &Model,
        state: &State,
        lake_index: usize,
        month: usize,
        seconds: u32,
    ) -> Result<Evaporation, String> {
        self.validate_observation(model, state)?;
        if month >= 12 || !(1..=3600).contains(&seconds) {
            return Err(
                "Closed-lake probe requires month 0..11 and duration 1..3600 seconds.".into(),
            );
        }
        let lake = self
            .lakes
            .get(lake_index)
            .ok_or("Invalid closed-lake probe index.")?;
        let response =
            -(-(seconds as f64) / model.settings().evaporation_response_seconds).exp_m1();
        let demand: Vec<_> = lake
            .regions
            .iter()
            .map(|&i| Demand {
                temperature_celsius: model.temperatures[month][i],
                potential_kilograms_per_square_meter: (model.capacities[month][i]
                    - state.0.vapor_kilograms[i])
                    .max(0.)
                    * response
                    / model.areas[i],
            })
            .collect();
        let i = lake.terminal_region;
        lake.evaporate(
            Liquid {
                high_kilograms: state.0.terminal_water_kilograms[i],
                low_kilograms: state.0.terminal_low_kilograms.as_ref().unwrap()[i],
            },
            &demand,
            model.settings().evaporation_enabled,
        )
    }
    pub fn capture(&self, model: &Model, state: &State) -> Result<Vec<Observation>, String> {
        self.validate_observation(model, state)?;
        self.lakes
            .iter()
            .map(|lake| {
                let i = lake.terminal_region;
                let liquid = Liquid {
                    high_kilograms: state.0.terminal_water_kilograms[i],
                    low_kilograms: state.0.terminal_low_kilograms.as_ref().unwrap()[i],
                };
                let (surface, failure) = match lake.surface(liquid) {
                    Ok(s) => (Some(s), None),
                    Err(e) => (None, Some(e)),
                };
                Ok(Observation {
                    terminal_region: i,
                    basin_node: lake.basin_node,
                    footprint_regions: lake.regions.len(),
                    liquid,
                    capacity_cubic_meters: lake.capacity_cubic_meters(),
                    surface,
                    surface_failure: failure,
                })
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn lake(datum: f64) -> Lake {
        Lake::new(
            0,
            0,
            vec![0, 1, 2],
            vec![
                Column {
                    bed_meters: datum,
                    area_square_meters: 2.,
                },
                Column {
                    bed_meters: datum + 1.,
                    area_square_meters: 3.,
                },
                Column {
                    bed_meters: datum + 2.,
                    area_square_meters: 4.,
                },
            ],
            Some(datum + 3.),
        )
        .unwrap()
    }
    fn water(volume: f64) -> Liquid {
        Liquid {
            high_kilograms: volume * 1000.,
            low_kilograms: 0.,
        }
    }
    #[test]
    fn storage_exposure_and_exact_shelves_are_independently_known() {
        let lake = lake(0.);
        for (v, h, a, regions) in [
            (0., None, 0., vec![]),
            (1., Some(0.5), 2., vec![0]),
            (2., Some(1.), 2., vec![0]),
            (4.5, Some(1.5), 5., vec![0, 1]),
            (7., Some(2.), 5., vec![0, 1]),
            (16., Some(3.), 9., vec![0, 1, 2]),
        ] {
            let s = lake.surface(water(v)).unwrap();
            assert_eq!(s.height_above_minimum_meters, h);
            assert_eq!(s.exposed_area_square_meters, a);
            assert_eq!(s.exposed_regions, regions);
            assert_eq!(s.reconstruction_residual_cubic_meters, 0.);
            assert_eq!(s.at_spill_threshold, v == 16.);
        }
        assert!(lake.surface(water(16.0001)).is_err());
        assert!(
            lake.surface(Liquid {
                high_kilograms: 16000.,
                low_kilograms: 2_f64.powi(-40)
            })
            .is_err()
        );
        assert!(
            !lake
                .surface(Liquid {
                    high_kilograms: 16000.,
                    low_kilograms: -2_f64.powi(-40)
                })
                .unwrap()
                .at_spill_threshold
        );
    }
    #[test]
    fn finite_grants_follow_exposure_temperature_and_disable_gates() {
        let lake = lake(0.);
        let demand = [Demand {
            temperature_celsius: 1.,
            potential_kilograms_per_square_meter: 100.,
        }; 3];
        let e = lake.evaporate(water(4.5), &demand, true).unwrap();
        assert_eq!(e.regional_grants_kilograms, [200., 300., 0.]);
        assert_eq!(e.remaining_liquid, water(4.));
        assert_eq!(e.actual_evaporation_kilograms, 500.);
        assert_eq!(e.allocation_residual_kilograms, 0.);
        for temperature in [0., -1.] {
            let cold = [Demand {
                temperature_celsius: temperature,
                ..demand[0]
            }; 3];
            assert_eq!(
                lake.evaporate(water(4.5), &cold, true)
                    .unwrap()
                    .remaining_liquid,
                water(4.5)
            );
        }
        assert_eq!(
            lake.evaporate(water(4.5), &demand, false)
                .unwrap()
                .remaining_liquid,
            water(4.5)
        );
        assert_eq!(
            lake.evaporate(water(0.), &demand, true)
                .unwrap()
                .actual_evaporation_kilograms,
            0.
        );
        let mut mixed = demand;
        mixed[0].temperature_celsius = 0.;
        assert_eq!(
            lake.evaporate(water(4.5), &mixed, true)
                .unwrap()
                .regional_grants_kilograms,
            [0., 300., 0.]
        );
    }
    #[test]
    fn donor_depletion_keeps_owned_tails_and_datum_does_not_change_exposure() {
        let demand = [Demand {
            temperature_celsius: 1.,
            potential_kilograms_per_square_meter: 1e6,
        }; 3];
        for datum in [0., -4000., 2_f64.powi(50)] {
            let l = lake(datum);
            let liquid = Liquid {
                high_kilograms: 1.,
                low_kilograms: -2_f64.powi(-54),
            };
            let e = l.evaporate(liquid, &demand, true).unwrap();
            assert!(e.actual_evaporation_kilograms <= liquid.stock().unwrap().available());
            assert!(e.remaining_liquid.high_kilograms > 0.);
            assert_eq!(e.regional_grants_kilograms[1..], [0., 0.]);
            assert_eq!(e.after_surface.exposed_regions, [0]);
            if datum == 2_f64.powi(50) {
                assert_eq!(e.before_surface.absolute_level_meters, None);
            }
        }
    }
    #[test]
    fn drying_crosses_shelves_without_spilling_or_changing_other_owners() {
        let l = lake(0.);
        let demand = [Demand {
            temperature_celsius: 1.,
            potential_kilograms_per_square_meter: 500.,
        }; 3];
        let e = l.evaporate(water(4.5), &demand, true).unwrap();
        assert_eq!(e.remaining_liquid, water(2.));
        assert_eq!(e.after_surface.exposed_regions, [0]);
        assert_eq!(e.after_surface.exposed_area_square_meters, 2.);
        let next = l.evaporate(e.remaining_liquid, &demand, true).unwrap();
        assert_eq!(next.regional_grants_kilograms, [1000., 0., 0.]);
    }
    #[test]
    fn closed_root_area_scaling_and_recipient_reordering_preserve_the_operator() {
        let forcing = [Demand {
            temperature_celsius: 10.,
            potential_kilograms_per_square_meter: 100.,
        }; 3];
        let base = lake(0.).evaporate(water(4.5), &forcing, true).unwrap();
        for scale in [0.5, 2.] {
            let l = Lake::new(
                10,
                0,
                vec![10, 20, 30],
                vec![
                    Column {
                        bed_meters: 0.,
                        area_square_meters: 2. * scale,
                    },
                    Column {
                        bed_meters: 1.,
                        area_square_meters: 3. * scale,
                    },
                    Column {
                        bed_meters: 2.,
                        area_square_meters: 4. * scale,
                    },
                ],
                None,
            )
            .unwrap();
            let e = l.evaporate(water(4.5 * scale), &forcing, true).unwrap();
            assert_eq!(
                e.before_surface.height_above_minimum_meters,
                base.before_surface.height_above_minimum_meters
            );
            assert_eq!(
                e.before_surface.exposed_area_square_meters,
                base.before_surface.exposed_area_square_meters * scale
            );
            assert_eq!(
                e.actual_evaporation_kilograms,
                base.actual_evaporation_kilograms * scale
            );
            assert!(!l.surface(water(32. * scale)).unwrap().at_spill_threshold);
            assert_eq!(l.capacity_cubic_meters(), None);
        }
        let reordered = Lake::new(
            10,
            0,
            vec![20, 10, 30],
            vec![
                Column {
                    bed_meters: 1.,
                    area_square_meters: 3.,
                },
                Column {
                    bed_meters: 0.,
                    area_square_meters: 2.,
                },
                Column {
                    bed_meters: 2.,
                    area_square_meters: 4.,
                },
            ],
            Some(3.),
        )
        .unwrap();
        let e = reordered.evaporate(water(4.5), &forcing, true).unwrap();
        assert_eq!(
            e.regional_grants_kilograms,
            [
                base.regional_grants_kilograms[1],
                base.regional_grants_kilograms[0],
                base.regional_grants_kilograms[2]
            ]
        );
        assert_eq!(e.remaining_liquid, base.remaining_liquid);
    }
    #[test]
    fn malformed_and_unrepresentable_inputs_do_not_mutate_callers() {
        let l = lake(0.);
        let input = water(1.);
        for d in [f64::NAN, f64::INFINITY, -1.] {
            let demand = [Demand {
                temperature_celsius: 1.,
                potential_kilograms_per_square_meter: d,
            }; 3];
            assert!(l.evaporate(input, &demand, true).is_err());
        }
        assert!(l.evaporate(input, &[], true).is_err());
        let malformed_temperature = [Demand {
            temperature_celsius: f64::NAN,
            potential_kilograms_per_square_meter: 0.,
        }; 3];
        assert!(l.evaporate(input, &malformed_temperature, false).is_err());
        assert!(
            Lake::new(
                0,
                0,
                vec![0],
                vec![Column {
                    bed_meters: 0.,
                    area_square_meters: 1e306
                }],
                Some(1.)
            )
            .is_err()
        );
        assert!(
            l.surface(Liquid {
                high_kilograms: 1.,
                low_kilograms: 0.1
            })
            .is_err()
        );
        assert!(
            l.surface(Liquid {
                high_kilograms: f64::from_bits(1),
                low_kilograms: 0.
            })
            .is_err()
        );
        assert_eq!(input, water(1.));
    }
}
