//! Seeded Poisson interior / planar Delaunay chart / spherical physical metric.
//! Bounded physical geometry, independent of camera, climate and water stocks.
use crate::{Random, drainage_landforms::Edge, terrain_patch_geometry::Settings};
use serde::Serialize;
use spade::{DelaunayTriangulation, Point2, Triangulation};
use std::collections::BTreeMap;

pub const MODEL_VERSION: &str = "poisson-spherical-triangle-patch-1";
pub const TRIANGULATOR_VERSION: &str = "spade-2.15.1";
pub const REGULAR_SOURCE_SAMPLING_VERSION: &str = "regular-source-bilinear-1";
pub const MAX_POINTS: usize = 262_144;
pub const MAX_NOMINAL_SIDE: u32 = 256;
pub const CANDIDATES_PER_ACTIVE_POINT: usize = 30;
pub const MINIMUM_INTERIOR_SPACING_FRACTION: f64 = 0.8;
pub const INTERIOR_MARGIN_FRACTION: f64 = 0.65;
type V = [f64; 3];

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Geometry {
    pub model_version: &'static str,
    pub triangulator_version: &'static str,
    pub settings: Settings,
    pub seed: String,
    pub random_version: &'static str,
    pub random_stream: String,
    /// Perimeter first, then the sampled interior in insertion order.
    pub chart_points_meters: Vec<[f64; 2]>,
    pub point_directions: Vec<V>,
    /// Chart-CCW, minimum node first, lexicographically sorted.
    pub triangles: Vec<[u32; 3]>,
    pub edges: Vec<Edge>,
    /// Spherical area thirds, NOT Voronoi cells or regular source cell areas.
    pub reference_areas_square_meters: Vec<f64>,
    pub fixed_external_boundary: Vec<bool>,
    pub minimum_interior_spacing_meters: f64,
    pub interior_margin_meters: f64,
    pub candidates_per_active_point: usize,
    pub total_spherical_area_square_meters: f64,
}

fn uniform(rng: &mut Random) -> f64 {
    f64::from(rng.next_u32()) / 4294967296.
}

/// Independently implemented bounded active-list dart throwing. Work is at
/// most two active-list iterations per accepted point, thirty candidates each.
/// Use dimensionless chart units; changing physical scale does not reroll points.
fn poisson(
    nx: u32,
    ny: u32,
    rng: &mut Random,
    point_budget: usize,
) -> Result<Vec<[f64; 2]>, String> {
    let low = [
        0.5 - f64::from(nx) / 2. + INTERIOR_MARGIN_FRACTION,
        0.5 - f64::from(ny) / 2. + INTERIOR_MARGIN_FRACTION,
    ];
    let high = [
        f64::from(nx) / 2. - 0.5 - INTERIOR_MARGIN_FRACTION,
        f64::from(ny) / 2. - 0.5 - INTERIOR_MARGIN_FRACTION,
    ];
    let radius = MINIMUM_INTERIOR_SPACING_FRACTION;
    let cell = radius / 2_f64.sqrt();
    let width = ((high[0] - low[0]) / cell).ceil() as usize;
    let height = ((high[1] - low[1]) / cell).ceil() as usize;
    let mut bins = vec![usize::MAX; width * height];
    let position = |p: [f64; 2]| {
        [
            ((p[0] - low[0]) / cell) as usize,
            ((p[1] - low[1]) / cell) as usize,
        ]
    };
    let first = [
        low[0] + uniform(rng) * (high[0] - low[0]),
        low[1] + uniform(rng) * (high[1] - low[1]),
    ];
    let mut points = vec![first];
    let [x, y] = position(first);
    bins[y * width + x] = 0;
    let mut active = vec![0_usize];
    while !active.is_empty() {
        let slot = ((u64::from(rng.next_u32()) * active.len() as u64) >> 32) as usize;
        let p = points[active[slot]];
        let mut accepted = false;
        for _ in 0..CANDIDATES_PER_ACTIVE_POINT {
            let distance = radius * (1. + 3. * uniform(rng)).sqrt();
            let (sin, cos) = (std::f64::consts::TAU * uniform(rng)).sin_cos();
            let q = [p[0] + distance * cos, p[1] + distance * sin];
            if q[0] < low[0] || q[1] < low[1] || q[0] >= high[0] || q[1] >= high[1] {
                continue;
            }
            let [x, y] = position(q);
            if bins[y * width + x] != usize::MAX {
                continue;
            }
            let mut valid = true;
            'neighbors: for row in y.saturating_sub(2)..=(y + 2).min(height - 1) {
                for column in x.saturating_sub(2)..=(x + 2).min(width - 1) {
                    let i = bins[row * width + column];
                    if i == usize::MAX {
                        continue;
                    }
                    let dx = q[0] - points[i][0];
                    let dy = q[1] - points[i][1];
                    if dx * dx + dy * dy < radius * radius {
                        valid = false;
                        break 'neighbors;
                    }
                }
            }
            if valid {
                if points.len() >= point_budget {
                    return Err("Poisson patch exceeds its point budget.".into());
                }
                bins[y * width + x] = points.len();
                active.push(points.len());
                points.push(q);
                accepted = true;
                break;
            }
        }
        if !accepted {
            active.swap_remove(slot);
        }
    }
    Ok(points)
}

