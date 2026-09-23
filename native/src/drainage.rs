//! Static, single-receiver drainage potential. No water transport or depression filling.
use crate::Surface;

#[derive(Clone, Debug, PartialEq)]
pub struct Drainage {
    /// Self means terminal: an existing wet region or a closed dry sink.
    pub receivers: Vec<u32>,
    /// Canonical terminal region; all cells of one initial water body share its minimum ID.
    pub outlets: Vec<u32>,
    /// Equal-height graph hops to a downhill exit or canonical closed-flat sink.
    pub flat_steps: Vec<u32>,
    /// Upstream dry-land reference area, including this cell when dry; not discharge.
    pub contributing_area: Vec<f64>,
}

impl Drainage {
    pub fn build(s: &Surface, heights: &[f64], bodies: &[u32]) -> Self {
        let n = heights.len();
        assert_eq!(n, s.areas.len());
        assert_eq!(n, bodies.len());
        let mut receivers: Vec<u32> = (0..n as u32).collect();
        let mut flat_steps = vec![u32::MAX; n];
        let mut roots = vec![u32::MAX; n + 1];
        for i in 0..n {
            assert!(heights[i].is_finite() && s.areas[i] > 0.);
            if bodies[i] > 0 {
                roots[bodies[i] as usize] = roots[bodies[i] as usize].min(i as u32);
                flat_steps[i] = 0;
                continue;
            }
            let mut steepest = 0.;
            for k in s.offsets[i]..s.offsets[i + 1] {
                let j = s.neighbors[k as usize] as usize;
                let slope = (heights[i] - heights[j]) / s.distances[k as usize];
                if slope > 0.
                    && (slope > steepest || (slope == steepest && (j as u32) < receivers[i]))
                {
                    receivers[i] = j as u32;
                    steepest = slope;
                }
            }
            if receivers[i] != i as u32 {
                flat_steps[i] = 0;
            }
        }

        let mut visited = vec![false; n];
        let mut component = Vec::new();
        let mut queue = Vec::new();
        for start in 0..n {
            if visited[start] || bodies[start] > 0 {
                continue;
            }
            component.clear();
            component.push(start);
            visited[start] = true;
            let mut cursor = 0;
            while cursor < component.len() {
                let i = component[cursor];
                cursor += 1;
                for k in s.offsets[i]..s.offsets[i + 1] {
                    let j = s.neighbors[k as usize] as usize;
                    if !visited[j] && bodies[j] == 0 && heights[j] == heights[i] {
                        visited[j] = true;
                        component.push(j);
                    }
                }
            }
            queue.clear();
            queue.extend(component.iter().copied().filter(|&i| flat_steps[i] == 0));
            queue.sort_unstable();
            if queue.is_empty() {
                // Ascending outer traversal makes start the smallest component ID.
                flat_steps[start] = 0;
                queue.push(start);
            }
            cursor = 0;
            while cursor < queue.len() {
                let i = queue[cursor];
                cursor += 1;
                for k in s.offsets[i]..s.offsets[i + 1] {
                    let j = s.neighbors[k as usize] as usize;
                    if bodies[j] == 0 && heights[j] == heights[i] && flat_steps[j] == u32::MAX {
                        receivers[j] = i as u32;
                        flat_steps[j] = flat_steps[i] + 1;
                        queue.push(j);
                    }
                }
            }
        }

        let mut incoming = vec![0_u32; n];
        let mut contributing_area: Vec<f64> = (0..n)
            .map(|i| if bodies[i] == 0 { s.areas[i] } else { 0. })
            .collect();
        for (i, &r) in receivers.iter().enumerate() {
            if r != i as u32 {
                incoming[r as usize] += 1;
            }
        }
        queue.clear();
        queue.extend((0..n).filter(|&i| incoming[i] == 0));
        let mut cursor = 0;
        while cursor < queue.len() {
            let i = queue[cursor];
            cursor += 1;
            let r = receivers[i] as usize;
            if r == i {
                continue;
            }
            contributing_area[r] += contributing_area[i];
            incoming[r] -= 1;
            if incoming[r] == 0 {
                queue.push(r);
            }
        }
        assert_eq!(queue.len(), n, "Drainage must be acyclic.");
        let mut outlets = vec![0; n];
        for &i in queue.iter().rev() {
            outlets[i] = if bodies[i] > 0 {
                roots[bodies[i] as usize]
            } else if receivers[i] == i as u32 {
                i as u32
            } else {
                outlets[receivers[i] as usize]
            };
        }
        Self {
            receivers,
            outlets,
            flat_steps,
            contributing_area,
        }
    }
}
