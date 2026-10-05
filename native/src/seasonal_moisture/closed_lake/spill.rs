//! Static first-connection certificates, not a grant policy or hydraulic solver.
use super::Layout;
use crate::{World, spill_connections::SpillConnections};
use serde::Serialize;

pub const ANALYSIS_VERSION: &str = "closed-leaf-spill-connections-1";

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Destination {
    ReferenceBody {
        #[serde(rename = "bodyId")]
        body_id: u32,
        /// Actual wet contact, not the body's canonical minimum-ID display slot.
        #[serde(rename = "contactRegion")]
        contact_region: usize,
    },
    ClosedTerminal {
        #[serde(rename = "terminalRegion")]
        terminal_region: usize,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Route {
    pub receiving_branch: usize,
    pub sill_plateau: usize,
    /// Existing canonical source-contact -> sill-only -> receiver-contact path.
    pub sill_passage_regions: Vec<u32>,
    /// Inclusive receiver contact -> static runoff terminal; no flow is applied.
    pub downhill_regions: Vec<u32>,
    pub destination: Destination,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Connection {
    pub terminal_region: usize,
    pub basin_node: usize,
    pub minimum_bed_meters: f64,
    pub first_connection_level_meters: Option<f64>,
    pub first_connection_height_above_minimum_meters: Option<f64>,
    pub capacity_cubic_meters: Option<f64>,
    pub routes: Vec<Route>,
    /// Only one geometric passage+terminal certificate. Does not authorize flow.
    pub has_unique_route: bool,
}

fn descend(
    world: &World,
    start: usize,
    source_branch: usize,
    include_children: bool,
) -> Result<(Vec<u32>, Destination), String> {
    let n = world.surface.areas.len();
    let mut region = start;
    let mut path = Vec::new();
    for _ in 0..n {
        if region >= n
            || world.basins.region_nodes()[region] == source_branch
            || (include_children
                && world.basins.nodes()[world.basins.region_nodes()[region]].parent
                    == Some(source_branch))
        {
            return Err("First spill re-enters its source or leaves the surface.".into());
        }
        path.push(region as u32);
        let receiver = world.drainage.receivers[region] as usize;
        if world.water.body_ids[region] > 0 && receiver != region {
            return Err("First-spill wet contact is not a terminal receiver.".into());
        }
        if receiver == region {
            let destination = if world.water.body_ids[region] > 0 {
                Destination::ReferenceBody {
                    body_id: world.water.body_ids[region],
                    contact_region: region,
                }
            } else {
                Destination::ClosedTerminal {
                    terminal_region: region,
                }
            };
            return Ok((path, destination));
        }
        if receiver >= n
            || world.terrain.elevation[receiver] > world.terrain.elevation[region]
            || (world.terrain.elevation[receiver] == world.terrain.elevation[region]
                && world.drainage.flat_steps[receiver] >= world.drainage.flat_steps[region])
        {
            return Err("First-spill descent is not a decreasing static drainage path.".into());
        }
        if !world.surface.neighbors
            [world.surface.offsets[region] as usize..world.surface.offsets[region + 1] as usize]
            .contains(&(receiver as u32))
        {
            return Err("First-spill descent jumps a non-neighbor edge.".into());
        }
        region = receiver;
    }
    Err("First-spill descent exceeded the acyclic surface bound.".into())
}

/// On the generated, validated reference graph, retain every direct receiving
/// branch/plateau candidate. Multiple candidates remain unresolved alternatives.
/// This is an inspection/preparation API, not per-tick routing.
pub fn first_connections(world: &World) -> Result<Vec<Connection>, String> {
    let layout = Layout::from_world(world)?;
    let geometry = checked_geometry(world)?;
    layout
        .lakes()
        .iter()
        .map(|lake| {
            let source = lake.basin_node();
            let node = &world.basins.nodes()[source];
            let routes = branch_routes(world, &geometry, source, false)?;
            Ok(Connection {
                terminal_region: lake.terminal_region(),
                basin_node: source,
                minimum_bed_meters: node.birth_level_meters,
                first_connection_level_meters: node.spill_level_meters,
                first_connection_height_above_minimum_meters: node
                    .spill_level_meters
                    .map(|h| h - node.birth_level_meters),
                capacity_cubic_meters: lake.capacity_cubic_meters(),
                has_unique_route: routes.len() == 1,
                routes,
            })
        })
        .collect()
}

pub(crate) fn checked_geometry(world: &World) -> Result<SpillConnections, String> {
    if world.drainage.flat_steps.len() != world.surface.areas.len() {
        return Err("First-spill flat-routing fields differ.".into());
    }
    world
        .drainage
        .route_runoff_units(&vec![0; world.surface.areas.len()])?;
    let geometry = SpillConnections::build(&world.surface, &world.terrain.elevation)?;
    if geometry.basins() != &world.basins {
        return Err("First-spill hierarchy does not match generated lake geometry.".into());
    }
    Ok(geometry)
}

/// Parent use is deliberately limited to immediate minimum-leaf children.
pub(crate) fn branch_routes(
    world: &World,
    geometry: &SpillConnections,
    source: usize,
    include_children: bool,
) -> Result<Vec<Route>, String> {
    let mut routes = Vec::new();
    for receiver in geometry.receivers(source)? {
        for plateau in receiver.plateaus {
            let passage = geometry.passage(source, receiver.branch, plateau)?;
            let entry = *passage.regions.last().ok_or("Empty first-spill passage.")? as usize;
            let (downhill_regions, destination) = descend(world, entry, source, include_children)?;
            routes.push(Route {
                receiving_branch: receiver.branch,
                sill_plateau: plateau,
                sill_passage_regions: passage.regions,
                downhill_regions,
                destination,
            });
        }
    }
    if world.basins.nodes()[source].parent.is_some() && routes.is_empty() {
        return Err("Non-root closed leaf has no first-connection passage.".into());
    }
    Ok(routes)
}
