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

/// Prescribed runoff accumulated along the static drainage receivers.
/// Volumes use the caller's integer water unit; this result is not a dynamic
/// lake inventory or a discharge rate.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RunoffRouting {
    /// Local runoff plus all upstream runoff passing through each region.
    pub through_region_units: Vec<i128>,
    /// Final runoff at canonical wet-body or closed-sink terminal regions.
    pub terminal_units: Vec<i128>,
    pub total_input_units: i128,
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

    /// Route one prescribed, nonnegative runoff volume per region in the
    /// existing static receiver graph. Wet-body terminals share their canonical
    /// outlet; dry closed sinks retain their own input. No water is consumed,
    /// stored in lakes, delayed, or moved according to a hydraulic rate here.
    pub fn route_runoff_units(&self, runoff_units: &[i128]) -> Result<RunoffRouting, String> {
        let count = self.receivers.len();
        if runoff_units.len() != count || self.outlets.len() != count {
            return Err("Runoff input does not match drainage regions.".into());
        }
        if runoff_units.iter().any(|&volume| volume < 0) {
            return Err("Runoff volumes must be nonnegative.".into());
        }
        let mut incoming = vec![0_u32; count];
        for (region, &receiver) in self.receivers.iter().enumerate() {
            let receiver = receiver as usize;
            if receiver >= count {
                return Err("Drainage receiver is outside the region graph.".into());
            }
            if receiver != region {
                incoming[receiver] = incoming[receiver]
                    .checked_add(1)
                    .ok_or("Drainage dependency count overflowed.")?;
            }
        }

        let mut through_region_units = runoff_units.to_vec();
        let mut order: Vec<_> = (0..count).filter(|&region| incoming[region] == 0).collect();
        let mut cursor = 0;
        while cursor < order.len() {
            let region = order[cursor];
            cursor += 1;
            let receiver = self.receivers[region] as usize;
            if receiver == region {
                continue;
            }
            if self.outlets[region] != self.outlets[receiver] {
                return Err("Drainage outlet disagrees with its receiver.".into());
            }
            through_region_units[receiver] = through_region_units[receiver]
                .checked_add(through_region_units[region])
                .ok_or("Runoff accumulation overflowed.")?;
            incoming[receiver] -= 1;
            if incoming[receiver] == 0 {
                order.push(receiver);
            }
        }
        if order.len() != count {
            return Err("Drainage receiver graph contains a cycle.".into());
        }

        let mut terminal_units = vec![0_i128; count];
        for (region, &volume) in through_region_units.iter().enumerate() {
            if self.receivers[region] as usize != region {
                continue;
            }
            let outlet = self.outlets[region] as usize;
            if outlet >= count
                || self.receivers[outlet] as usize != outlet
                || self.outlets[outlet] as usize != outlet
            {
                return Err("Drainage terminal has an invalid canonical outlet.".into());
            }
            terminal_units[outlet] = terminal_units[outlet]
                .checked_add(volume)
                .ok_or("Runoff terminal sum overflowed.")?;
        }
        let total_input_units = runoff_units.iter().try_fold(0_i128, |sum, &volume| {
            sum.checked_add(volume)
                .ok_or("Runoff input sum overflowed.")
        })?;
        let total_terminal_units = terminal_units.iter().try_fold(0_i128, |sum, &volume| {
            sum.checked_add(volume)
                .ok_or("Runoff terminal sum overflowed.")
        })?;
        if total_input_units != total_terminal_units {
            return Err("Runoff routing did not conserve input.".into());
        }
        Ok(RunoffRouting {
            through_region_units,
            terminal_units,
            total_input_units,
        })
    }
}
