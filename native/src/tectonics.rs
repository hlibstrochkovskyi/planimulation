//! Static, seeded plate kinematics. No mantle solver, crust model, or time integration.
use crate::{Random, Surface, V, add, cross, dot, norm, unit};
use std::cmp::Ordering;
use std::collections::BinaryHeap;

pub const QUIET: u32 = 0;
pub const CONVERGENT: u32 = 1;
pub const DIVERGENT: u32 = 2;
pub const TRANSFORM: u32 = 3;
pub const QUIET_METERS_PER_YEAR: f64 = 1e-8;

#[derive(Clone, Debug, PartialEq)]
pub struct Tectonics {
    pub owners: Vec<u32>,
    pub seeds: Vec<u32>,
    pub angular_velocities: Vec<V>,  // radians/year, xyz
    pub boundary_cells: Vec<u32>,    // two region IDs per segment, lower ID first
    pub boundary_directions: Vec<V>, // two endpoints per segment
    pub boundary_motion: Vec<f64>,   // opening, signed shear; meters/year
    pub boundary_types: Vec<u32>,
}

#[derive(Clone, Copy)]
struct Visit {
    cost: f64,
    plate: u32,
    cell: u32,
}
impl PartialEq for Visit {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}
impl Eq for Visit {}
impl PartialOrd for Visit {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for Visit {
    fn cmp(&self, other: &Self) -> Ordering {
        // BinaryHeap is a max-heap. Reverse all keys for a stable min-heap.
        other
            .cost
            .total_cmp(&self.cost)
            .then_with(|| other.plate.cmp(&self.plate))
            .then_with(|| other.cell.cmp(&self.cell))
    }
}
fn sub(a: V, b: V) -> V {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
pub fn velocity(omega: V, direction: V, radius: f64) -> V {
    cross(omega, direction).map(|v| v * radius)
}

/// Normal points from the lower-side region toward the higher-side region.
/// Positive opening means divergence. Oblique motion retains both components.
pub fn relative_motion(omega_a: V, omega_b: V, position: V, normal: V, radius: f64) -> (f64, f64) {
    let relative = velocity(sub(omega_b, omega_a), position, radius);
    (
        dot(relative, normal),
        dot(relative, cross(position, normal)),
    )
}
pub fn classify(opening: f64, shear: f64) -> u32 {
    if opening.hypot(shear) <= QUIET_METERS_PER_YEAR {
        QUIET
    } else if opening.abs() >= shear.abs() {
        if opening < 0. { CONVERGENT } else { DIVERGENT }
    } else {
        TRANSFORM
    }
}

impl Tectonics {
    pub fn build(
        surface: &Surface,
        seed: &str,
        count: u32,
        max_speed_cm_year: f64,
        radius: f64,
    ) -> Self {
        let n = surface.centers.len();
        assert!((2..=32).contains(&count) && count as usize <= n);
        let mut roots_rng = Random::stream(seed, "tectonics.seeds");
        let mut seeds = Vec::with_capacity(count as usize);
        let mut closest = vec![2.0_f64; n];
        let mut next = (roots_rng.next() * n as f64) as usize;
        for _ in 0..count {
            seeds.push(next as u32);
            for (i, value) in closest.iter_mut().enumerate() {
                *value = value.min((1. - dot(surface.centers[next], surface.centers[i])).max(0.));
            }
            // Chosen roots have exactly zero probability despite roundoff in dot(v,v).
            for &root in &seeds {
                closest[root as usize] = 0.;
            }
            let total: f64 = closest.iter().map(|v| v * v).sum();
            let mut target = roots_rng.next() * total;
            for (i, value) in closest.iter().enumerate() {
                if *value > 0. {
                    next = i;
                }
                target -= value * value;
                if target < 0. {
                    break;
                }
            }
        }
        let mut resistance_rng = Random::stream(seed, "tectonics.partition-resistance");
        let modes: Vec<_> = (0..6)
            .map(|_| {
                (
                    [
                        (resistance_rng.next() * 2. - 1.) * 4.,
                        (resistance_rng.next() * 2. - 1.) * 4.,
                        (resistance_rng.next() * 2. - 1.) * 4.,
                    ],
                    resistance_rng.next() * std::f64::consts::TAU,
                )
            })
            .collect();
        let resistance: Vec<f64> = surface
            .centers
            .iter()
            .map(|p| {
                1. + 0.35
                    * modes
                        .iter()
                        .map(|(wave, phase)| (dot(*wave, *p) + phase).sin())
                        .sum::<f64>()
                    / 6.
            })
            .collect();
        let mut owners = vec![u32::MAX; n];
        let mut queue = BinaryHeap::new();
        for (plate, &cell) in seeds.iter().enumerate() {
            queue.push(Visit {
                cost: 0.,
                plate: plate as u32,
                cell,
            });
        }
        while let Some(Visit { cost, plate, cell }) = queue.pop() {
            let i = cell as usize;
            if owners[i] != u32::MAX {
                continue;
            }
            owners[i] = plate;
            for k in surface.offsets[i]..surface.offsets[i + 1] {
                let j = surface.neighbors[k as usize] as usize;
                if owners[j] != u32::MAX {
                    continue;
                }
                // Dimensionless positive cost, independent of physical radius and speed.
                let edge = norm(sub(surface.centers[i], surface.centers[j]))
                    * (resistance[i] + resistance[j])
                    * 0.5;
                queue.push(Visit {
                    cost: cost + edge,
                    plate,
                    cell: j as u32,
                });
            }
        }
        let mut motion_rng = Random::stream(seed, "tectonics.motion");
        let angular_velocities = (0..count)
            .map(|_| {
                let y = motion_rng.next() * 2. - 1.;
                let phi = motion_rng.next() * std::f64::consts::TAU;
                let r = (1. - y * y).sqrt();
                let angular_speed =
                    (0.25 + 0.75 * motion_rng.next()) * max_speed_cm_year / 100. / radius;
                [
                    r * phi.cos() * angular_speed,
                    y * angular_speed,
                    r * phi.sin() * angular_speed,
                ]
            })
            .collect();
        let mut result = Self {
            owners,
            seeds,
            angular_velocities,
            boundary_cells: vec![],
            boundary_directions: vec![],
            boundary_motion: vec![],
            boundary_types: vec![],
        };
        for face in surface.faces.as_chunks::<3>().0 {
            let centroid = unit(add(
                add(
                    surface.centers[face[0] as usize],
                    surface.centers[face[1] as usize],
                ),
                surface.centers[face[2] as usize],
            ));
            for j in 0..3 {
                let a = face[j].min(face[(j + 1) % 3]) as usize;
                let b = face[j].max(face[(j + 1) % 3]) as usize;
                if result.owners[a] == result.owners[b] {
                    continue;
                }
                // Each face contributes one half of the bent barycentric dual edge.
                let midpoint = unit(add(surface.centers[a], surface.centers[b]));
                let position = unit(add(centroid, midpoint));
                let mut normal = unit(cross(centroid, midpoint));
                if dot(normal, sub(surface.centers[b], surface.centers[a])) < 0. {
                    normal = normal.map(|v| -v);
                }
                let (opening, shear) = relative_motion(
                    result.angular_velocities[result.owners[a] as usize],
                    result.angular_velocities[result.owners[b] as usize],
                    position,
                    normal,
                    radius,
                );
                result.boundary_cells.extend([a as u32, b as u32]);
                result.boundary_directions.extend([centroid, midpoint]);
                result.boundary_motion.extend([opening, shear]);
                result.boundary_types.push(classify(opening, shear));
            }
        }
        result
    }
}
