//! Opt-in geological assumption: impermeable depressions are fully supplied.
//! Potential lake surfaces transmit contributing area; submerged actual bed
//! does not undergo stream-power incision. This is not seasonal water physics.
use crate::{
    facet_incision, landscape, potential_patch_surface, terrain_patch_facets,
    terrain_patch_geometry::Geometry,
};
use serde::Serialize;

pub const MODEL_VERSION: &str = "humid-patch-routing-1";
pub const ASSUMPTION: &str = "Fully supplied impermeable potential lakes in geological prehistory; no actual water inventory or sediment deposition; frozen lake levels within each incision stage.";

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Routing {
    pub model_version: &'static str,
    pub assumption: &'static str,
    pub potential: potential_patch_surface::Potential,
    pub proxy_routing: terrain_patch_facets::Routing,
    /// Actual bed may still contain genuine closed basins. Never substitute
    /// proxy connectivity for this physical morphology diagnostic.
    pub actual_bed_routing: terrain_patch_facets::Routing,
}
pub(crate) struct Router(terrain_patch_facets::Router);
impl Router {
    pub(crate) fn new(g: &Geometry) -> Result<Self, String> {
        Ok(Self(terrain_patch_facets::Router::new(g)?))
    }
    pub(crate) fn route(&self, bed: &[f64]) -> Result<Routing, String> {
        let potential = potential_patch_surface::analyze(self.0.geometry(), bed)?;
        let proxy_routing = self.0.route(&potential.surface_meters)?;
        if proxy_routing.closed_sink_count != 0
            || proxy_routing.closed_fraction_of_active_area != 0.
        {
            return Err("Potential lake geometry did not connect to the perimeter.".into());
        }
        Ok(Routing {
            model_version: MODEL_VERSION,
            assumption: ASSUMPTION,
            potential,
            proxy_routing,
            actual_bed_routing: self.0.route(bed)?,
        })
    }
}
/// Called only with flow just rebuilt by the owned router from this same bed.
/// Full proxy ancestry supplies A, but wet proxy nodes become fixed incision
/// terminals. Copy accepted dry-node heights back; leave actual lake bed intact.
pub(crate) fn incise(
    g: &Geometry,
    bed: &[f64],
    flow: &Routing,
    settings: facet_incision::Settings,
) -> Result<(landscape::Incision, facet_incision::Constraint), String> {
    let mut descents = flow.proxy_routing.descents.clone();
    for (i, d) in descents.iter_mut().enumerate() {
        if flow.potential.depth_meters[i] > 0. {
            *d = terrain_patch_facets::Descent {
                receivers: [i as u32; 2],
                ..Default::default()
            };
        }
    }
    let mut result = facet_incision::incise(
        &flow.potential.surface_meters,
        &g.reference_areas_square_meters,
        &flow.proxy_routing.contributing_area_square_meters,
        &descents,
        settings,
    )?;
    for (i, &height) in bed.iter().enumerate() {
        if flow.potential.depth_meters[i] > 0. {
            // A terminal cannot incise. These checks protect the representation
            // projection; they do not correct an accounting discrepancy.
            if result.incision.removed_thickness_meters[i] != 0.
                || result.receiver_constraint.prevented_incision_meters[i] != 0.
                || result.incision.elevation_meters[i] != flow.potential.surface_meters[i]
            {
                return Err("Potential lake incision terminal unexpectedly changed.".into());
            }
            result.incision.elevation_meters[i] = height;
        }
    }
    Ok((result.incision, result.receiver_constraint))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        terrain_patch_geometry::{self, Settings},
        terrain_patch_landscape as preparation,
    };
    fn grid() -> Geometry {
        terrain_patch_geometry::build(Settings {
            radius_meters: 6371000.,
            center_direction: [1., 0., 0.],
            orientation_radians: 0.,
            cells_x: 7,
            cells_y: 7,
            spacing_meters: 250.,
        })
        .unwrap()
    }
    #[test]
    fn proxy_connectivity_is_not_actual_bed_drainage_or_funded_water() {
        let g = grid();
        let mut h = vec![10.; 49];
        h[24] = -5.;
        let p = Router::new(&g).unwrap().route(&h).unwrap();
        assert!(p.actual_bed_routing.closed_fraction_of_active_area > 0.);
        assert_eq!(p.proxy_routing.closed_fraction_of_active_area, 0.);
        assert_eq!(p.potential.depth_meters[24], 15.);
        assert_eq!(p.potential.summary.positive_depth_nodes, 1);
        assert!(p.proxy_routing.contributing_area_square_meters.iter()
            .zip(&p.actual_bed_routing.contributing_area_square_meters)
            .any(|(&proxy, &actual)| proxy > actual + 0.5*g.reference_areas_square_meters[24]));
        let before = h.clone();
        let (e, _) = incise(
            &g,
            &h,
            &p,
            facet_incision::Settings {
                erodibility_per_year: 1e-6,
                step_years: 75000.,
            },
        )
        .unwrap();
        assert_eq!(e.elevation_meters, h);
        assert_eq!(e.removed_material_cubic_meters, 0.);
        assert_eq!(h, before);
    }
    #[test]
    fn submerged_bed_is_protected_while_dry_sills_can_erode() {
        let g = grid();
        let mut h: Vec<_> = (0..49)
            .map(|i| {
                if g.fixed_external_boundary[i] {
                    0.
                } else {
                    10.
                }
            })
            .collect();
        h[24] = -5.;
        let flow = Router::new(&g).unwrap().route(&h).unwrap();
        assert_eq!(flow.potential.surface_meters[24], 10.);
        let (e, c) = incise(
            &g,
            &h,
            &flow,
            facet_incision::Settings {
                erodibility_per_year: 1e-6,
                step_years: 75000.,
            },
        )
        .unwrap();
        assert_eq!(e.elevation_meters[24], -5.);
        assert_eq!(e.removed_thickness_meters[24], 0.);
        assert_eq!(c.prevented_incision_meters[24], 0.);
        assert!(e.removed_material_cubic_meters > 0.);
        let actual: f64 = h
            .iter()
            .zip(&e.elevation_meters)
            .zip(&g.reference_areas_square_meters)
            .map(|((&old, &new), &a)| (old - new) * a)
            .sum();
        assert!((actual / e.removed_material_cubic_meters - 1.).abs() < 1e-12);
        for (i, &old) in h.iter().enumerate() {
            assert!(e.elevation_meters[i] <= old);
        }
        let next = Router::new(&g).unwrap().route(&e.elevation_meters).unwrap();
        assert!(next.potential.surface_meters[24] < 10.);
    }
    #[test]
    fn no_pits_reproduce_dry_facet_incision_exactly() {
        let g = grid();
        let h: Vec<_> = (0..49).map(|i| i as f64).collect();
        let settings = preparation::Settings {
            erodibility_per_year: 1e-6,
            diffusivity_square_meters_per_year: 0.,
            step_years: 1000.,
            steps: 1,
        };
        let dry = preparation::prepare_facets(&g, &h, &[0.; 49], settings).unwrap();
        let wet = preparation::prepare_humid_facets(&g, &h, &[0.; 49], settings).unwrap();
        assert_eq!(dry.elevation_meters, wet.elevation_meters);
        assert_eq!(
            dry.removed_incision_thickness_meters,
            wet.removed_incision_thickness_meters
        );
        assert_eq!(
            dry.detached_material_reservoir_cubic_meters,
            wet.detached_material_reservoir_cubic_meters
        );
        assert!(dry.history[0].potential_geometry.is_none());
        assert_eq!(
            wet.history[0]
                .potential_geometry
                .unwrap()
                .positive_depth_nodes,
            0
        );
    }
    #[test]
    fn coupled_preparation_repeats_and_actual_prism_decomposes() {
        let g = grid();
        let mut h = vec![10.; 49];
        h[24] = -5.;
        let u: Vec<_> = g
            .fixed_external_boundary
            .iter()
            .map(|fixed| if *fixed { 0. } else { 0.0005 })
            .collect();
        let settings = preparation::Settings {
            erodibility_per_year: 1e-6,
            diffusivity_square_meters_per_year: 1.,
            step_years: 1000.,
            steps: 8,
        };
        let p = preparation::prepare_humid_facets(&g, &h, &u, settings).unwrap();
        let q = preparation::prepare_humid_facets(&g, &h, &u, settings).unwrap();
        assert_eq!(
            serde_json::to_value(&p).unwrap(),
            serde_json::to_value(&q).unwrap()
        );
        for (i, &initial) in h.iter().enumerate() {
            let reconstructed = initial + p.accepted_uplift_displacement_meters[i]
                - p.removed_incision_thickness_meters[i]
                + p.signed_hillslope_height_change_meters[i];
            assert!((p.elevation_meters[i] - reconstructed).abs() < 1e-12);
            if g.fixed_external_boundary[i] {
                assert_eq!(p.elevation_meters[i], initial);
            }
        }
        assert!(p.relative_prism_ledger_residual < 1e-12);
        assert!(p.history.iter().all(|s| s.potential_geometry.is_some()));
    }
    #[test]
    fn zero_forcing_never_projects_potential_lake_surface_into_bed() {
        let g = grid();
        let mut h = vec![10.; 49];
        h[24] = -5.;
        let p = preparation::prepare_humid_facets(
            &g,
            &h,
            &[0.; 49],
            preparation::Settings {
                erodibility_per_year: 0.,
                diffusivity_square_meters_per_year: 0.,
                step_years: 1000.,
                steps: 4,
            },
        )
        .unwrap();
        assert_eq!(p.elevation_meters, h);
        assert_eq!(p.detached_material_reservoir_cubic_meters, 0.);
        assert_eq!(p.final_routing.potential.summary.positive_depth_nodes, 1);
        let mut u = vec![0.; 49];
        u[0] = 1.;
        assert!(
            preparation::prepare_humid_facets(
                &g,
                &h,
                &u,
                preparation::Settings {
                    erodibility_per_year: 1e-6,
                    diffusivity_square_meters_per_year: 1.,
                    step_years: 1000.,
                    steps: 1,
                }
            )
            .is_err()
        );
        assert_eq!(h[24], -5.);
    }
}
