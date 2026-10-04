//! Bounded fast fill/spill on exclusive leaves with optional model-11 parent handoff.
//! No discharge law, general parent frontier or concurrent overflow allocation.
use super::{Checkpoint as SeasonalCheckpoint, closed_lake, merged_lake, reference_pool};
use crate::{World, moisture_transport::total_mass, surface_water::CompensatedStock};
use closed_lake::spill::{Connection, Destination, Route};
use serde::{Deserialize, Serialize};

pub const MODEL_VERSION: &str = "closed-leaf-spill-transfer-1";
const MAX_EDGE_GRANTS_PER_RESOLUTION: usize = 4096;

/// A normalized pair at each region. Flows and owned queues use separate arrays.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Components {
    pub high_kilograms: Vec<f64>,
    pub low_kilograms: Vec<f64>,
}
impl Components {
    fn zero(n: usize) -> Self {
        Self {
            high_kilograms: vec![0.; n],
            low_kilograms: vec![0.; n],
        }
    }
    pub(super) fn stock(&self, r: usize) -> Result<CompensatedStock, String> {
        CompensatedStock::new(self.high_kilograms[r], self.low_kilograms[r], f64::MAX)
    }
    pub(super) fn set(&mut self, r: usize, stock: CompensatedStock) {
        self.high_kilograms[r] = stock.high;
        self.low_kilograms[r] = stock.low;
    }
    pub(super) fn credit(&mut self, r: usize, amount: f64) -> Result<(), String> {
        let mut stock = self.stock(r)?;
        let before = (stock.high, stock.low);
        stock.credit(amount)?;
        if amount != 0. && before == (stock.high, stock.low) {
            return Err("Leaf spill input is below its owned pair's resolution.".into());
        }
        self.set(r, stock);
        Ok(())
    }
    pub(super) fn total(&self) -> f64 {
        total_mass(
            &self
                .high_kilograms
                .iter()
                .chain(&self.low_kilograms)
                .copied()
                .collect::<Vec<_>>(),
        )
    }
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Checkpoint {
    pub model_version: String,
    /// Owned liquid waiting for capacity-bounded application, not lake geometry.
    pub pending_input: Components,
    /// Gross edge flows only; neither ledger is a spendable inventory.
    pub cumulative_outgoing: Components,
    pub cumulative_incoming: Components,
}
impl Checkpoint {
    pub(super) fn zero(n: usize) -> Self {
        Self {
            model_version: MODEL_VERSION.into(),
            pending_input: Components::zero(n),
            cumulative_outgoing: Components::zero(n),
            cumulative_incoming: Components::zero(n),
        }
    }
}

/// Accepted direct edge transfer, including pass-through of a full lower leaf.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Event {
    pub input_terminal_region: usize,
    pub source_terminal_region: usize,
    pub route: Route,
    pub kilograms: f64,
}

pub(super) fn append_events(target: &mut Vec<Event>, events: Vec<Event>) -> Result<(), String> {
    if events.len() > MAX_EDGE_GRANTS_PER_RESOLUTION.saturating_sub(target.len()) {
        return Err("Leaf spill exceeded the 4096 caller-interval edge-grant bound; use a shorter interval.".into());
    }
    target.extend(events);
    Ok(())
}

pub(super) struct Layout {
    connections: Vec<Connection>,
    by_terminal: Vec<Option<usize>>,
    reference_level: f64,
}

fn empty(stock: CompensatedStock) -> bool {
    stock.high == 0. && stock.low == 0.
}
fn full(stock: CompensatedStock, cap: f64) -> bool {
    stock.high == cap && stock.low == 0.
}

