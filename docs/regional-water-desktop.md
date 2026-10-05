# Regional surface water in the desktop

Status: implemented October 5, 2026, as a separately selected experimental mode.
This connects the existing [regional surface-flow model](regional-surface-flow.md)
to Electron, both projections, and complete native checkpoint files. It changes
no equations, schema-15 state, default model, or protocols 11–13.

## Operation and ownership

Generate a world and choose **Initialize regional surface flow** before another
seasonal mode. Pinned adapter settings use a 10 m initial active partition, a
900 s maximum coupling interval, compensated condensed-water stocks, finite
reference-body owners, default upslope response, roughness 0.04, and a maximum
mobility of `1e6 m²/s`. Temperature/wind defaults remain pinned. These are product
settings, not calibrated constants. Existing hourly/daily advance, run/pause,
candidate cancellation and manual-inventory isolation apply. Regenerate or open
a compatible checkpoint to switch modes; old files are never converted.

The native model owns regional liquid on initial land and one finite liquid
inventory per initial connected reference body. The global inspector adds body
liquid **once**, alongside six regional stocks. Selecting a reference-water region
shows its body ID and the whole body's mobile inventory, explicitly not an
allocation to that region. Capture, cumulative flow and unapplied requests are
not additional stocks. Camera, layers and frame rate do not advance native time.

## Display

Three additional analytical layers work in both the flat map and relief globe:

| Layer | Native quantity | Unit / fixed logarithmic saturation |
| --- | --- | --- |
| Regional pooled liquid depth | Liquid leading component / density / whole region area | m / 1,000 m |
| Cumulative neighboring surface inflow | Actual incoming directed-face histories | km³ / 1,000 km³ |
| Cumulative neighboring surface outflow | Actual outgoing directed-face histories | km³ / 1,000 km³ |

Flow layers are cumulative integrals, **not stocks or instantaneous discharge**.
Fixed scales can hide subpixel changes; received numbers remain in the inspector.
The legacy terminal-column layer is disabled in this mode. Zero-second observations
retain cumulative histories but show zero last-interval transfers/diagnostics.

**Land and water** and combined depth use live regional liquid on initial land
plus prescribed initial reference-body geometry. Separate globe caps use native
regional levels, never rewrite the bed, and obey display-only exaggeration bounds.
Flat/analytical playback updates textures without rebuilding water meshes;
showing the physical globe rebuilds dirty caps. Regional wet masks do not fabricate body
IDs. **Water bodies** explicitly shows initial reference connectivity; drainage
and basin analysis remain static initial geography. Whole-region caps and wet
masks, including tiny films, are not resolved shorelines or accurate wet areas.

## Protocol and persistence

The narrow bridge method is `initializeRegionalMoisture(epoch)`; subsequent
`seasonalMoisture(epoch, seconds)` calls advance/observe the selected mode.
Protocol 14 uses `regionalMoisture`, model 15, and separate observation pins
`regional-surface-observation-1` and `regional-surface-transfers-1`. The original
regional depth observation's shape remains unchanged.

The little-endian field-major body is exactly `(22 × N + 2 × B) × 8` bytes:

| Fields | Contents |
| --- | --- |
| 0–5 | Six rounded leading regional stock arrays |
| 6–13 | Eight last-interval surface transfer arrays |
| 14–17 | Four last-interval delayed-runoff arrays |
| 18–19 | Regional depth/level; no representable depth uses a zero level placeholder |
| 20–21 | Rounded cumulative incoming/outgoing actual-face histories |
| After `22 × N` values | Body high components, then signed low components, each length `B` |

`N` is the region count; `B` is the initial body count. Body slots follow ascending
nonzero initial body IDs reconstructed from the same world, never duplicated per
wet region. The header includes the full native budget, interval accepted-flow
and unapplied-request diagnostics, cumulative face transfer and stability bound.
The adapter checks settings/pins, clocks/shapes, finite fields, normalized body
pairs, ownership, derived depth/levels and rounded stock/flow reconciliation.
These checks do not replace native compensated per-owner checkpoint validation.

Existing seasonal file controls export the complete schema-15 JSON using
`regionalMoistureCheckpoint`, protocol 14. Loading validates a separate candidate,
preserves original numeric tokens and all regional/body/face low components,
and starts paused with an observation. Existing [file/candidate rules](seasonal-water-checkpoints.md)
apply: 64 MiB complete files, 32 MiB display bodies, no arbitrary renderer paths,
no silent upgrades. Custom headless settings reject rather than being coerced.

## Validation and remaining gates

The [compact validation record](data/regional-desktop-validation.json) retains
commands, pins, checkpoint size, stress-fixture provenance and remaining gates.

`native/tests/regional_desktop.rs` checks exact field-major native agreement,
separate body ownership, unchanged observations/geography, complete replay and
unpublished rejection of incompatible modes/intervals. Node integration tests
check daily/hourly exact state, signed-zero transport, transactional loading,
malformed frames, actual neighboring flow, independent caps and zero/all-water
extremes. An active-flow 365-day display run crosses the year boundary and replays
the next day exactly, including nonzero directed-face low components.
A default 40,962-region one-hour checkpoint occupies **17,294,662 bytes**
and continues another hour exactly like uninterrupted same-build state. This is
a short finest-grid integration test, not annual qualification or a size promise.

Electron checks cover initialization, all three new layers in both views, explicit
labels/budgets, file save/load and exact continuation. A synthetic stress fixture
funds rain at regions 8 and 156 by evaporation from actual finite bodies, with
matching histories. Visible cumulative inflow changes; saved desktop state
matches a separate headless controller. This is **not spontaneous flooding in an
ordinary generated world**. Historical annual evidence and prototype failures
remain in the model document; interface checks are not new physical evidence.

Initial body heads/coasts remain prescribed despite finite inventories. Whole-column
wet/dry and submerged-soil coupling, uniform roughness, capped mobility, dense
active-flow spatial sensitivity and long/extreme trajectories still need review.
No new GUI performance benchmark is claimed; model-3 timings do not apply to
model 15. This closes the display/persistence gate, not general hydrology,
calibrated climate, river discharge, ecological readiness, or milestones C/D.
