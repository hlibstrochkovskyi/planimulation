# Prescribed runoff routing

Status: implemented as a pure native-core operation on the existing static drainage graph. This is an input-routing stage, not lake storage, flood dynamics, a climate-derived runoff model, or desktop playback.

## Method and provenance

The first stage of [Barnes et al.'s Fill–Spill–Merge method](https://esurf.copernicus.org/articles/9/105/2021/) moves surface water down an acyclic receiver graph before redistributing it in a depression hierarchy. Planimulation already has a spherical, single-receiver drainage graph and a separate basin hierarchy. `Drainage::route_runoff_units` adapts that **topological accumulation idea** to its existing graph; no external source code is copied or vendored.

The input is one prescribed, nonnegative integer volume per region. The caller defines the unit; a future integration with exact initial-water accounting must use its declared unit consistently. In one topological pass, the operation records the volume through each region and aggregates final volume at either a closed dry sink or the canonical terminal of an existing water body. All terminals of one water body share the same output slot. There is no ocean discard: on this closed planet, ocean input remains a recorded terminal amount.

The exact integer budget is:

```text
sum(prescribed regional runoff) = sum(canonical terminal runoff)
```

Negative inputs, mismatched arrays, cycles, invalid terminal labels, and integer overflow reject without modifying the drainage graph or generated world. The operation is deterministic and has linear time and memory in the region count. Directed and generated-world tests compare it with independently traced terminal paths.

## Boundary of this increment

The routing operation alone does **not** change a lake or ocean. An opt-in [bounded prescribed-water inventory](prescribed-water-inventory.md) accepts its terminal amounts into active basin stocks and can transfer one unambiguous spill source across existing sill contacts, then merge full sibling stocks. Neither operation has elapsed time, discharge, precipitation conversion, soil loss, evaporation, flood waves, or a water-surface-gradient law. Static bed receivers remain a potential route, not proof of a dynamic flow rate. This is not the complete [Fill–Spill–Merge method](https://esurf.copernicus.org/articles/9/105/2021/) or a [finite-volume hydrodynamic solver](https://www.hec.usace.army.mil/confluence/rasdocs/ras1dtechref/6.2/theoretical-basis-for-one-dimensional-and-two-dimensional-hydrodynamic-calculations/2d-unsteady-flow-hydrodynamics). In particular, the published Fill–Spill–Merge ocean-as-sink convention cannot be copied into a closed planetary water budget.

The next integration must derive map/globe views from the changing frontier, then update routing when the wet topology changes. Simultaneous allocation and longer-run limits in [water accounting](water-accounting-contract.md) remain gates.