/// Move actual scalar grants, retaining tails, without snapping either endpoint.
/// Four passes bound decomposition of a pair and a compensated capacity deficit.
pub(super) fn fill(
    donor: &mut CompensatedStock,
    recipient: &mut CompensatedStock,
    cap: f64,
) -> Result<Vec<f64>, String> {
    CompensatedStock::new(donor.high, donor.low, f64::MAX)?;
    CompensatedStock::new(recipient.high, recipient.low, cap)?;
    let before = (*donor, *recipient);
    let mut grants = Vec::new();
    for _ in 0..4 {
        if empty(*donor) || full(*recipient, cap) {
            break;
        }
        let old = (recipient.high, recipient.low);
        let old_donor = (donor.high, donor.low);
        let grant = recipient.deposit(donor.available(), cap);
        if !grant.is_finite()
            || grant <= 0.
            || old == (recipient.high, recipient.low)
            || donor.withdraw(grant) != grant
            || old_donor == (donor.high, donor.low)
        {
            return Err(
                "Leaf spill transfer cannot make representable capacity-safe progress.".into(),
            );
        }
        CompensatedStock::new(donor.high, donor.low, f64::MAX)?;
        CompensatedStock::new(recipient.high, recipient.low, cap)?;
        grants.push(grant);
    }
    if !empty(*donor) && !full(*recipient, cap) {
        return Err("Leaf spill transfer exceeded its four-grant arithmetic bound.".into());
    }
    let amount = total_mass(&grants);
    for residual in [
        total_mass(&[donor.high - before.0.high, donor.low - before.0.low, amount]),
        total_mass(&[
            recipient.high - before.1.high,
            recipient.low - before.1.low,
            -amount,
        ]),
    ] {
        if !residual.is_finite()
            || residual.abs()
                > 32. * f64::EPSILON * before.0.high.max(before.1.high).max(amount).max(1.)
        {
            return Err("Leaf spill transfer exceeds its local arithmetic tolerance.".into());
        }
    }
    Ok(grants)
}

impl Layout {
    pub fn from_world(world: &World, geometry: &closed_lake::Layout) -> Result<Self, String> {
        let connections = closed_lake::spill::first_connections(world)?;
        if connections.len() != geometry.lakes().len() {
            return Err("Leaf spill geometry differs.".into());
        }
        let mut by_terminal = vec![None; world.surface.areas.len()];
        for (b, (connection, lake)) in connections.iter().zip(geometry.lakes()).enumerate() {
            if connection.terminal_region != lake.terminal_region()
                || connection.basin_node != lake.basin_node()
                || connection.capacity_cubic_meters != lake.capacity_cubic_meters()
                || by_terminal[connection.terminal_region].replace(b).is_some()
            {
                return Err("Leaf spill certificate differs from its exclusive owner.".into());
            }
        }
        Ok(Self {
            connections,
            by_terminal,
            reference_level: world.water.level_meters,
        })
    }

    fn capacity(&self, b: usize) -> Result<f64, String> {
        let cap = self.connections[b]
            .capacity_cubic_meters
            .map_or(f64::MAX, |v| {
                v * super::WATER_DENSITY_KILOGRAMS_PER_CUBIC_METER
            });
        if !cap.is_finite() || cap <= 0. {
            return Err("Invalid leaf spill capacity.".into());
        }
        Ok(cap)
    }
    fn liquid(cp: &SeasonalCheckpoint, r: usize, cap: f64) -> Result<CompensatedStock, String> {
        CompensatedStock::new(
            cp.terminal_water_kilograms[r],
            cp.terminal_low_kilograms.as_ref().unwrap()[r],
            cap,
        )
    }
    fn set_liquid(cp: &mut SeasonalCheckpoint, r: usize, stock: CompensatedStock) {
        cp.terminal_water_kilograms[r] = stock.high;
        cp.terminal_low_kilograms.as_mut().unwrap()[r] = stock.low;
    }
    fn destination_region(destination: &Destination) -> usize {
        match *destination {
            Destination::ClosedTerminal { terminal_region } => terminal_region,
            Destination::ReferenceBody { contact_region, .. } => contact_region,
        }
    }
    fn route(&self, b: usize) -> Result<&Route, String> {
        let routes = &self.connections[b].routes;
        if routes.len() != 1 {
            return Err(
                "Leaf spill needs one unique geographic route; junction allocation is unsupported."
                    .into(),
            );
        }
        Ok(&routes[0])
    }

