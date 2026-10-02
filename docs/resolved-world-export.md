# Resolved initial-world export

Status: implemented read-only desktop export for the currently accepted generated world. This is a versioned data report, not an import format or a simulation checkpoint.

`Export world data` writes `planimulation-resolved-initial-world` version 1 as deterministic UTF-8 JSON. The main process exports the validated native world it retained when the renderer accepted generation; it does not sample map pixels, regenerate from a possibly edited form, or read display exaggeration. A same-directory temporary file is renamed only after a successful bounded write. The destination remains unchanged if serialization fails. Reports over 256 MiB are rejected before replacement.

The report records the fully resolved recipe (including model and random versions), initial fingerprint, generation statistics, topology and geometry, diagnostic field, plate kinematics, crust, elevation contributions, fitted initial water, static drainage, and basin hierarchy. Dense fields are arrays in native region or basin-node order. `regionNodes` maps regions to hierarchy nodes. Units and meanings follow the corresponding versioned model documents; array names use those model names. Floating-point JSON values preserve ordinary numeric round trips in the supported JavaScript environment, but the report is not a canonical byte-for-byte native-state image.

Only **initial generated fields** appear. Manual prescribed-water input, active basin stocks, diagnostic playback, camera, selection, and future seasonal state are excluded. The separate water checkpoint is required to resume supported manual water steps. The exported report cannot be opened as a recipe or resumed. Its initial fingerprint is a regression identifier, not a cryptographic digest of every report field.

The v1 projection is explicit: adding a future model field will not silently add it to old-format reports. A new compatible field or changed meaning requires a deliberate format revision and tests. Export is intentionally user-triggered; the main process retains the initial decoded native fields to avoid copying the whole world back from the renderer. The large JSON file is for inspection and offline analysis, not the eventual compact complete simulation checkpoint.

Tests compare report fields with native generation, verify deterministic repeat export after a manual water step, reject a non-finite field without overwriting the destination, and exercise the desktop save dialog at the standard 10,242-region resolution.
