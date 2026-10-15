//! Bounded gnomonic quadrilateral patch on a reference sphere, not a closed
//! World or Surface. Side transmissivities are a small-chart two-point
//! approximation, not an exact nonorthogonal Laplace-Beltrami discretization.
use crate::hillslope_transport::Edge;
use serde::{Deserialize, Serialize};

type V = [f64; 3];
pub const MODEL_VERSION: &str = "fixed-spherical-patch-1";
pub const MAX_CELLS: usize = 262_144;
pub const MAX_EXTENT_RADIUS_RATIO: f64 = 0.05;

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    pub radius_meters: f64,
    /// Accepted unit input is normalized internally. Retain the caller's
    /// validated setting so rebuilding repeats that exact construction.
    pub center_direction: V,
    pub orientation_radians: f64,
    pub cells_x: u32,
    pub cells_y: u32,
    /// Spacing in the gnomonic chart, not a constant spherical edge length.
    pub spacing_meters: f64,
}
impl Settings {
    /// Refuse invalid dimensions/extents before allocating geometry arrays.
    pub fn validate(&self) -> Result<(), String> {
        if !self.radius_meters.is_finite()
            || !(100_000. ..=20_000_000.).contains(&self.radius_meters)
            || self.center_direction.iter().any(|v| !v.is_finite())
            || (dot(self.center_direction, self.center_direction) - 1.).abs() > 1e-12
            || !self.orientation_radians.is_finite()
            || !(0. ..std::f64::consts::TAU).contains(&self.orientation_radians)
            || !(4..=512).contains(&self.cells_x)
            || !(4..=512).contains(&self.cells_y)
            || self.cells_x as usize * self.cells_y as usize > MAX_CELLS
            || !self.spacing_meters.is_finite()
            || !(10. ..=5000.).contains(&self.spacing_meters)
            || f64::from(self.cells_x.max(self.cells_y)) * self.spacing_meters / self.radius_meters
                > MAX_EXTENT_RADIUS_RATIO
        {
            return Err("Invalid or excessive spherical terrain-patch settings.".into());
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Geometry {
    pub model_version: &'static str,
    pub settings: Settings,
    /// Row-major (cells_x+1)×(cells_y+1) shared vertices.
    pub vertex_directions: Vec<V>,
    /// Row-major cell centers at half-integer chart coordinates.
    pub cell_directions: Vec<V>,
    pub reference_areas_square_meters: Vec<f64>,
    /// Unique canonical side-sharing faces, in cell/east/north order.
    pub side_faces: Vec<Edge>,
    /// Eight-neighbor routing geometry; diagonals are not diffusion faces.
    pub offsets: Vec<u32>,
    pub neighbors: Vec<u32>,
    pub distances_meters: Vec<f64>,
    /// The owning physical experiment must declare external boundary behavior.
    /// This geometric perimeter is not a water, lake, ocean or discharge mask.
    pub fixed_external_boundary: Vec<bool>,
}

fn dot(a: V, b: V) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
fn cross(a: V, b: V) -> V {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}
fn difference(a: V, b: V) -> V {
    std::array::from_fn(|i| a[i] - b[i])
}
fn unit(p: V) -> Result<V, String> {
    let norm = dot(p, p).sqrt();
    if !norm.is_finite() || norm <= 0. {
        return Err("Unrepresentable spherical terrain-patch direction.".into());
    }
    let p = p.map(|v| v / norm);
    if p.iter().any(|v| !v.is_finite()) || (dot(p, p) - 1.).abs() > 1e-12 {
        return Err("Invalid computed spherical terrain-patch direction.".into());
    }
    Ok(p)
}
fn spherical_length(a: V, b: V, radius: f64) -> Result<f64, String> {
    // cross(a,b-a) avoids cancellation between two nearly equal O(1) products.
    let c = cross(a, difference(b, a));
    let length = dot(c, c).sqrt().atan2(dot(a, b)) * radius;
    if !length.is_finite() || length <= 0. {
        return Err("Unrepresentable positive spherical terrain-patch length.".into());
    }
    Ok(length)
}
fn triangle_area(a: V, b: V, c: V) -> f64 {
    // Equivalent to the triple product, but stable for a tiny spherical cell.
    let numerator = dot(a, cross(difference(b, a), difference(c, a))).abs();
    2. * numerator.atan2(1. + dot(a, b) + dot(b, c) + dot(c, a))
}

/// Pure geometry. Anchor global north (Y), switching to Z near the poles;
/// record orientation rather than choosing it from a camera or random stream.
pub fn build(settings: Settings) -> Result<Geometry, String> {
    settings.validate()?;
    let nx = settings.cells_x as usize;
    let ny = settings.cells_y as usize;
    let n = nx * ny;
    let center = unit(settings.center_direction)?;
    let anchor = if center[1].abs() > 0.9 {
        [0., 0., 1.]
    } else {
        [0., 1., 0.]
    };
    let east = unit(cross(center, anchor))?;
    let north = cross(east, center);
    let (sin, cos) = settings.orientation_radians.sin_cos();
    let rotated_east = std::array::from_fn::<_, 3, _>(|i| east[i] * cos + north[i] * sin);
    let rotated_north = std::array::from_fn::<_, 3, _>(|i| north[i] * cos - east[i] * sin);
    let sample = |column: f64, row: f64| {
        let x = (column - nx as f64 * 0.5) * settings.spacing_meters / settings.radius_meters;
        let y = (row - ny as f64 * 0.5) * settings.spacing_meters / settings.radius_meters;
        unit(std::array::from_fn(|i| {
            center[i] + rotated_east[i] * x + rotated_north[i] * y
        }))
    };
    let mut vertices = Vec::with_capacity((nx + 1) * (ny + 1));
    for row in 0..=ny {
        for column in 0..=nx {
            vertices.push(sample(column as f64, row as f64)?);
        }
    }
    let mut centers = Vec::with_capacity(n);
    let mut areas = Vec::with_capacity(n);
    let mut fixed = Vec::with_capacity(n);
    let area_scale = settings.radius_meters * settings.radius_meters;
    for row in 0..ny {
        for column in 0..nx {
            centers.push(sample(column as f64 + 0.5, row as f64 + 0.5)?);
            let a = vertices[row * (nx + 1) + column];
            let b = vertices[row * (nx + 1) + column + 1];
            let c = vertices[(row + 1) * (nx + 1) + column + 1];
            let d = vertices[(row + 1) * (nx + 1) + column];
            let area = (triangle_area(a, b, c) + triangle_area(a, c, d)) * area_scale;
            if !area.is_finite() || area <= 0. {
                return Err("Unrepresentable positive spherical terrain-patch area.".into());
            }
            areas.push(area);
            fixed.push(row == 0 || column == 0 || row + 1 == ny || column + 1 == nx);
        }
    }

    let routing_count = 8 * n - 6 * nx - 6 * ny + 4;
    let mut offsets = Vec::with_capacity(n + 1);
    let mut neighbors = Vec::with_capacity(routing_count);
    for row in 0..ny {
        for column in 0..nx {
            offsets.push(neighbors.len() as u32);
            for other_row in row.saturating_sub(1)..=(row + 1).min(ny - 1) {
                for other_column in column.saturating_sub(1)..=(column + 1).min(nx - 1) {
                    if row != other_row || column != other_column {
                        neighbors.push((other_row * nx + other_column) as u32);
                    }
                }
            }
        }
    }
    offsets.push(neighbors.len() as u32);
    let mut distances = vec![0.; neighbors.len()];
    for i in 0..n {
        for k in offsets[i] as usize..offsets[i + 1] as usize {
            let j = neighbors[k] as usize;
            if i < j {
                let distance = spherical_length(centers[i], centers[j], settings.radius_meters)?;
                let start = offsets[j] as usize;
                let position = neighbors[start..offsets[j + 1] as usize]
                    .binary_search(&(i as u32))
                    .map_err(|_| "Nonreciprocal computed terrain-patch routing graph.")?;
                distances[k] = distance;
                distances[start + position] = distance;
            }
        }
    }
    let mut faces = Vec::with_capacity(ny * (nx - 1) + nx * (ny - 1));
    for row in 0..ny {
        for column in 0..nx {
            let i = row * nx + column;
            for (j, endpoints) in [
                (
                    (column + 1 < nx).then_some(i + 1),
                    (
                        row * (nx + 1) + column + 1,
                        (row + 1) * (nx + 1) + column + 1,
                    ),
                ),
                (
                    (row + 1 < ny).then_some(i + nx),
                    (
                        (row + 1) * (nx + 1) + column,
                        (row + 1) * (nx + 1) + column + 1,
                    ),
                ),
            ] {
                let Some(j) = j else { continue };
                let start = offsets[i] as usize;
                let position = neighbors[start..offsets[i + 1] as usize]
                    .binary_search(&(j as u32))
                    .map_err(|_| "Missing computed terrain-patch side neighbor.")?;
                faces.push(Edge {
                    a: i as u32,
                    b: j as u32,
                    shared_face_length_meters: spherical_length(
                        vertices[endpoints.0],
                        vertices[endpoints.1],
                        settings.radius_meters,
                    )?,
                    center_distance_meters: distances[start + position],
                });
            }
        }
    }
    Ok(Geometry {
        model_version: MODEL_VERSION,
        settings,
        vertex_directions: vertices,
        cell_directions: centers,
        reference_areas_square_meters: areas,
        side_faces: faces,
        offsets,
        neighbors,
        distances_meters: distances,
        fixed_external_boundary: fixed,
    })
}