    /// Provisional only: the enclosing seasonal interval supplies atomic commit.
    pub fn resolve(
        &self,
        cp: &mut SeasonalCheckpoint,
        pool: &reference_pool::Layout,
        merge: Option<&merged_lake::Layout>,
    ) -> Result<Vec<Event>, String> {
        if let Some(m) = merge {
            m.settle(cp)?;
        }
        // All local inputs fill their own leaf first, before selecting overflow.
        for (b, connection) in self.connections.iter().enumerate() {
            let r = connection.terminal_region;
            if merge.is_some_and(|m| m.terminal_active(cp, r)) {
                continue;
            }
            let mut input = cp
                .leaf_spill_state
                .as_ref()
                .unwrap()
                .pending_input
                .stock(r)?;
            let cap = self.capacity(b)?;
            let mut liquid = Self::liquid(cp, r, cap)?;
            fill(&mut input, &mut liquid, cap)?;
            cp.leaf_spill_state
                .as_mut()
                .unwrap()
                .pending_input
                .set(r, input);
            Self::set_liquid(cp, r, liquid);
        }
        if let Some(m) = merge {
            m.settle(cp)?;
        }
        let sources = self
            .connections
            .iter()
            .enumerate()
            .filter_map(|(b, c)| {
                let input = &cp.leaf_spill_state.as_ref().unwrap().pending_input;
                (input.high_kilograms[c.terminal_region] != 0.
                    || input.low_kilograms[c.terminal_region] != 0.)
                    .then_some(b)
            })
            .collect::<Vec<_>>();
        if sources.len() > 1 {
            return Err(
                "Concurrent leaf spill sources require a shared event/merge policy.".into(),
            );
        }
        let Some(&source) = sources.first() else {
            return Ok(Vec::new());
        };
        let root = self.connections[source].terminal_region;
        let mut donor = cp
            .leaf_spill_state
            .as_ref()
            .unwrap()
            .pending_input
            .stock(root)?;
        let mut path = Vec::<usize>::new();
        let mut visited = vec![false; self.connections.len()];
        let mut b = source;
        let mut events = Vec::new();
        for _ in 0..self.connections.len() {
            if visited[b] {
                return Err("Leaf spill re-entered a visited owner.".into());
            }
            if path.len() >= MAX_EDGE_GRANTS_PER_RESOLUTION {
                return Err("Leaf spill exceeded its 4096 visited-edge work bound.".into());
            }
            visited[b] = true;
            path.push(b);
            let route = self.route(b)?;
            let crest = self.connections[b]
                .first_connection_level_meters
                .ok_or("Leaf spill needs a resolved absolute sill level.")?;
            let destination = Self::destination_region(&route.destination);
            let grants = match route.destination {
                Destination::ClosedTerminal { terminal_region } => {
                    if merge.is_some_and(|m| m.terminal_active(cp, terminal_region)) {
                        return Err("Leaf spill into an active merged parent requires a receiving-frontier policy.".into());
                    }
                    let receiver = self.by_terminal[terminal_region]
                        .ok_or("Leaf spill receiver has no exclusive leaf owner.")?;
                    let receiving_crest = self.connections[receiver]
                        .first_connection_level_meters
                        .ok_or("Leaf spill recipient has no resolved capacity-bounded outlet.")?;
                    if receiving_crest > crest {
                        return Err(
                            "Leaf spill recipient capacity lies above the supplying head.".into(),
                        );
                    }
                    let cap = self.capacity(receiver)?;
                    let mut liquid = Self::liquid(cp, terminal_region, cap)?;
                    let grants = fill(&mut donor, &mut liquid, cap)?;
                    Self::set_liquid(cp, terminal_region, liquid);
                    grants
                }
                Destination::ReferenceBody {
                    body_id,
                    contact_region,
                } => {
                    if !self.reference_level.is_finite() || self.reference_level >= crest {
                        return Err("Leaf spill reference recipient is backpressured under the fixed-level assumption.".into());
                    }
                    let body = pool.by_region[contact_region]
                        .ok_or("Leaf spill misses a reference contact.")?;
                    if pool.ids[body] != body_id {
                        return Err("Leaf spill reference contact has the wrong body.".into());
                    }
                    let mut liquid = CompensatedStock::new(
                        cp.reference_body_high_kilograms.as_ref().unwrap()[body],
                        cp.reference_body_low_kilograms.as_ref().unwrap()[body],
                        f64::MAX,
                    )?;
                    let grants = fill(&mut donor, &mut liquid, f64::MAX)?;
                    cp.reference_body_high_kilograms.as_mut().unwrap()[body] = liquid.high;
                    cp.reference_body_low_kilograms.as_mut().unwrap()[body] = liquid.low;
                    if !empty(donor) {
                        return Err(
                            "Leaf spill reference stock reached its arithmetic ceiling.".into()
                        );
                    }
                    grants
                }
            };
            for grant in grants {
                let ledger = cp.leaf_spill_state.as_mut().unwrap();
                // Full intermediate leaves record incoming/outgoing, not copied stock.
                for &edge in &path {
                    if events.len() >= MAX_EDGE_GRANTS_PER_RESOLUTION {
                        return Err("Leaf spill exceeded its 4096 edge-grant work bound.".into());
                    }
                    let from = self.connections[edge].terminal_region;
                    let edge_route = self.route(edge)?;
                    let to = Self::destination_region(&edge_route.destination);
                    ledger.cumulative_outgoing.credit(from, grant)?;
                    ledger.cumulative_incoming.credit(to, grant)?;
                    events.push(Event {
                        input_terminal_region: root,
                        source_terminal_region: from,
                        route: edge_route.clone(),
                        kilograms: grant,
                    });
                }
            }
            cp.leaf_spill_state
                .as_mut()
                .unwrap()
                .pending_input
                .set(root, donor);
            if let Some(m) = merge {
                m.settle_at(cp, root)?;
                if destination != root {
                    m.settle_at(cp, destination)?;
                }
                if m.terminal_active(cp, root) {
                    return Ok(events);
                }
                if !empty(donor) && m.terminal_active(cp, destination) {
                    return Err(
                        "Remaining external spill into a newly merged parent is unsupported."
                            .into(),
                    );
                }
            }
            if empty(donor) {
                return Ok(events);
            }
            let receiver = self.by_terminal[destination]
                .ok_or("Remaining leaf spill input has no bounded recipient.")?;
            let next_crest = self.connections[receiver]
                .first_connection_level_meters
                .ok_or("Full receiving leaf has no resolved outlet.")?;
            if next_crest >= crest {
                return Err(
                    "Full receiving leaf requires backpressure/parent merge, not another spill."
                        .into(),
                );
            }
            b = receiver;
        }
        Err("Leaf spill exceeded the acyclic leaf bound.".into())
    }

