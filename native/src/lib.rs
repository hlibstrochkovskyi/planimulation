use serde::{Deserialize, Serialize};
use std::collections::{BTreeSet, HashMap};
use std::f64::consts::PI;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Recipe {
    pub schema_version: u32,
    pub model_version: String,
    pub random_version: String,
    pub seed: String,
    pub subdivision: u32,
    pub radius_meters: f64,
    pub plate_count: u32,
    pub max_plate_speed_cm_per_year: f64,
    pub continental_fraction: f64,
    pub continental_scale: f64,
    pub relief_scale: f64,
    pub boundary_width_km: f64,
    pub detail_amplitude_meters: f64,
    pub water: water::WaterSettings,
}

impl Recipe {
    pub fn validate(&self) -> Result<(), String> {
        if self.schema_version != 1
            || self.model_version != "basins-1"
            || self.random_version != "fnv1a-utf8-mulberry32-1"
        {
            return Err("Unsupported recipe version; expected basins-1.".into());
        }
        if self.seed.trim().is_empty() || self.seed.encode_utf16().count() > 128 {
            return Err("Seed must contain 1–128 UTF-16 code units and cannot be blank.".into());
        }
        if self.subdivision > 6
            || !self.radius_meters.is_finite()
            || !(100_000.0..=20_000_000.0).contains(&self.radius_meters)
        {
            return Err("Unsupported resolution or radius.".into());
        }
        if !(2..=32).contains(&self.plate_count)
            || self.plate_count > 10 * 4_u32.pow(self.subdivision) + 2
            || !self.max_plate_speed_cm_per_year.is_finite()
            || !(0.0..=20.0).contains(&self.max_plate_speed_cm_per_year)
        {
            return Err("Plate count must be 2–32 and no larger than the region count; maximum plate speed must be 0–20 cm/year.".into());
        }
        if !self.continental_fraction.is_finite()
            || !(0.0..=1.).contains(&self.continental_fraction)
            || !self.continental_scale.is_finite()
            || !(0.5..=2.).contains(&self.continental_scale)
        {
            return Err(
                "Continental fraction must be 0–1; continental scale must be 0.5–2.".into(),
            );
        }
        if !self.relief_scale.is_finite()
            || !(0.0..=2.).contains(&self.relief_scale)
            || !self.boundary_width_km.is_finite()
            || !(50.0..=1000.).contains(&self.boundary_width_km)
            || !self.detail_amplitude_meters.is_finite()
            || !(0.0..=1000.).contains(&self.detail_amplitude_meters)
        {
            return Err(
                "Relief scale must be 0–2, boundary width 50–1000 km, detail amplitude 0–1000 m."
                    .into(),
            );
        }
        self.water.validate(4. * PI * self.radius_meters.powi(2))
    }
}

type V = [f64; 3];
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
fn norm(a: V) -> f64 {
    a[0].hypot(a[1]).hypot(a[2])
}
fn unit(a: V) -> V {
    let n = norm(a);
    a.map(|v| v / n)
}
fn add(a: V, b: V) -> V {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}
fn area(a: V, b: V, c: V) -> f64 {
    2.0 * dot(a, cross(b, c))
        .abs()
        .atan2(1.0 + dot(a, b) + dot(b, c) + dot(c, a))
}
pub fn hash(bytes: &[u8]) -> u32 {
    bytes.iter().fold(0x811c9dc5, |h, b| {
        (h ^ u32::from(*b)).wrapping_mul(0x01000193)
    })
}
pub struct Random(pub u32);
impl Random {
    pub fn stream(seed: &str, name: &str) -> Self {
        Self(hash(&serde_json::to_vec(&[seed, name]).unwrap()))
    }
    pub fn next_u32(&mut self) -> u32 {
        self.0 = self.0.wrapping_add(0x6d2b79f5);
        let mut v = self.0;
        v = (v ^ (v >> 15)).wrapping_mul(v | 1);
        v ^= v.wrapping_add((v ^ (v >> 7)).wrapping_mul(v | 61));
        v ^ (v >> 14)
    }
    fn next(&mut self) -> f64 {
        f64::from(self.next_u32()) / 4294967296.0
    }
}

