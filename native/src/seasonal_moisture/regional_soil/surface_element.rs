//! Positive triangle coefficient integration; standalone flat-land verification.
//! No evolving front qualification, seasonal selector or persisted state.
use super::{surface, surface_centered, surface_gradient};

pub(super) const MODEL_VERSION: &str = "regional-surface-flat-element-mobility-candidate-1";

// Positive interior six-point rule, exact through degree four. Weights describe
// triangle averages (sum one), not reference-triangle integrals (area 1/2).
// Source and independent certificates: docs/surface-element-mobility.md.
const ORBITS: [(f64, f64); 2] = [
    (0.816847572980459, 0.109951743655322),
    (0.108103018168070, 0.223381589678011),
];

// Bounded convex arithmetic, not clipping. Equal inputs return unchanged.
pub(super) fn mix(a: f64, b: f64, fraction: f64) -> f64 {
    if a == b {
        return a;
    }
    let (lo, hi, t) = if a < b {
        (a, b, fraction)
    } else {
        (b, a, 1. - fraction)
    };
    if t <= 0.5 {
        lo + t * (hi - lo)
    } else {
        hi - (1. - t) * (hi - lo)
    }
}

fn average(values: &[f64]) -> f64 {
    values
        .iter()
        .enumerate()
        .skip(1)
        .fold(values[0], |a, (i, &b)| mix(a, b, 1. / (i + 1) as f64))
}

fn cubature(depth: [f64; 3], f: &impl Fn(f64) -> Result<f64, String>) -> Result<f64, String> {
    let mut means = [0.; 2];
    for (orbit, &(a, _)) in ORBITS.iter().enumerate() {
        let mut values = [0.; 3];
        for i in 0..3 {
            let others = mix(depth[(i + 1) % 3], depth[(i + 2) % 3], 0.5);
            values[i] = f(mix(others, depth[i], a))?;
        }
        means[orbit] = average(&values);
    }
    Ok(mix(
        means[0],
        means[1],
        ORBITS[1].1 / (ORBITS[0].1 + ORBITS[1].1),
    ))
}

fn integrate(
    depth: [f64; 3],
    subdivisions: u8,
    f: &impl Fn(f64) -> Result<f64, String>,
) -> Result<f64, String> {
    if subdivisions == 0 {
        return cubature(depth, f);
    }
    let [a, b, c] = depth;
    let ab = mix(a, b, 0.5);
    let bc = mix(b, c, 0.5);
    let ca = mix(c, a, 0.5);
    let values = [[a, ab, ca], [ab, b, bc], [ca, bc, c], [ab, bc, ca]]
        .map(|d| integrate(d, subdivisions - 1, f))
        .into_iter()
        .collect::<Result<Vec<_>, _>>()?;
    // Midpoint subdivision gives four equal-area chordal subtriangles.
    Ok(average(&values))
}

