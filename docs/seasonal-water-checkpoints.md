# Desktop seasonal-water checkpoints

Status: implemented October 3, 2026. This connects the existing complete `seasonal-moisture-3` schema-3 state to desktop file operations. Physical equations, model versions, default parameters, generated geography, and manual water accounting are unchanged. It is a complete checkpoint of this bounded seasonal model, not every subsystem of a future planet simulation.

The October 4 [upslope-response increment](orographic-response.md) additionally supports complete version-4/schema-4 state with pinned default orographic settings and `orographicModelVersion`. Its export kind is `orographicMoistureCheckpoint`, protocol 12; its display kind is `orographicMoisture`, also protocol 12. Shape/bounds/candidate/file/numeric-transport rules below are shared. Schema/model/settings/module pins must agree; restoration never silently upgrades a legacy file. The validation record linked below remains historical version-3 evidence; the new mode has its [own evidence and retained failures](data/orographic-response-validation.json).

## Use and saved scope

The October 9 [unified soil-water desktop mode](soil-water-desktop.md) additionally supports family `regional-seasonal-water-1`, schema 1, via `soilMoistureCheckpoint` and `soilMoisture`, protocol 15. This is not legacy seasonal schema 1. Its five paired regional owners, counted-once body pairs, complete directed histories and persistent donor-retention diagnostics stay distinct from the six-stock legacy models described below. Original-text `restoreSoilMoisture`, candidate acceptance and shared bounded file operations preserve complete native state without migration. Only the family's pinned product settings are accepted.

The October 5 [regional-water desktop increment](regional-water-desktop.md) additionally supports the existing complete model-15/schema-15 checkpoint via `regionalMoistureCheckpoint`, protocol 14. It preserves finite body high/low inventories, regional liquid high/low fields, capture provenance and complete directed-face high/low histories. Its distinct display frame does not expand or reinterpret protocols 11–13. Body liquid is counted once, never per wet region. Only its pinned product settings (10 m active partition, 900 s coupling ceiling and default capped mobility) are accepted; other headless settings explicitly reject. Existing token/file/candidate rules below apply unchanged.

The [surface/terminal precision increment](surface-precision.md) separately supports version 7/schema 7 via `preciseMoistureCheckpoint`, protocol 13. Its four signed low-component arrays and terminal-stock module pin are native physical state, not ledger corrections. Original decimal tokens, candidate validation, exact continuation, and file bounds remain unchanged. Headless-only schemas 5/6 reject; no file is silently upgraded. Historical version-3 validation below remains separate evidence.

Pause seasonal playback and wait for its outstanding interval to finish. **Save seasonal checkpoint** exports a settled native snapshot. **Open seasonal checkpoint** can replace either an ordinary world or a paused seasonal/manual session through a separate candidate process. A canceled or failed open keeps the accepted native state and its displayed fields.

The checkpoint contains the full generation recipe; moisture, transport, temperature, wind, surface, and runoff model versions; all resolved climate settings; elapsed integer seconds; six owned stock arrays; cumulative regional surface/routing transfers; signed Kahan roundoff corrections; and cumulative evaporation/precipitation. Restore regenerates fixed physical geometry and forcing using the pinned recipe and validates the full checkpoint in Rust.

The desktop supports only the default moisture/temperature/wind settings already supported by its viewer. Valid headless checkpoints with other settings reject explicitly instead of being reinterpreted. Legacy seasonal schemas 1/2, unknown fields, mismatched versions, invalid shapes, clocks, stocks, graph ownership, capacities, and budgets reject. The distinct unified family's schema 1 is dispatched by family, not the schema number alone. A compatible schema and conserved totals do not prove historical reachability of an edited file. Cross-build/platform bitwise replay and migrations are not promised.

Camera, selected region, current layer, diagnostic diffusion, and the last display interval's transfer/rate arrays are not checkpoint state. The loaded view starts paused with a zero-second observation: current stocks and cumulative budgets are restored, while last-interval transfers/rates are zero until the next advance. This avoids inventing rates from cumulative totals. The displayed fingerprint still describes initial geography, not live seasonal stocks.

Recipes, resolved initial-world reports, and the separate exact manual water checkpoints do not contain seasonal progress. Regeneration, closing, or reloading discards unsaved progress. This feature does not convert between the exact manual basin inventory and the approximate finite seasonal partition.

## Native protocol and exact numeric transport

Two named preload operations expose file dialogs, not arbitrary paths or raw IPC: `openSeasonalCheckpoint()` and `saveSeasonalCheckpoint(epoch)`.

