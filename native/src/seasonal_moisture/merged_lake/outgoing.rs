//! One full one-level parent can send owned queue excess along one real outlet.
//! No next-parent activation, receiver overflow chain, or concurrent source policy.
use super::super::{
    Checkpoint as SeasonalCheckpoint,
    closed_lake::spill::{self, Destination, Route},
    leaf_spill, reference_pool,
};
use super::{Layout, frontier};
use crate::{World, moisture_transport::total_mass, surface_water::CompensatedStock};
use serde::{Deserialize, Serialize};

pub const PARENT_MODEL_VERSION: &str = "common-sill-outlet-4";
pub const LIFECYCLE_MODEL_VERSION: &str = "common-sill-lifecycle-3";
pub const MODEL_VERSION: &str = "common-sill-outgoing-transfer-1";
const MAX_GRANTS: usize = 4096;

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Connection {
    pub basin_node: usize,
    pub first_connection_level_meters: Option<f64>,
    pub routes: Vec<Route>,
    pub has_unique_route: bool,
}
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Event {
    pub input_terminal_region: usize,
    pub source_basin_node: usize,
    pub route: Route,
    pub kilograms: f64,
}
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Checkpoint {
    pub model_version: String,
    /// Group-indexed gross departures, not liquid stocks or child leaf departures.
    pub cumulative_outgoing: leaf_spill::Components,
    /// Region-indexed actual receiver contacts, independently checked against outlets.
    pub cumulative_incoming: leaf_spill::Components,
}
impl Checkpoint {
    pub(super) fn zero(groups: usize, regions: usize) -> Self {
        Self {
            model_version: MODEL_VERSION.into(),
            cumulative_outgoing: leaf_spill::Components::zero(groups),
            cumulative_incoming: leaf_spill::Components::zero(regions),
        }
    }
}
pub(crate) fn state(cp: &SeasonalCheckpoint) -> Option<&Checkpoint> {
    cp.merged_lake_state
        .as_ref()?
        .frontier
        .as_ref()?
        .outgoing_spill
        .as_ref()
}
pub(crate) fn incoming(
    cp: &SeasonalCheckpoint,
    region: usize,
) -> Result<Option<CompensatedStock>, String> {
    state(cp)
        .map(|s| s.cumulative_incoming.stock(region))
        .transpose()
}
pub(crate) fn append_events(target: &mut Vec<Event>, events: Vec<Event>) -> Result<(), String> {
    if events.len() > MAX_GRANTS.saturating_sub(target.len()) {
        return Err("Parent spill exceeds the 4096 caller-interval grant bound.".into());
    }
    target.extend(events);
    Ok(())
}
impl Layout {
    pub fn build_outlets(&self, world: &World) -> Result<Vec<Connection>, String> {
        let geometry = spill::checked_geometry(world)?;
        self.groups
            .iter()
            .map(|group| {
                let basin_node = group.description.basin_node;
                let routes = spill::branch_routes(world, &geometry, basin_node, true)?;
                Ok(Connection {
                    basin_node,
                    first_connection_level_meters: world.basins.nodes()[basin_node]
                        .spill_level_meters,
                    has_unique_route: routes.len() == 1,
                    routes,
                })
            })
            .collect()
    }
    pub fn outgoing_sources(&self, cp: &SeasonalCheckpoint) -> Result<Vec<usize>, String> {
        if self.outgoing_connections.is_none() {
            return Ok(Vec::new());
        }
        self.groups
            .iter()
            .enumerate()
            .filter_map(|(g, group)| {
                self.active(cp, g)?;
                group
                    .description
                    .child_terminals
                    .iter()
                    .any(|&r| {
                        cp.leaf_spill_state
                            .as_ref()
                            .unwrap()
                            .pending_input
                            .high_kilograms[r]
                            > 0.
                    })
                    .then_some(Ok(g))
            })
            .collect()
    }
    fn outlet(&self, g: usize) -> Result<(&Connection, &Route, f64), String> {
        let c = &self
            .outgoing_connections
            .as_ref()
            .ok_or("Parent outlets are not enabled.")?[g];
        if c.routes.len() != 1 {
            return Err("Parent spill needs one unique geographic route; junction allocation is unsupported.".into());
        }
        let head = c
            .first_connection_level_meters
            .ok_or("Closed parent has no external outlet.")?;
        Ok((c, &c.routes[0], head))
    }
    pub fn spill_pending(
        &self,
        cp: &mut SeasonalCheckpoint,
        pool: &reference_pool::Layout,
        leaves: &leaf_spill::Layout,
        g: usize,
    ) -> Result<Vec<Event>, String> {
        let group = &self.groups[g];
        let p = self
            .active(cp, g)
            .ok_or("Parent spill source is not active.")?;
        let parent = &cp.merged_lake_state.as_ref().unwrap().parents[p];
        if (parent.surplus_high_kilograms, parent.surplus_low_kilograms) != (group.capacity(), 0.) {
            return Err("Parent spill source is below its represented outlet capacity.".into());
        }
        let (connection, route, head) = self.outlet(g)?;
        let destination = leaf_spill::Layout::destination_region(&route.destination);
        if self.by_terminal[destination] == Some(g) {
            return Err("Parent spill route returns to its own child.".into());
        }
        let mut events = Vec::new();
        for &r in &group.description.child_terminals {
            let mut donor = cp
                .leaf_spill_state
                .as_ref()
                .unwrap()
                .pending_input
                .stock(r)?;
            if donor.high == 0. {
                continue;
            }
            let grants = match route.destination {
                Destination::ClosedTerminal { terminal_region } => {
                    leaves.receive_terminal(cp, terminal_region, head, &mut donor, Some(self))?
                }
                Destination::ReferenceBody {
                    body_id,
                    contact_region,
                } => {
                    leaves.receive_reference(cp, pool, body_id, contact_region, head, &mut donor)?
                }
            };
            if donor.high != 0. || donor.low != 0. {
                return Err("Parent spill receiver requires overflow/nested merging; the complete interval is refused.".into());
            }
            for grant in grants {
                if events.len() >= MAX_GRANTS {
                    return Err("Parent spill exceeds its 4096 resolution grant bound.".into());
                }
                let ledger = frontier::state_mut(cp)
                    .outgoing_spill
                    .as_mut()
                    .ok_or("Missing parent outgoing provenance.")?;
                ledger.cumulative_outgoing.credit(g, grant)?;
                ledger.cumulative_incoming.credit(destination, grant)?;
                events.push(Event {
                    input_terminal_region: r,
                    source_basin_node: connection.basin_node,
                    route: route.clone(),
                    kilograms: grant,
                });
            }
            cp.leaf_spill_state
                .as_mut()
                .unwrap()
                .pending_input
                .set(r, donor);
        }
        Ok(events)
    }
    pub fn validate_outgoing(
        &self,
        cp: &SeasonalCheckpoint,
        pool: &reference_pool::Layout,
        leaves: &leaf_spill::Layout,
    ) -> Result<(f64, usize), String> {
        let Some(connections) = &self.outgoing_connections else {
            if state(cp).is_some() {
                return Err("Parent outgoing provenance outside its pinned mode.".into());
            }
            return Ok((0., 0));
        };
        let s = state(cp).ok_or("Missing parent outgoing provenance.")?;
        let history = cp
            .merged_lake_state
            .as_ref()
            .unwrap()
            .frontier
            .as_ref()
            .unwrap();
        let n = self.by_region.len();
        let k = self.groups.len();
        if s.model_version != MODEL_VERSION
            || history.merge_counts.len() != k
            || s.cumulative_outgoing.high_kilograms.len() != k
            || s.cumulative_outgoing.low_kilograms.len() != k
            || s.cumulative_incoming.high_kilograms.len() != n
            || s.cumulative_incoming.low_kilograms.len() != n
            || connections.len() != k
        {
            return Err("Invalid parent outgoing version or ledger shape.".into());
        }
        let mut expected = leaf_spill::Components::zero(n);
        for g in 0..k {
            let flow = s.cumulative_outgoing.stock(g)?;
            if flow.high == 0. {
                continue;
            }
            if cp.elapsed_seconds == 0 || history.merge_counts[g] == 0 {
                return Err("Parent departure has no historical owner.".into());
            }
            let (_, route, head) = self.outlet(g)?;
            let destination = leaf_spill::Layout::destination_region(&route.destination);
            if self.by_terminal[destination] == Some(g) {
                return Err("Parent departure re-enters its source.".into());
            }
            match route.destination {
                Destination::ClosedTerminal { terminal_region }
                    if !leaves.has_terminal(terminal_region) =>
                {
                    return Err("Parent departure misses its leaf receiver.".into());
                }
                Destination::ReferenceBody {
                    body_id,
                    contact_region,
                } if pool.by_region[contact_region].is_none_or(|b| pool.ids[b] != body_id)
                    || !leaves.reference_below(head) =>
                {
                    return Err(
                        "Parent departure has an invalid/backpressured reference contact.".into(),
                    );
                }
                _ => {}
            }
            expected.credit(destination, flow.high)?;
            expected.credit(destination, flow.low)?;
        }
        let mut maximum = (0., 0);
        for r in 0..n {
            let actual = s.cumulative_incoming.stock(r)?;
            let target = expected.stock(r)?;
            if (target.high == 0. && actual.high != 0.)
                || (cp.elapsed_seconds == 0 && actual.high != 0.)
            {
                return Err("Parent incoming flow has no geographic source.".into());
            }
            let residual = total_mass(&[actual.high - target.high, actual.low - target.low]);
            let relative = residual.abs() / actual.high.max(target.high).max(1.);
            if !relative.is_finite() {
                return Err("Nonfinite parent spill geographic identity.".into());
            }
            if relative > maximum.0 {
                maximum = (relative, r);
            }
        }
        Ok(maximum)
    }
}