pub(super) fn mobility(
    mut depth: [f64; 3],
    slope: f64,
    roughness: f64,
    cap: f64,
    subdivisions: u8,
) -> Result<f64, String> {
    if depth.iter().any(|h| !h.is_finite() || *h < 0.)
        || !slope.is_finite()
        || slope < 0.
        || !roughness.is_finite()
        || roughness <= 0.
        || !cap.is_finite()
        || cap < 0.
        || subdivisions > 3
    {
        return Err("Invalid bounded element-mobility settings.".into());
    }
    // Canonical ordering makes the scalar integral exactly permutation invariant.
    depth.sort_by(f64::total_cmp);
    if depth[2] == 0. || cap == 0. {
        return Ok(0.);
    }
    if slope == 0. {
        // Positive affine depth almost everywhere; its zero boundary has no area.
        return Ok(cap);
    }
    let denominator = roughness * slope.sqrt();
    if !denominator.is_finite() || denominator <= 0. {
        return Err("Unrepresentable element-mobility denominator.".into());
    }
    let scalar = |h: f64| {
        let numerator = h.powf(5. / 3.);
        if !numerator.is_finite() {
            return Err("Unrepresentable element-mobility numerator.".into());
        }
        Ok((numerator / denominator).min(cap))
    };
    // Monotone depth law certifies a truly capped whole triangle, not just nodes.
    if depth[0] == depth[2] || scalar(depth[0])? == cap {
        return scalar(depth[0]);
    }
    let maximum_numerator = depth[2].powf(5. / 3.);
    if !maximum_numerator.is_finite() {
        return Err("Unrepresentable element-mobility numerator.".into());
    }
    let value = if maximum_numerator / denominator <= cap {
        integrate(depth, subdivisions, &scalar)?
    } else {
        // Split at the affine depth where the cap begins. A fixed node set can
        // otherwise miss an arbitrarily small uncapped corner and incorrectly
        // appear quadrature-converged. Capped pieces are integrated exactly.
        let threshold = (cap * denominator).powf(3. / 5.);
        let [a, b, c] = depth;
        if !threshold.is_finite() || threshold <= a || threshold >= c {
            return Err("Unrepresentable element cap intersection.".into());
        }
        let ac = (threshold - a) / (c - a);
        if threshold <= b {
            let ab = (threshold - a) / (b - a);
            mix(
                cap,
                integrate([a, threshold, threshold], subdivisions, &scalar)?,
                ab * ac,
            )
        } else {
            let bc = (threshold - b) / (c - b);
            let corner = integrate([a, threshold, threshold], subdivisions, &scalar)?;
            let first = integrate([a, b, threshold], subdivisions, &scalar)?;
            // Two uncapped triangles and one capped triangle; nested convex
            // means use their geometric area fractions without renormalization.
            mix(mix(cap, corner, ac), first, bc)
        }
    };
    if !value.is_finite() || value < 0. || value > cap {
        return Err("Unrepresentable element-mobility average.".into());
    }
    Ok(value)
}

pub(super) struct Prepared<'a> {
    gradient: &'a surface_gradient::Prepared,
    flat: surface_centered::Prepared<'a>,
    subdivisions: u8,
}

pub(super) fn local_controls() -> Result<Vec<serde_json::Value>, String> {
    let mut controls = Vec::new();
    let p = 5. / 3.;
    for h in [0.001_f64, 1., 1000.] {
        let scale = h.powf(p);
        for cap_ratio in [1e-6_f64, 0.2, 2.] {
            let cap = cap_ratio * scale;
            let s = cap_ratio.powf(1. / p).min(1.);
            let exact_one = 2. * scale * (s.powf(p + 1.) / (p + 1.) - s.powf(p + 2.) / (p + 2.))
                + cap * (1. - s).powi(2);
            let exact_two = 2. * scale * s.powf(p + 2.) / (p + 2.) + cap * (1. - s * s);
            for (depth, exact) in [([0., 0., h], exact_one), ([0., h, h], exact_two)] {
                for q in 0..=3 {
                    let value = mobility(depth, 1., 1., cap, q)?;
                    controls.push(serde_json::json!({"depthsMeters":depth,"slope":1.,"roughness":1.,"cap":cap,
                        "quadratureSubdivisions":q,"maximumNodesPerTriangle":12 * 4_usize.pow(q as u32),
                        "exactTriangleMean":exact,"computedTriangleMean":value,"signedRelativeError":value/exact-1.}));
                }
            }
        }
    }
    Ok(controls)
}

impl<'a> Prepared<'a> {
    pub fn new(gradient: &'a surface_gradient::Prepared, subdivisions: u8) -> Result<Self, String> {
        let flat = surface_centered::Prepared::new(gradient)?;
        if subdivisions > 3 || !gradient.has_positive_local_weights() {
            return Err(
                "Element mobility requires bounded quadrature and positive local weights.".into(),
            );
        }
        Ok(Self {
            gradient,
            flat,
            subdivisions,
        })
    }

    pub fn advance(&self, stocks: surface::Stocks<'_>, seconds: f64) -> Result<usize, String> {
        self.flat.validate(&stocks, seconds)?;
        surface::advance_flat_element(self.gradient, self.subdivisions, stocks, seconds)
    }
}

#[cfg(test)]
mod tests {
    use super::super::{Mass, ResolutionBudget, surface_flow, surface_verification};
    use super::*;
    use crate::Surface;

