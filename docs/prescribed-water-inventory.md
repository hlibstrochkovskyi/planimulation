# Bounded prescribed-water inventory

Status: opt-in native-core state and version-2 checkpoint for prescribed runoff with bounded geographic spill and merge. A [manual desktop view](prescribed-water-desktop.md) can drive, display, save, restore, and continue supported steps. It is not a general dynamic water solver or a climate model. Version-1 checkpoints are not silently migrated.

## Accounting contract

The state imports the [exact initial-water accounting](exact-initial-accounting.md) for a generated recipe, then applies [prescribed runoff routing](prescribed-runoff-routing.md) on that world's static receiver graph. Each canonical terminal's integer volume is assigned to its initial active basin branch. Existing oceans and closed dry sinks remain accounted for; no terminal input is dropped. The physical ledger uses `i128` units of `2^-56 m³`, with decimal strings in JSON checkpoints:

```text
sum(active branch stock) = exact imported initial total + accepted prescribed input
```

Capacities are reconstructed from the same represented regional `area × depth` contributions as the initial import, evaluated at each branch's spill level. When one directly supplied branch reaches a threshold, its stock saturates exactly. Excess moves only through an existing sill contact to a unique nonfull active receiver. If every immediate child of a merge is active and exactly full, their complete integer stocks become one parent stock; any remaining excess stays with the parent and may continue upward. This transition is finite and atomic. Inputs below the displayed level's resolution remain owned by their receiving branch.

The policy deliberately rejects a step with multiple direct spill sources or multiple open geographic receivers. It never assigns a tied remainder according to region or branch ID. It also rejects an overfull source with no open passage. These are supported-case boundaries, not a claim that such runoff is physically impossible. Direct input to other branches is aggregated before testing the one spill source; arbitrary concurrent flow rates and proportional allocation are not implemented.

The optional level query inverts the existing basin storage curve approximately for inspection. It never writes the exact stock or controls routing. A level may appear unchanged after a small accepted input.

## Checkpoint and limits

The checkpoint records its own version, the complete versioned initial import and recipe, current exclusive frontier IDs and exact stocks, accepted-input total, and a step count bounded to JSON's exact integer range. Restoration regenerates the pinned world and rejects a mismatched origin, noncanonical decimal units, overlapping or incomplete frontiers, stock outside basin bounds, or a broken total ledger. The checkpoint is an exact same-build continuation of accepted states, not a cryptographic proof that an arbitrary edited but balanced JSON state was historically reachable. Failed inputs leave the complete checkpoint unchanged. Directed tests cover dry sinks, a fully wet closed planet, small exact ocean inputs, a generated-world spill into a neighboring leaf followed by a merge, JSON replay before and after that merge, simultaneous-source refusal, and atomic failures.

This is **prescribed terminal delivery and threshold redistribution**, not a timed flow law. The downhill receiver graph is frozen at the initially generated world; expanding water can alter the true shoreline, but this increment does not recompute drainage or travel time. Regional depth and water-body IDs are now derived for display from the exact stocks without feeding back into them. Its approximate branch level is not a physically equilibrated water surface after a finite interval. It cannot withdraw water or model evaporation. Its exact conservation result applies to accepted bounded steps, not to arbitrary simultaneous forcing. The next physical integration must refresh or replace routes when wet topology changes. General simultaneous allocation and long-running forcing remain open in [water accounting](water-accounting-contract.md).
