# Bounded dry terrain preparation

Status: implemented and tested as an independent native kernel. It is **not** connected to the generated-world pipeline, desktop, recipe, water fitting, drainage, or basin analysis. Current worlds and fingerprints are unchanged. It does not represent elapsed geological time or climate-driven erosion.

The immediate purpose is to establish a material-accounted bed-change operation before changing the versioned generator. The kernel accepts a closed spherical region graph, physical edge distances, reference-sphere region areas, initial bed elevations, and at most 16 preparation passes. It returns a prepared elevation field, gross eroded and deposited thickness per region, gross transported reference-area-equivalent volume, and requested/applied pass counts. It does not mutate its input.

For one undirected edge with high region `a`, low region `b`, distance `d`, areas `A_a` and `A_b`, and snapshot heights `h_a > h_b`, the candidate transport volume is:

```text
excess = max(0, h_a - h_b - 0.004 × d)
candidate_volume = 0.25 × excess / (1/A_a + 1/A_b)
```

The dimensionless `0.004` threshold is a **mesh-scale smoothing heuristic**, not a measured talus angle. Initial tests at subdivisions 2–4 found generated maximum neighbor slopes of approximately 0.0026–0.0090 in three fixed worlds; a 0.01 threshold did nothing in all three. The coefficients require future sensitivity and resolution studies before being considered a final planet model.

Each pass evaluates all edges from the same beginning-of-pass heights. Candidate volumes are summed by donor and receiver. A transfer is then scaled by the smaller of its donor's and receiver's available factors, so gross removal and gross deposition each remain at most 100 m per region per pass. The identical scaled volume is subtracted from one region and added to the other after division by their areas. This preserves the reference-area material ledger within floating-point error. Material is deposited in the same pass; there is no untracked suspended stock or export. Pass order and edge traversal are deterministic for a fixed graph.

This is not a physical sediment model: bed elevation is used as a surrogate material column with uniform density, areas ignore topographic slope, and there is no regolith, grain size, moisture, vegetation, or tectonic replenishment. A pass has no year/day duration. The routine validates finite heights, positive areas/distances, bounded pass count, and reciprocal edges with matching distances. Invalid inputs reject without modifying the supplied bed.

Directed tests cover an analytically known three-region transfer, no movement below threshold, simultaneous per-region caps, malformed graphs and non-finite fields, deterministic repeated runs, area/datum/relabeling invariance, and area-weighted material accounting on several seeded generated worlds. These tests demonstrate a conservative operation, **not** satisfactory erosion patterns or resolution convergence. Before product integration, a new model/recipe version must explicitly place preparation before initial water fitting, update elevation provenance and wire fields, recompute water/drainage/basins from the changed bed, and compare seed ensembles at multiple resolutions. Later changes to an already wet world must instead retain its resolved water volume; they cannot silently refit a coverage target.