    fn geometry(cap: f64, datum: f64) -> surface_gradient::Prepared {
        let mesh = Surface::build(1, 1000.);
        let layout = surface_flow::Layout::from_fields(
            &mesh,
            1000.,
            vec![datum; 42],
            vec![true; 42],
            0.,
            surface_flow::Settings {
                roughness: 0.04,
                maximum_diffusivity_square_meters_per_second: cap,
            },
        )
        .unwrap();
        surface_gradient::Prepared::new(&mesh, layout, 1000.).unwrap()
    }

    fn step(
        g: &surface_gradient::Prepared,
        initial: &[Mass],
        seconds: f64,
    ) -> (Vec<Mass>, Vec<Mass>, Option<ResolutionBudget>) {
        let mut liquid = initial.to_vec();
        let mut transfers = vec![Mass::default(); 2 * g.layout().faces.len()];
        let mut resolution = Some(ResolutionBudget::default());
        assert_eq!(
            Prepared::new(g, 1)
                .unwrap()
                .advance(
                    surface::Stocks {
                        by_region: &vec![None; 42],
                        liquid: &mut liquid,
                        bodies: &mut [],
                        transfers: &mut transfers,
                        resolution: &mut resolution
                    },
                    seconds
                )
                .unwrap(),
            1
        );
        let residual =
            surface_verification::audit(g.layout(), initial, &liquid, &transfers).unwrap();
        assert!(residual.into_iter().all(|r| r < 1e-12));
        (liquid, transfers, resolution)
    }

    #[test]
    fn actual_dry_arrivals_are_frozen_conservative_and_stock_bounded() {
        let g = geometry(1000., 0.);
        let mut pool = vec![Mass::default(); 42];
        pool[0].high = 1000. * g.layout().areas[0];
        let a = step(&g, &pool, g.layout().stable_seconds);
        assert_eq!(a, step(&g, &pool, g.layout().stable_seconds));
        assert_eq!(
            a,
            step(&geometry(1000., 1e15), &pool, g.layout().stable_seconds)
        );
        assert!(a.0[0].high >= 0.55 * pool[0].high);
        let neighbors: Vec<_> = g
            .layout()
            .faces
            .iter()
            .filter(|f| f.regions.contains(&0))
            .map(|f| f.regions[0] + f.regions[1])
            .collect();
        assert!(neighbors.iter().all(|&i| a.0[i].high > 0.));
        assert!(
            (1..42)
                .filter(|i| !neighbors.contains(i))
                .all(|i| a.0[i] == Mass::default())
        );
        let zero = geometry(0., 0.);
        let off = step(&zero, &pool, 0.01);
        assert_eq!(off.0, pool);
        assert!(off.1.iter().all(|m| *m == Mass::default()));
    }

    #[test]
    fn fully_wet_capped_transfers_and_exact_rest_preserve_old_arithmetic() {
        let g = geometry(1e-6, 0.);
        let initial: Vec<_> = g
            .layout()
            .areas
            .iter()
            .map(|&a| Mass {
                high: 1000. * a,
                low: 0.,
            })
            .collect();
        let rest = step(&g, &initial, 0.01);
        assert_eq!(rest.0, initial);
        assert!(rest.1.iter().all(|m| *m == Mass::default()));
        let mut wet = initial;
        wet[0].high *= 2.;
        let element = step(&g, &wet, 0.01);
        let mut liquid = wet.clone();
        let mut transfers = vec![Mass::default(); 2 * g.layout().faces.len()];
        let mut resolution = Some(ResolutionBudget::default());
        surface_centered::Prepared::new(&g)
            .unwrap()
            .advance(
                surface::Stocks {
                    by_region: &vec![None; 42],
                    liquid: &mut liquid,
                    bodies: &mut [],
                    transfers: &mut transfers,
                    resolution: &mut resolution,
                },
                0.01,
            )
            .unwrap();
        assert_eq!(element, (liquid, transfers, resolution));
    }

