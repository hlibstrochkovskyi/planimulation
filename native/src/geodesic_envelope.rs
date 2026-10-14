//! Initial belt attenuation from direct minor-arc distances on a reference
//! sphere. Bounded weighted-source BVH, not graph paths or tectonic history.
use serde::Serialize;
type V = [f64; 3];
pub const MODEL_VERSION: &str = "geodesic-arc-envelope-1";
pub const MAX_SOURCES: usize = 131_072;
const LEAF: usize = 8;
const CAP_GUARD: f64 = 128. * f64::EPSILON;
fn dot(a: V, b: V) -> f64 {
    a.into_iter().zip(b).map(|(a, b)| a * b).sum()
}
fn cross(a: V, b: V) -> V {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}
fn unit(v: V) -> V {
    let length = dot(v, v).sqrt();
    v.map(|v| v / length)
}
fn direction(v: V) -> Result<V, String> {
    if v.iter().any(|v| !v.is_finite()) || (dot(v, v) - 1.).abs() > 1e-12 {
        return Err("Geodesic sampling requires a finite unit direction.".into());
    }
    Ok(unit(v))
}
fn angle(a: V, b: V) -> f64 {
    dot(cross(a, b), cross(a, b)).sqrt().atan2(dot(a, b))
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Source {
    start: V,
    end: V,
    tangent: V,
    midpoint: V,
    length: f64,
    cap_radius: f64,
    /// Closing, spreading-ridge and continental-rift magnitudes, meters.
    amplitudes_meters: [f64; 3],
}
impl Source {
    /// Coincident endpoints represent a point source. Positive unresolved tiny
    /// arcs and near-antipodal endpoints refuse; no arbitrary great circle.
    pub fn new(start: V, end: V, amplitudes_meters: [f64; 3]) -> Result<Self, String> {
        let start = direction(start)?;
        let end = direction(end)?;
        if amplitudes_meters
            .iter()
            .any(|v| !v.is_finite() || !(0. ..=12000.).contains(v))
        {
            return Err("Invalid geodesic source amplitude.".into());
        }
        let length = angle(start, end);
        if length == 0. && start != end
            || length > 0. && length < 1e-12
            || length >= std::f64::consts::PI - 1e-12
        {
            return Err("Unresolved or ambiguous geodesic source arc.".into());
        }
        let tangent = if length == 0. {
            [0.; 3]
        } else {
            unit(cross(unit(cross(start, end)), start))
        };
        let midpoint = if length == 0. {
            start
        } else {
            unit(std::array::from_fn(|i| {
                start[i] * (length * 0.5).cos() + tangent[i] * (length * 0.5).sin()
            }))
        };
        let cap_radius = angle(midpoint, start)
            .max(angle(midpoint, end))
            .max(length * 0.5)
            + CAP_GUARD;
        Ok(Self {
            start,
            end,
            tangent,
            midpoint,
            length,
            cap_radius,
            amplitudes_meters,
        })
    }
    fn distance_unchecked(&self, p: V) -> f64 {
        if self.length == 0. {
            return angle(p, self.start);
        }
        // Maximize p·(a*cos(t)+tangent*sin(t)) on the closed minor arc.
        let t = dot(p, self.tangent).atan2(dot(p, self.start));
        if t > 0. && t < self.length {
            let closest = unit(std::array::from_fn(|i| {
                self.start[i] * t.cos() + self.tangent[i] * t.sin()
            }));
            angle(p, closest)
        } else {
            angle(p, self.start).min(angle(p, self.end))
        }
    }
    pub fn distance_radians(&self, p: V) -> Result<f64, String> {
        Ok(self.distance_unchecked(direction(p)?))
    }
}
#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    pub radius_meters: f64,
    pub width_meters: f64,
}
#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Sample {
    pub values_meters: [f64; 3],
    pub winner_sources: [Option<usize>; 3],
    pub evaluated_arcs: usize,
    pub evaluated_bounds: usize,
}
struct Node {
    center: V,
    radius: f64,
    maxima: [f64; 3],
    range: std::ops::Range<usize>,
    children: Option<[usize; 2]>,
}
pub struct Field {
    sources: Vec<Source>,
    order: Vec<usize>,
    nodes: Vec<Node>,
    settings: Settings,
}
impl Field {
    pub fn new(sources: Vec<Source>, settings: Settings) -> Result<Self, String> {
        if sources.len() > MAX_SOURCES
            || !settings.radius_meters.is_finite()
            || !(100_000. ..=20_000_000.).contains(&settings.radius_meters)
            || !settings.width_meters.is_finite()
            || !(50_000. ..=1_000_000.).contains(&settings.width_meters)
        {
            return Err("Invalid or excessive geodesic envelope settings/sources.".into());
        }
        let order: Vec<_> = (0..sources.len())
            .filter(|&i| sources[i].amplitudes_meters.iter().any(|&v| v > 0.))
            .collect();
        let mut field = Self {
            sources,
            order,
            nodes: Vec::new(),
            settings,
        };
        if !field.order.is_empty() {
            field.build(0..field.order.len());
        }
        Ok(field)
    }
    pub fn sources(&self) -> &[Source] {
        &self.sources
    }
    fn build(&mut self, range: std::ops::Range<usize>) -> usize {
        let mut sum = [0.; 3];
        let mut minimum = [f64::INFINITY; 3];
        let mut maximum = [f64::NEG_INFINITY; 3];
        let mut maxima = [0_f64; 3];
        for &i in &self.order[range.clone()] {
            let s = &self.sources[i];
            for axis in 0..3 {
                sum[axis] += s.midpoint[axis];
                minimum[axis] = minimum[axis].min(s.midpoint[axis]);
                maximum[axis] = maximum[axis].max(s.midpoint[axis]);
                maxima[axis] = maxima[axis].max(s.amplitudes_meters[axis]);
            }
        }
        let center = if dot(sum, sum) > 1e-24 {
            unit(sum)
        } else {
            self.sources[self.order[range.start]].midpoint
        };
        let radius = self.order[range.clone()]
            .iter()
            .map(|&i| {
                angle(center, self.sources[i].midpoint) + self.sources[i].cap_radius + CAP_GUARD
            })
            .fold(0., f64::max)
            .min(std::f64::consts::PI);
        let id = self.nodes.len();
        self.nodes.push(Node {
            center,
            radius,
            maxima,
            range: range.clone(),
            children: None,
        });
        if range.len() > LEAF {
            let axis = (0..3)
                .max_by(|&a, &b| (maximum[a] - minimum[a]).total_cmp(&(maximum[b] - minimum[b])))
                .unwrap();
            self.order[range.clone()].sort_unstable_by(|&a, &b| {
                self.sources[a].midpoint[axis]
                    .total_cmp(&self.sources[b].midpoint[axis])
                    .then(a.cmp(&b))
            });
            let mid = range.start + range.len() / 2;
            let children = [self.build(range.start..mid), self.build(mid..range.end)];
            self.nodes[id].children = Some(children);
        }
        id
    }
    fn upper(&self, id: usize, p: V) -> [f64; 3] {
        let node = &self.nodes[id];
        let lower = (angle(p, node.center) - node.radius - CAP_GUARD).max(0.);
        let decay = (-lower * self.settings.radius_meters / self.settings.width_meters).exp();
        node.maxima.map(|v| {
            let upper = v * decay;
            if upper == 0. { 0. } else { upper.next_up() }
        })
    }
    fn evaluate(&self, source: usize, p: V, result: &mut Sample) {
        result.evaluated_arcs += 1;
        let s = &self.sources[source];
        let decay = (-s.distance_unchecked(p) * self.settings.radius_meters
            / self.settings.width_meters)
            .exp();
        for j in 0..3 {
            let value = s.amplitudes_meters[j] * decay;
            if value > result.values_meters[j]
                || value > 0.
                    && value == result.values_meters[j]
                    && result.winner_sources[j].is_none_or(|old| source < old)
            {
                result.values_meters[j] = value;
                result.winner_sources[j] = Some(source);
            }
        }
    }
    /// Deterministic bounded-tree query. Worst case remains a full source scan;
    /// no approximate nearest-source truncation is substituted for the maximum.
    pub fn sample(&self, p: V) -> Result<Sample, String> {
        let p = direction(p)?;
        let mut result = Sample {
            values_meters: [0.; 3],
            winner_sources: [None; 3],
            evaluated_arcs: 0,
            evaluated_bounds: 0,
        };
        let mut stack = Vec::with_capacity(32);
        if !self.nodes.is_empty() {
            stack.push(0);
        }
        while let Some(id) = stack.pop() {
            result.evaluated_bounds += 1;
            let upper = self.upper(id, p);
            if !(0..3).any(|j| upper[j] > 0. && upper[j] >= result.values_meters[j]) {
                continue;
            }
            if let Some(children) = self.nodes[id].children {
                let a = self.upper(children[0], p).into_iter().fold(0., f64::max);
                let b = self.upper(children[1], p).into_iter().fold(0., f64::max);
                result.evaluated_bounds += 2;
                if a >= b {
                    stack.push(children[1]);
                    stack.push(children[0]);
                } else {
                    stack.push(children[0]);
                    stack.push(children[1]);
                }
            } else {
                for &i in &self.order[self.nodes[id].range.clone()] {
                    self.evaluate(i, p, &mut result);
                }
            }
        }
        Ok(result)
    }
    /// Direct oracle using exactly the same local geometry/law, no pruning.
    pub fn sample_brute_force(&self, p: V) -> Result<Sample, String> {
        let p = direction(p)?;
        let mut result = Sample {
            values_meters: [0.; 3],
            winner_sources: [None; 3],
            evaluated_arcs: 0,
            evaluated_bounds: 0,
        };
        for i in 0..self.sources.len() {
            self.evaluate(i, p, &mut result);
        }
        Ok(result)
    }
}