fn dot(a: V, b: V) -> f64 {
    a.into_iter().zip(b).map(|(x, y)| x * y).sum()
}
fn sub(a: V, b: V) -> V {
    std::array::from_fn(|i| a[i] - b[i])
}
fn cross(a: V, b: V) -> V {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}
fn unit(p: V) -> V {
    let length = dot(p, p).sqrt();
    p.map(|v| v / length)
}
fn spherical_area(a: V, b: V, c: V, radius: f64) -> f64 {
    2. * dot(a, cross(sub(b, a), sub(c, a)))
        .abs()
        .atan2(1. + dot(a, b) + dot(b, c) + dot(c, a))
        * radius
        * radius
}

pub fn build(seed: &str, settings: Settings) -> Result<Geometry, String> {
    settings.validate()?;
    if seed.trim().is_empty()
        || seed.encode_utf16().count() > 128
        || settings.cells_x > MAX_NOMINAL_SIDE
        || settings.cells_y > MAX_NOMINAL_SIDE
    {
        return Err("Unsupported irregular patch seed or nominal dimensions.".into());
    }
    // Mesh sampling is independent of geological/seed-field streams. Location
    // distinguishes patches; orientation, spacing and radius do not reroll it.
    let stream = serde_json::to_string(&(
        MODEL_VERSION,
        "interior",
        settings.center_direction,
        settings.cells_x,
        settings.cells_y,
    ))
    .map_err(|e| e.to_string())?;
    let mut rng = Random::stream(seed, &stream);
    let mut points = Vec::new();
    for row in 0..settings.cells_y {
        for column in 0..settings.cells_x {
            if row == 0
                || column == 0
                || row + 1 == settings.cells_y
                || column + 1 == settings.cells_x
            {
                points.push([
                    (f64::from(column) + 0.5 - f64::from(settings.cells_x) / 2.)
                        * settings.spacing_meters,
                    (f64::from(row) + 0.5 - f64::from(settings.cells_y) / 2.)
                        * settings.spacing_meters,
                ]);
            }
        }
    }
    let boundary_count = points.len();
    points.extend(
        poisson(
            settings.cells_x,
            settings.cells_y,
            &mut rng,
            MAX_POINTS - boundary_count,
        )?
        .into_iter()
        .map(|p| p.map(|x| x * settings.spacing_meters)),
    );
    let fixed: Vec<_> = (0..points.len()).map(|i| i < boundary_count).collect();
    let triangulation = DelaunayTriangulation::<Point2<f64>>::bulk_load_stable(
        points.iter().map(|p| Point2::new(p[0], p[1])).collect(),
    )
    .map_err(|e| format!("Patch triangulation refused: {e:?}"))?;
    if triangulation.num_vertices() != points.len()
        || triangulation
            .vertices()
            .enumerate()
            .any(|(i, v)| v.position() != Point2::new(points[i][0], points[i][1]))
    {
        return Err("Triangulation changed point identity or removed duplicates.".into());
    }
    let mut triangles: Vec<[u32; 3]> = triangulation
        .inner_faces()
        .map(|f| {
            let mut t = f.vertices().map(|v| v.index() as u32);
            let [a, b, c] = t.map(|i| points[i as usize]);
            if (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0]) < 0. {
                t.swap(1, 2);
            }
            let first = t.iter().enumerate().min_by_key(|(_, v)| *v).unwrap().0;
            t.rotate_left(first);
            t
        })
        .collect();
    triangles.sort_unstable();
    if triangles.len() != 2 * points.len() - 2 - boundary_count
        || triangles.windows(2).any(|v| v[0] == v[1])
    {
        return Err("Invalid irregular patch face count/identity.".into());
    }
    let center = unit(settings.center_direction);
    let anchor = if center[1].abs() > 0.9 {
        [0., 0., 1.]
    } else {
        [0., 1., 0.]
    };
    let east = unit(cross(center, anchor));
    let north = cross(east, center);
    let (sin, cos) = settings.orientation_radians.sin_cos();
    let east: V = std::array::from_fn(|i| east[i] * cos + north[i] * sin);
    // Construct the second rotated axis from the already rotated east vector.
    let north = cross(east, center);
    let directions: Vec<_> = points
        .iter()
        .map(|p| {
            unit(std::array::from_fn(|i| {
                center[i]
                    + east[i] * p[0] / settings.radius_meters
                    + north[i] * p[1] / settings.radius_meters
            }))
        })
        .collect();
    if directions
        .iter()
        .any(|p| p.iter().any(|v| !v.is_finite()) || (dot(*p, *p) - 1.).abs() > 1e-12)
    {
        return Err("Invalid irregular patch unit directions.".into());
    }
    let mut reference_areas = vec![0.; points.len()];
    let mut pairs = BTreeMap::<(u32, u32), usize>::new();
    let mut total_area = 0.;
    for &t in &triangles {
        let [a, b, c] = t.map(|i| directions[i as usize]);
        let area = spherical_area(a, b, c, settings.radius_meters);
        if !area.is_finite() || area <= 0. {
            return Err("Nonpositive irregular patch triangle area.".into());
        }
        total_area += area;
        for i in t {
            reference_areas[i as usize] += area / 3.;
        }
        for (a, b) in [(t[0], t[1]), (t[1], t[2]), (t[2], t[0])] {
            *pairs.entry((a.min(b), a.max(b))).or_default() += 1;
        }
    }
    if pairs.len() != 3 * points.len() - 3 - boundary_count
        || pairs.iter().any(|(&(a, b), &count)| {
            !(1..=2).contains(&count) || count == 1 && (!fixed[a as usize] || !fixed[b as usize])
        })
        || pairs.values().filter(|&&c| c == 1).count() != boundary_count
        || reference_areas.iter().any(|a| !a.is_finite() || *a <= 0.)
        || !total_area.is_finite()
    {
        return Err("Invalid irregular patch incidence or reference area.".into());
    }
    let mut edges = Vec::with_capacity(pairs.len());
    for ((a, b), _) in pairs {
        let p = directions[a as usize];
        let q = directions[b as usize];
        let v = cross(p, sub(q, p));
        let distance = dot(v, v).sqrt().atan2(dot(p, q)) * settings.radius_meters;
        if !distance.is_finite() || distance <= 0. {
            return Err("Nonpositive irregular patch physical edge.".into());
        }
        edges.push(Edge {
            a,
            b,
            distance_meters: distance,
        });
    }
    Ok(Geometry {
        model_version: MODEL_VERSION,
        triangulator_version: TRIANGULATOR_VERSION,
        settings,
        seed: seed.into(),
        random_version: "fnv1a-utf8-mulberry32-1",
        random_stream: stream,
        chart_points_meters: points,
        point_directions: directions,
        triangles,
        edges,
        reference_areas_square_meters: reference_areas,
        fixed_external_boundary: fixed,
        minimum_interior_spacing_meters: MINIMUM_INTERIOR_SPACING_FRACTION
            * settings.spacing_meters,
        interior_margin_meters: INTERIOR_MARGIN_FRACTION * settings.spacing_meters,
        candidates_per_active_point: CANDIDATES_PER_ACTIVE_POINT,
        total_spherical_area_square_meters: total_area,
    })
}