    #[test]
    fn unsupported_geometry_and_malformed_calls_refuse_without_mutation() {
        let mut g = geometry(1000., 0.);
        assert!(Prepared::new(&g, 4).is_err());
        for case in 0..10 {
            let mut liquid = vec![Mass::default(); 42];
            let mut transfers = vec![Mass::default(); 2 * g.layout().faces.len()];
            let mut by_region = vec![None; 42];
            let mut bodies = vec![];
            let mut resolution = Some(ResolutionBudget::default());
            let mut seconds = 0.01;
            match case {
                0 => seconds = -1.,
                1 => seconds = f64::NAN,
                2 => seconds = g.layout().stable_seconds * 16385.,
                3 => {
                    by_region.pop();
                }
                4 => by_region[0] = Some(0),
                5 => bodies.push(Mass::default()),
                6 => {
                    liquid.pop();
                }
                7 => {
                    transfers.pop();
                }
                8 => transfers[0].high = -1.,
                _ => liquid[0] = Mass { high: 0., low: 1. },
            }
            let before = (
                liquid.clone(),
                transfers.clone(),
                bodies.clone(),
                resolution,
            );
            assert!(
                Prepared::new(&g, 1)
                    .unwrap()
                    .advance(
                        surface::Stocks {
                            by_region: &by_region,
                            liquid: &mut liquid,
                            bodies: &mut bodies,
                            transfers: &mut transfers,
                            resolution: &mut resolution
                        },
                        seconds
                    )
                    .is_err()
            );
            assert_eq!((liquid, transfers, bodies, resolution), before);
        }
        // Exercise the actual public preparation paths rather than changing
        // physical data through a renderer or a prepared immutable layout.
        let mesh = Surface::build(1, 1000.);
        let mut beds = vec![0.; 42];
        beds[1] = 1.;
        for (beds, land) in [(beds, vec![true; 42]), (vec![0.; 42], vec![false; 42])] {
            g = surface_gradient::Prepared::new(
                &mesh,
                surface_flow::Layout::from_fields(&mesh, 1000., beds, land, 0., Default::default())
                    .unwrap(),
                1000.,
            )
            .unwrap();
            assert!(Prepared::new(&g, 1).is_err());
        }
    }

    #[test]
    fn positive_rule_certifies_independent_polynomial_moments() {
        // For h=lambda_0, the normalized triangle moment is 2/((k+1)(k+2)).
        for k in 0..=4 {
            let exact = 2. / ((k + 1) * (k + 2)) as f64;
            for subdivisions in 0..=3 {
                let value = integrate([1., 0., 0.], subdivisions, &|h| Ok(h.powi(k))).unwrap();
                assert!(
                    (value - exact).abs() < 2e-15,
                    "degree {k}: {value} != {exact}"
                );
            }
        }
        assert!(ORBITS.iter().all(|&(a, w)| a > 0. && a < 1. && w > 0.));
    }

    #[test]
    fn dry_corner_power_integrals_refine_against_exact_formulas() {
        for (depth, exact) in [([0., 0., 1.], 9. / 44.), ([0., 1., 1.], 6. / 11.)] {
            let coarse = mobility(depth, 1., 1., 2., 0).unwrap();
            let fine = mobility(depth, 1., 1., 2., 3).unwrap();
            assert!((fine - exact).abs() < (coarse - exact).abs());
            assert!((fine - exact).abs() < 3e-6, "{depth:?}: {fine} != {exact}");
            for d in [
                depth,
                [depth[1], depth[2], depth[0]],
                [depth[2], depth[1], depth[0]],
            ] {
                assert_eq!(fine, mobility(d, 1., 1., 2., 3).unwrap());
            }
        }
    }

    #[test]
    fn capped_corner_integrals_are_not_the_cap_of_the_average() {
        let p = 5. / 3.;
        let cap: f64 = 0.2;
        let s = cap.powf(1. / p);
        let one =
            2. * (s.powf(p + 1.) / (p + 1.) - s.powf(p + 2.) / (p + 2.)) + cap * (1. - s).powi(2);
        let two = 2. * s.powf(p + 2.) / (p + 2.) + cap * (1. - s * s);
        for (depth, exact) in [([0., 0., 1.], one), ([0., 1., 1.], two)] {
            let a = mobility(depth, 1., 1., cap, 0).unwrap();
            let b = mobility(depth, 1., 1., cap, 3).unwrap();
            assert!(b > 0. && b < cap);
            assert!((b - exact).abs() < (a - exact).abs());
            assert!((b - exact).abs() < 1e-4, "{b} != {exact}");
        }
    }