pub struct Surface {
    pub centers: Vec<V>,
    pub faces: Vec<u32>,
    pub offsets: Vec<u32>,
    pub neighbors: Vec<u32>,
    pub distances: Vec<f64>,
    pub areas: Vec<f64>,
    pub boundary_offsets: Vec<u32>,
    pub boundaries: Vec<V>,
}

impl Surface {
    pub fn build(level: u32, radius: f64) -> Self {
        assert!(level <= 6 && radius.is_finite() && radius > 0.0);
        let p = (1.0 + 5.0_f64.sqrt()) / 2.0;
        let mut vertices: Vec<V> = vec![
            [-1., p, 0.],
            [1., p, 0.],
            [-1., -p, 0.],
            [1., -p, 0.],
            [0., -1., p],
            [0., 1., p],
            [0., -1., -p],
            [0., 1., -p],
            [p, 0., -1.],
            [p, 0., 1.],
            [-p, 0., -1.],
            [-p, 0., 1.],
        ]
        .into_iter()
        .map(unit)
        .collect();
        let mut faces: Vec<u32> = vec![
            0, 11, 5, 0, 5, 1, 0, 1, 7, 0, 7, 10, 0, 10, 11, 1, 5, 9, 5, 11, 4, 11, 10, 2, 10, 7,
            6, 7, 1, 8, 3, 9, 4, 3, 4, 2, 3, 2, 6, 3, 6, 8, 3, 8, 9, 4, 9, 5, 2, 4, 11, 6, 2, 10,
            8, 6, 7, 9, 8, 1,
        ];
        for _ in 0..level {
            let mut cache = HashMap::new();
            let mut bisect = |a: u32, b: u32| -> u32 {
                *cache.entry((a.min(b), a.max(b))).or_insert_with(|| {
                    let id = vertices.len() as u32;
                    vertices.push(unit(add(vertices[a as usize], vertices[b as usize])));
                    id
                })
            };
            let mut next = Vec::with_capacity(faces.len() * 4);
            for t in faces.as_chunks::<3>().0 {
                let (a, b, c) = (t[0], t[1], t[2]);
                let (ab, bc, ca) = (bisect(a, b), bisect(b, c), bisect(c, a));
                next.extend_from_slice(&[a, ab, ca, b, bc, ab, c, ca, bc, ab, bc, ca]);
            }
            faces = next;
        }
        let mut adjacency = vec![BTreeSet::new(); vertices.len()];
        let mut incident = vec![Vec::new(); vertices.len()];
        for t in faces.as_chunks::<3>().0 {
            let centroid = unit(add(
                add(vertices[t[0] as usize], vertices[t[1] as usize]),
                vertices[t[2] as usize],
            ));
            for j in 0..3 {
                adjacency[t[j] as usize].insert(t[(j + 1) % 3]);
                adjacency[t[j] as usize].insert(t[(j + 2) % 3]);
                incident[t[j] as usize].push(centroid);
            }
        }
        let mut s = Self {
            centers: vertices,
            faces,
            offsets: vec![],
            neighbors: vec![],
            distances: vec![],
            areas: vec![],
            boundary_offsets: vec![],
            boundaries: vec![],
        };
        for (id, adjacent) in adjacency.iter().enumerate() {
            let center = s.centers[id];
            s.offsets.push(s.neighbors.len() as u32);
            s.boundary_offsets.push(s.boundaries.len() as u32);
            let mut ring = incident[id].clone();
            let anchor = ring[0];
            for &n in adjacent {
                let other = s.centers[n as usize];
                s.neighbors.push(n);
                s.distances
                    .push(norm(cross(center, other)).atan2(dot(center, other)) * radius);
                ring.push(unit(add(center, other)));
            }
            let tangent = unit(cross(
                if center[2].abs() < 0.9 {
                    [0., 0., 1.]
                } else {
                    [0., 1., 0.]
                },
                center,
            ));
            let bitangent = cross(center, tangent);
            ring.sort_by(|a, b| {
                dot(*a, bitangent)
                    .atan2(dot(*a, tangent))
                    .total_cmp(&dot(*b, bitangent).atan2(dot(*b, tangent)))
            });
            let anchor_index = ring.iter().position(|v| *v == anchor).unwrap();
            ring.rotate_left(anchor_index);
            let solid: f64 = (0..ring.len())
                .map(|j| area(center, ring[j], ring[(j + 1) % ring.len()]))
                .sum();
            s.areas.push(solid * radius * radius);
            s.boundaries.extend(ring);
        }
        s.offsets.push(s.neighbors.len() as u32);
        s.boundary_offsets.push(s.boundaries.len() as u32);
        s
    }
}