    /// Independent edge provenance, separate from each inventory identity.
    pub fn validate(
        &self,
        cp: &SeasonalCheckpoint,
        pool: &reference_pool::Layout,
    ) -> Result<(f64, usize), String> {
        let ledger = cp
            .leaf_spill_state
            .as_ref()
            .ok_or("Missing leaf spill checkpoint.")?;
        let n = self.by_terminal.len();
        if ledger.model_version != MODEL_VERSION {
            return Err("Unsupported leaf spill accounting version.".into());
        }
        for components in [
            &ledger.pending_input,
            &ledger.cumulative_outgoing,
            &ledger.cumulative_incoming,
        ] {
            if components.high_kilograms.len() != n || components.low_kilograms.len() != n {
                return Err("Invalid leaf spill component shape.".into());
            }
            for r in 0..n {
                let stock = components.stock(r)?;
                if cp.elapsed_seconds == 0 && !empty(stock) {
                    return Err("Nonzero initial leaf spill account.".into());
                }
            }
        }
        let mut expected = Components::zero(n);
        for r in 0..n {
            if self.by_terminal[r].is_none()
                && (!empty(ledger.pending_input.stock(r)?)
                    || !empty(ledger.cumulative_outgoing.stock(r)?))
            {
                return Err("Leaf spill stock or outgoing flow has the wrong owner.".into());
            }
            let outgoing = ledger.cumulative_outgoing.stock(r)?;
            if !empty(outgoing) {
                let b = self.by_terminal[r].unwrap();
                let route = self.route(b)?;
                let destination = Self::destination_region(&route.destination);
                match route.destination {
                    Destination::ClosedTerminal { .. }
                        if self.by_terminal[destination].is_none() =>
                    {
                        return Err("Leaf spill flow misses its owner.".into());
                    }
                    Destination::ReferenceBody { body_id, .. }
                        if pool.by_region[destination].is_none_or(|b| pool.ids[b] != body_id) =>
                    {
                        return Err("Leaf spill flow misses its reference body.".into());
                    }
                    _ => {}
                }
                expected.credit(destination, outgoing.high)?;
                expected.credit(destination, outgoing.low)?;
            }
        }
        let mut maximum = (0., 0);
        for r in 0..n {
            let actual = ledger.cumulative_incoming.stock(r)?;
            let target = expected.stock(r)?;
            if empty(target) && !empty(actual) {
                return Err("Leaf spill incoming flow has no geographic source.".into());
            }
            let residual = total_mass(&[actual.high - target.high, actual.low - target.low]);
            let relative = residual.abs() / actual.high.max(target.high).max(1.);
            if !relative.is_finite() {
                return Err("Nonfinite leaf spill graph identity.".into());
            }
            if relative > maximum.0 {
                maximum = (relative, r);
            }
        }
        Ok(maximum)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bounded_fill_matches_dyadic_oracle_and_retains_signed_tails() {
        for input in 0..256 {
            for existing in 0..128 {
                let mut donor = CompensatedStock::new(input as f64 / 16., 0., f64::MAX).unwrap();
                let mut recipient = CompensatedStock::new(existing as f64 / 16., 0., 8.).unwrap();
                let grants = fill(&mut donor, &mut recipient, 8.).unwrap();
                let exact = input.min(128 - existing) as f64 / 16.;
                assert_eq!(total_mass(&grants), exact);
                assert_eq!(donor.high, input as f64 / 16. - exact);
                assert_eq!(recipient.high, existing as f64 / 16. + exact);
                assert_eq!(donor.low, 0.);
                assert_eq!(recipient.low, 0.);
            }
        }
        for tail in [-2_f64.powi(-54), 2_f64.powi(-54)] {
            let mut donor = CompensatedStock::new(1., tail, f64::MAX).unwrap();
            let mut recipient = CompensatedStock::new(0., 0., 2.).unwrap();
            fill(&mut donor, &mut recipient, 2.).unwrap();
            assert!(empty(donor));
            assert_eq!((recipient.high, recipient.low), (1., tail));
        }
        let mut donor = CompensatedStock::new(1., 0., f64::MAX).unwrap();
        let mut recipient = CompensatedStock::new(2., -2_f64.powi(-54), 2.).unwrap();
        fill(&mut donor, &mut recipient, 2.).unwrap();
        assert!(full(recipient, 2.));
        assert_eq!((donor.high, donor.low), (1., -2_f64.powi(-54)));
    }
    #[test]
    fn subnormal_input_moves_and_unresolvable_credit_refuses() {
        let tiny = f64::from_bits(1);
        let mut donor = CompensatedStock::new(tiny, 0., f64::MAX).unwrap();
        let mut recipient = CompensatedStock::new(0., 0., 1.).unwrap();
        fill(&mut donor, &mut recipient, 1.).unwrap();
        assert!(empty(donor));
        assert_eq!(recipient.high, tiny);
        let mut donor = CompensatedStock::new(1e-320, 0., f64::MAX).unwrap();
        let mut recipient = CompensatedStock::new(5e14, 1e-300, f64::MAX).unwrap();
        assert!(fill(&mut donor, &mut recipient, f64::MAX).is_err());
        // A tiny receiving deficit must not manufacture water when subtracting
        // its grant cannot change a much larger donor pair.
        let mut donor = CompensatedStock::new(5e14, 1e-300, f64::MAX).unwrap();
        let mut recipient = CompensatedStock::new(1., -1e-320, 1.).unwrap();
        assert!(fill(&mut donor, &mut recipient, 1.).is_err());
    }

    fn chain(
        heights: &[f64],
        bodies: &[u32],
    ) -> (Layout, SeasonalCheckpoint, reference_pool::Layout) {
        // A synthetic reciprocal chain tests topology, not a restorable recipe.
        let recipe = serde_json::from_str(include_str!(
            "../../../docs/scenarios/seasonal-temperature.json"
        ))
        .unwrap();
        let mut world = World::generate(recipe).unwrap();
        let model = super::super::Model::from_world(
            &world,
            Default::default(),
            Default::default(),
            Default::default(),
        )
        .unwrap();
        let mut cp = model.initial_state().checkpoint();
        let n = heights.len();
        let mut neighbors = Vec::new();
        let mut offsets = vec![0];
        for r in 0..n {
            if r > 0 {
                neighbors.push((r - 1) as u32);
            }
            if r + 1 < n {
                neighbors.push((r + 1) as u32);
            }
            offsets.push(neighbors.len() as u32);
        }
        world.surface = crate::Surface {
            centers: vec![],
            faces: vec![],
            distances: vec![1.; neighbors.len()],
            offsets,
            neighbors,
            areas: vec![1.; n],
            boundary_offsets: vec![],
            boundaries: vec![],
        };
        world.terrain.elevation = heights.to_vec();
        world.water.body_ids = bodies.to_vec();
        world.water.level_meters = 0.;
        world.basins = crate::basins::Basins::build(&world.surface, heights).unwrap();
        world.drainage = crate::drainage::Drainage::build(&world.surface, heights, bodies);
        let geometry = closed_lake::Layout::from_world(&world).unwrap();
        let layout = Layout::from_world(&world, &geometry).unwrap();
        let pool = reference_pool::Layout::new(bodies);
        cp.elapsed_seconds = 1;
        cp.terminal_water_kilograms = vec![0.; n];
        cp.terminal_low_kilograms = Some(vec![0.; n]);
        cp.reference_body_high_kilograms = Some(vec![0.; pool.ids.len()]);
        cp.reference_body_low_kilograms = Some(vec![0.; pool.ids.len()]);
        cp.leaf_spill_state = Some(Checkpoint::zero(n));
        (layout, cp, pool)
    }

    #[test]
    fn same_crest_receiver_accepts_available_room_but_full_receiver_needs_merge() {
        let (layout, mut cp, pool) = chain(&[0., 2., 1.], &[0; 3]);
        cp.leaf_spill_state
            .as_mut()
            .unwrap()
            .pending_input
            .credit(0, 2500.)
            .unwrap();
        layout.resolve(&mut cp, &pool, None).unwrap();
        assert_eq!(cp.terminal_water_kilograms, [2000., 0., 500.]);
        let mut provisional = cp.clone();
        provisional
            .leaf_spill_state
            .as_mut()
            .unwrap()
            .pending_input
            .credit(0, 501.)
            .unwrap();
        assert!(
            layout
                .resolve(&mut provisional, &pool, None)
                .unwrap_err()
                .contains("parent merge")
        );
        assert_eq!(cp.terminal_water_kilograms, [2000., 0., 500.]);
    }

    #[test]
    fn ambiguous_junction_and_reference_backpressure_are_explicit_refusals() {
        let (layout, mut cp, pool) = chain(&[0., 4., 1., 4., 2.], &[0; 5]);
        cp.leaf_spill_state
            .as_mut()
            .unwrap()
            .pending_input
            .credit(2, 3001.)
            .unwrap();
        assert!(
            layout
                .resolve(&mut cp, &pool, None)
                .unwrap_err()
                .contains("unique geographic route")
        );
        let (mut layout, mut cp, pool) = chain(&[0., 1., 2., 0.], &[1, 1, 0, 0]);
        cp.leaf_spill_state
            .as_mut()
            .unwrap()
            .pending_input
            .credit(3, 2001.)
            .unwrap();
        layout.reference_level = 2.;
        assert!(
            layout
                .resolve(&mut cp, &pool, None)
                .unwrap_err()
                .contains("backpressured")
        );
    }

    #[test]
    fn a_closed_root_retains_input_without_fabricating_a_drain() {
        let (layout, mut cp, pool) = chain(&[0.], &[0]);
        cp.leaf_spill_state
            .as_mut()
            .unwrap()
            .pending_input
            .credit(0, 1234.)
            .unwrap();
        assert!(layout.resolve(&mut cp, &pool, None).unwrap().is_empty());
        assert_eq!(cp.terminal_water_kilograms, [1234.]);
        assert_eq!(
            cp.leaf_spill_state.as_ref().unwrap().pending_input.total(),
            0.
        );
    }

    #[test]
    fn caller_event_bound_refuses_without_truncating_or_changing_prior_events() {
        let event = Event {
            input_terminal_region: 0,
            source_terminal_region: 0,
            route: Route {
                receiving_branch: 1,
                sill_plateau: 0,
                sill_passage_regions: vec![0, 1, 2],
                downhill_regions: vec![2],
                destination: Destination::ClosedTerminal { terminal_region: 2 },
            },
            kilograms: 1.,
        };
        let mut events = Vec::new();
        append_events(&mut events, vec![event.clone(); 4096]).unwrap();
        let before = events.clone();
        assert!(
            append_events(&mut events, vec![event])
                .unwrap_err()
                .contains("caller-interval")
        );
        assert_eq!(events, before);
    }
}