/// A declared continuous source interpolant, not conservative stock transfer or
/// new resolved source detail. Queries must lie inside the source centers hull.
pub fn sample_regular_source(
    settings: Settings,
    bed: &[f64],
    queries: &[[f64; 2]],
) -> Result<Vec<f64>, String> {
    settings.validate()?;
    let nx = settings.cells_x as usize;
    let ny = settings.cells_y as usize;
    if bed.len() != nx * ny || queries.len() > MAX_POINTS || bed.iter().any(|v| !v.is_finite()) {
        return Err("Invalid regular source interpolation workload or bed.".into());
    }
    let low = [
        (0.5 - nx as f64 / 2.) * settings.spacing_meters,
        (0.5 - ny as f64 / 2.) * settings.spacing_meters,
    ];
    let high = [
        (nx as f64 / 2. - 0.5) * settings.spacing_meters,
        (ny as f64 / 2. - 0.5) * settings.spacing_meters,
    ];
    queries
        .iter()
        .map(|p| {
            if p.iter().any(|v| !v.is_finite())
                || p[0] < low[0]
                || p[0] > high[0]
                || p[1] < low[1]
                || p[1] > high[1]
            {
                return Err("Source query is outside the retained cell-center hull.".into());
            }
            let u = if p[0] == high[0] {
                (nx - 1) as f64
            } else {
                (p[0] - low[0]) / settings.spacing_meters
            };
            let v = if p[1] == high[1] {
                (ny - 1) as f64
            } else {
                (p[1] - low[1]) / settings.spacing_meters
            };
            let x = (u.floor() as usize).min(nx - 2);
            let y = (v.floor() as usize).min(ny - 2);
            let tx = u - x as f64;
            let ty = v - y as f64;
            if !(0. ..=1.).contains(&tx) || !(0. ..=1.).contains(&ty) {
                return Err("Unrepresentable source interpolation coordinates.".into());
            }
            // Difference-form interpolation preserves constant fields exactly.
            let bottom = bed[y * nx + x] + tx * (bed[y * nx + x + 1] - bed[y * nx + x]);
            let top =
                bed[(y + 1) * nx + x] + tx * (bed[(y + 1) * nx + x + 1] - bed[(y + 1) * nx + x]);
            let result = bottom + ty * (top - bottom);
            if !result.is_finite() {
                return Err("Nonfinite regular source interpolant.".into());
            }
            Ok(result)
        })
        .collect()
}