pub struct World {
    pub recipe: Recipe,
    pub surface: Surface,
    pub field: Vec<f64>,
    pub tick: u64,
    pub initial_mass: f64,
    pub tectonics: tectonics::Tectonics,
    pub crust: crust::Crust,
    pub terrain: terrain::Terrain,
    pub water: water::Water,
    pub drainage: drainage::Drainage,
    pub basins: basins::Basins,
}
impl World {
    pub fn generate(recipe: Recipe) -> Result<Self, String> {
        recipe.validate()?;
        let surface = Surface::build(recipe.subdivision, recipe.radius_meters);
        let crust = crust::Crust::build(
            &surface,
            &recipe.seed,
            recipe.continental_fraction,
            recipe.continental_scale,
        );
        let tectonics = tectonics::Tectonics::build(
            &surface,
            &recipe.seed,
            recipe.plate_count,
            recipe.max_plate_speed_cm_per_year,
            recipe.radius_meters,
        );
        let terrain = terrain::Terrain::build(&surface, &crust, &tectonics, &recipe);
        let water = water::Water::generate(&surface, &terrain.elevation, &recipe.water)?;
        let drainage = drainage::Drainage::build(&surface, &terrain.elevation, &water.body_ids);
        let basins = basins::Basins::build(&surface, &terrain.elevation)?;
        let mut rng = Random::stream(&recipe.seed, "diagnostic-field");
        let modes: Vec<_> = (0..8)
            .map(|_| {
                (
                    [
                        (rng.next() * 2. - 1.) * 9.,
                        (rng.next() * 2. - 1.) * 9.,
                        (rng.next() * 2. - 1.) * 9.,
                    ],
                    rng.next() * PI * 2.,
                )
            })
            .collect();
        let field: Vec<f64> = surface
            .centers
            .iter()
            .map(|v| {
                modes
                    .iter()
                    .map(|(m, p)| (dot(*m, *v) + p).sin())
                    .sum::<f64>()
                    / 8.
            })
            .collect();
        let total: f64 = surface.areas.iter().sum();
        if (total / (4. * PI * recipe.radius_meters.powi(2)) - 1.).abs() > 1e-10 {
            return Err("Surface area invariant failed.".into());
        }
        let mut w = Self {
            recipe,
            surface,
            field,
            tick: 0,
            initial_mass: 0.,
            tectonics,
            crust,
            terrain,
            water,
            drainage,
            basins,
        };
        w.initial_mass = w.mass();
        Ok(w)
    }
    pub fn mass(&self) -> f64 {
        self.field
            .iter()
            .zip(&self.surface.areas)
            .map(|(v, a)| (v + 1.) * 0.5 * a)
            .sum()
    }
    /// Diagnostic graph diffusion, not hydrology or a calibrated physical timestep.
    /// Each undirected edge exchanges one equal-and-opposite amount. The 0.1
    /// coefficient and maximum degree six make every update a convex combination.
    pub fn advance(&mut self, steps: u32) -> Result<(), String> {
        if !(1..=100).contains(&steps) {
            return Err("Steps must be from 1 to 100.".into());
        }
        let s = &self.surface;
        let mut delta = vec![0.; self.field.len()];
        for _ in 0..steps {
            delta.fill(0.);
            for i in 0..self.field.len() {
                for k in s.offsets[i]..s.offsets[i + 1] {
                    let j = s.neighbors[k as usize] as usize;
                    if j <= i {
                        continue;
                    }
                    let flux = 0.1 * s.areas[i].min(s.areas[j]) * (self.field[i] - self.field[j]);
                    delta[i] -= flux;
                    delta[j] += flux;
                }
            }
            for (i, value) in self.field.iter_mut().enumerate() {
                *value += delta[i] / s.areas[i];
            }
            self.tick += 1;
        }
        Ok(())
    }
}

pub mod basins;
pub mod crust;
pub mod drainage;
pub mod reservoir;
pub mod tectonics;
pub mod terrain;
pub mod water;
pub mod wire;