    #[test]
    fn cap_intersection_resolves_a_corner_missed_by_all_unsplit_nodes() {
        let cap: f64 = 1e-6;
        let s = cap.powf(3. / 5.);
        let p = 5. / 3.;
        let exact =
            2. * (s.powf(p + 1.) / (p + 1.) - s.powf(p + 2.) / (p + 2.)) + cap * (1. - s).powi(2);
        let raw = integrate([0., 0., 1.], 3, &|h| Ok(h.powf(p).min(cap))).unwrap();
        assert_eq!(raw, cap);
        let split = mobility([0., 0., 1.], 1., 1., cap, 3).unwrap();
        assert!(split < cap);
        assert!((split - exact).abs() / exact < 1e-7);
        // Crossing the other side of the middle-depth vertex exercises the
        // quadrilateral decomposition and its independent exact two-wet case.
        let cap: f64 = 0.7;
        let s = cap.powf(3. / 5.);
        let exact = 2. * s.powf(p + 2.) / (p + 2.) + cap * (1. - s * s);
        let split = mobility([0., 1., 1.], 1., 1., cap, 3).unwrap();
        assert!((split - exact).abs() / exact < 1e-5);
    }

    #[test]
    fn both_cap_split_geometries_match_independent_affine_height_density() {
        let [a, b, c] = [0.1_f64, 0.4, 1.];
        let p = 5. / 3.;
        for cap in [0.1_f64, b.powf(p), 0.7, 1.] {
            let threshold = cap.powf(1. / p);
            let mut exact = 0.;
            for (lo, hi, left) in [(a, b, true), (b, c, false)] {
                let end = threshold.min(hi).max(lo);
                let divisor = if left {
                    (b - a) * (c - a)
                } else {
                    (c - a) * (c - b)
                };
                let primitive = |h: f64| {
                    if left {
                        h.powf(p + 2.) / (p + 2.) - a * h.powf(p + 1.) / (p + 1.)
                    } else {
                        c * h.powf(p + 1.) / (p + 1.) - h.powf(p + 2.) / (p + 2.)
                    }
                };
                let cap_primitive = |h: f64| {
                    if left {
                        (h - a).powi(2) / 2.
                    } else {
                        -(c - h).powi(2) / 2.
                    }
                };
                exact += 2. / divisor
                    * (primitive(end) - primitive(lo)
                        + cap * (cap_primitive(hi) - cap_primitive(end)));
            }
            let value = mobility([a, b, c], 1., 1., cap, 3).unwrap();
            assert!(
                (value / exact - 1.).abs() < 1e-7,
                "cap {cap}: {value} != {exact}"
            );
        }
    }

    #[test]
    fn dry_uniform_cap_and_near_equal_limits_are_explicit() {
        for q in 0..=3 {
            assert_eq!(mobility([0.; 3], 0., 1., 7., q).unwrap(), 0.);
            assert_eq!(mobility([1.; 3], 0., 1., 7., q).unwrap(), 7.);
            assert_eq!(mobility([0., 0., 1.], 0., 1., 7., q).unwrap(), 7.);
            assert_eq!(mobility([1., 2., 3.], 1., 1., 0., q).unwrap(), 0.);
            assert_eq!(mobility([1., 2., 3.], 1., 1., 0.1, q).unwrap(), 0.1);
            let h = [1., 1. + f64::EPSILON, 1. + 2. * f64::EPSILON];
            let a = mobility(h, 1., 1., 7., q).unwrap();
            assert!(a >= 1. && a <= h[2].powf(5. / 3.));
        }
    }

    #[test]
    fn bounded_kernel_refuses_invalid_or_unrepresentable_inputs() {
        for (h, g, n, cap, q) in [
            ([-1., 0., 1.], 1., 1., 1., 0),
            ([f64::NAN, 0., 1.], 1., 1., 1., 0),
            ([1.; 3], -1., 1., 1., 0),
            ([1.; 3], f64::INFINITY, 1., 1., 0),
            ([1.; 3], 1., 0., 1., 0),
            ([1.; 3], 1., 1., -1., 0),
            ([1.; 3], 1., 1., f64::NAN, 0),
            ([1.; 3], 1., 1., 1., 4),
            ([f64::MAX; 3], 1., 1., 1., 0),
            ([1.; 3], f64::MAX, f64::MAX, 1., 0),
        ] {
            assert!(mobility(h, g, n, cap, q).is_err());
        }
    }
}