Native `exportMoisture` validates the owned state and serializes its complete checkpoint. Its response is `kind: "moistureCheckpoint"`, protocol 11, with schema/model/clock metadata and a UTF-8 JSON body. It is distinct from the eighteen-field `moisture` display frame. World protocols 9/10 and manual checkpoint layout remain unchanged.

Native `restoreMoisture` receives `checkpointJson` as a JSON string inside the command envelope. JavaScript preflights recipe/version/clock metadata but sends the **original JSON text** to Rust. It does not parse and stringify physical numbers, which could discard signed zero. The pinned `serde_json` configuration includes `float_roundtrip`. Tests compare the complete canonical checkpoint text before/after restoration and after subsequent physical steps, including nonzero signed roundoff corrections and a valid negative-zero correction.

Limits are explicit and kind-specific:

| Payload | Bound |
| --- | ---: |
| Complete seasonal checkpoint file/body | 64 MiB |
| Escaped seasonal restore command envelope | 128 MiB + 1 KiB |
| Ordinary commands, including manual checkpoint restoration | 8 MiB |
| Ordinary binary response bodies, including seasonal display frames | 32 MiB |
| Response JSON header | 32 KiB |

Export reserves one byte for the file's final newline. Import reads bounded chunks, checks growth against the limit, requires a regular file, and rejects invalid UTF-8. Limits are allocation safeguards, not a promise that arbitrary edited/pretty-printed files or future resolutions fit. The existing 60-second native timeout and single-command backpressure apply. JSON serialization, parsing, pipes, and Electron IPC still allocate/copy; checkpoint I/O is an infrequent operation, not zero-copy playback.

## Candidate and file lifecycle

1. Read/preflight a bounded file; an intent revision prevents a delayed file dialog/read from resurrecting a canceled or replaced request.
2. Generate the checkpoint's recipe in a candidate process, separate from the accepted world.
3. Verify recipe equality and default settings; restore with the existing complete native validator. Current implementation regenerates inside `Model::restore` as well; no unchecked fast path is introduced.
4. Decode a zero-second observation, prepare both GPU projections, then accept that candidate. Its restored clock, initialization flag, and previous cumulative budget become the controller baseline for the next interval.
5. If validation/preparation is canceled, close only the candidate. An already requested old-world interval can still finish; actual epoch replacement prevents its result overwriting the new world.

Save captures native JSON before opening its dialog and verifies that the same accepted world remains before publishing. Write uses an exclusively created same-directory temporary file, awaits its data write, requests [`FileHandle.sync`](https://nodejs.org/api/fs.html#filehandlesync), closes it, and then awaits [`rename`](https://nodejs.org/api/fs.html#fspromisesrenameoldpath-newpath). Ordinary write/rename failure removes only that operation's temporary file and retains the existing target. The manual checkpoint writer shares this helper without changing its schema or 8 MiB limit.

This is not a full power-loss durability guarantee: directory metadata is not explicitly synced, device/OS flush semantics vary, and a process crash can leave a temporary file. Concurrent external edits to the destination are not coordinated or merged. Only the trusted main-frame bridge can invoke these file operations; renderer code never receives a filesystem path capability.

## Validation

The retained [validation record](data/seasonal-checkpoint-validation.json) describes checks on the supported Linux build:

- Complete native export/restore matches the authoritative checkpoint without changing the world or state.
- Adapter save/load at day 365, with nonzero surface/routing Kahan corrections, exactly matches uninterrupted day 366 stocks, transfers, budgets, and the entire serialized checkpoint.
- A valid signed-zero correction survives the JSON path exactly; malformed checkpoints/settings and both inventory modes remain isolated.
- Cancellation during candidate initialization preserves an in-flight accepted step; cancellation after preparation preserves the old session; stale epochs cannot export or replace a newer accepted world.
- A level-6, 40,962-region two-day checkpoint occupies **15,001,764 bytes** for the default recipe. Reload and an additional one-hour advance exactly match uninterrupted complete state. This short test does not establish long-run fine-grid stability or a maximum possible checkpoint size.
- Real Electron checks exercise both projections, save/cancel/write failure, damaged/oversized imports, delayed stale dialogs, recipe/clock/budget restoration, and an identical next-day complete JSON checkpoint.
- File helper tests cover exact-limit reads, multiple chunks, invalid UTF-8, oversize/non-file rejection, overwrite, and temporary cleanup after rename failure.

No model equation or tolerance changes were needed. Existing [nineteen-case physical evidence](seasonal-moisture.md#measured-version-3-evidence) remains separate from these adapter/persistence checks. Groundwater, terrain-dependent precipitation, basin spill/merge ownership, changing coasts, thermodynamic freezing, calibration, and history timelines are still future work; milestone D remains incomplete.
